//! Apple platform implementation using direct Network.framework FFI
//!
//! Uses direct C bindings to Network.framework for monitoring network path changes.

use super::*;
use libc::{c_void, freeifaddrs, getifaddrs, if_nametoindex, ifaddrs, AF_INET, AF_INET6};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::Arc;

// Include network_sys as a submodule
#[path = "network_sys.rs"]
mod network_sys;
use network_sys::*;

type PathChangeCallback = Box<dyn Fn(ChangeEvent) + Send + 'static>;

pub struct AppleDirectMonitor {
    monitor: Option<nw_path_monitor_t>,
    queue: Option<dispatch_queue_t>,
    callback_holder: Option<Arc<Mutex<PathChangeCallback>>>,
    update_block: Option<Box<PathUpdateBlock>>,
}

unsafe impl Send for AppleDirectMonitor {}
unsafe impl Sync for AppleDirectMonitor {}

impl Drop for AppleDirectMonitor {
    fn drop(&mut self) {
        unsafe {
            if let Some(monitor) = self.monitor {
                nw_path_monitor_cancel(monitor);
                nw_release(monitor as *mut c_void);
            }
            if let Some(queue) = self.queue {
                dispatch_release(queue);
            }
        }
        // Block will be released when dropped
        self.update_block = None;
    }
}

impl PlatformMonitor for AppleDirectMonitor {
    fn list_interfaces(&self) -> Result<Vec<Interface>, Error> {
        // Use getifaddrs to get interface information
        unsafe {
            let mut ifap: *mut ifaddrs = ptr::null_mut();
            if getifaddrs(&mut ifap) != 0 {
                return Err(Error::PlatformError("Failed to get interfaces".into()));
            }

            let mut interfaces_map: HashMap<String, Interface> = HashMap::new();
            let mut current = ifap;

            while !current.is_null() {
                let ifa = &*current;
                if let Some(name) = ifa.ifa_name.as_ref() {
                    let name_str = CStr::from_ptr(name).to_string_lossy().to_string();
                    let name_cstring = CString::new(name_str.as_str()).unwrap();

                    // Get interface index
                    let if_index = if_nametoindex(name_cstring.as_ptr());

                    let interface = interfaces_map.entry(name_str.clone()).or_insert(Interface {
                        name: name_str.clone(),
                        index: if_index,
                        ips: Vec::new(),
                        status: Status::Unknown,
                        interface_type: detect_interface_type(&name_str),
                        is_expensive: detect_expensive_interface(&name_str),
                    });

                    // Check if interface is up
                    if ifa.ifa_flags & libc::IFF_UP as u32 != 0 {
                        interface.status = Status::Up;
                    } else {
                        interface.status = Status::Down;
                    }

                    // Extract IP addresses
                    if let Some(addr) = ifa.ifa_addr.as_ref() {
                        match addr.sa_family as i32 {
                            AF_INET => {
                                let sockaddr = addr as *const _ as *const libc::sockaddr_in;
                                let ip = Ipv4Addr::from((*sockaddr).sin_addr.s_addr.to_be());
                                interface.ips.push(IpAddr::V4(ip));
                            }
                            AF_INET6 => {
                                let sockaddr = addr as *const _ as *const libc::sockaddr_in6;
                                let ip = Ipv6Addr::from((*sockaddr).sin6_addr.s6_addr);
                                interface.ips.push(IpAddr::V6(ip));
                            }
                            _ => {}
                        }
                    }
                }
                current = ifa.ifa_next;
            }

            // Enhance with Network.framework info if we have a monitor
            if let Some(_monitor) = self.monitor {
                // We can't easily get the current path synchronously without blocks
                // This is a limitation of the API design
                // In practice, the callback mechanism would keep this updated
            }

            freeifaddrs(ifap);
            Ok(interfaces_map.into_values().collect())
        }
    }

    fn start_watching(
        &mut self,
        callback: Box<dyn Fn(ChangeEvent) + Send + 'static>,
    ) -> PlatformHandle {
        self.callback_holder = Some(Arc::new(Mutex::new(callback)));

        unsafe {
            // Create NWPathMonitor
            let monitor = nw_path_monitor_create();
            if monitor.is_null() {
                return Box::new(MonitorStopHandle { monitor: None });
            }
            self.monitor = Some(monitor);

            // Create dispatch queue
            let queue_name = CString::new("com.tapsrs.pathmonitor").unwrap();
            let queue = dispatch_queue_create(queue_name.as_ptr(), ptr::null());
            if queue.is_null() {
                nw_release(monitor as *mut c_void);
                self.monitor = None;
                return Box::new(MonitorStopHandle { monitor: None });
            }
            self.queue = Some(queue);

            // Set up path update handler
            let callback_holder = self.callback_holder.as_ref().unwrap().clone();
            let update_block = PathUpdateBlock::new(move |path: nw_path_t| {
                // Get path status
                let status = nw_path_get_status(path);
                let is_expensive = nw_path_is_expensive(path);
                let is_constrained = nw_path_is_constrained(path);

                // Check what interfaces are being used
                let uses_wifi = nw_path_uses_interface_type(path, NW_INTERFACE_TYPE_WIFI);
                let uses_cellular = nw_path_uses_interface_type(path, NW_INTERFACE_TYPE_CELLULAR);
                let uses_wired = nw_path_uses_interface_type(path, NW_INTERFACE_TYPE_WIRED);

                // Log the change
                log::info!("Path changed: status={status}, expensive={is_expensive}, constrained={is_constrained}, wifi={uses_wifi}, cellular={uses_cellular}, wired={uses_wired}");

                // Notify via callback
                let callback = callback_holder.lock().unwrap();
                callback(ChangeEvent::PathChanged {
                    description: format!("Network path changed (status: {}, expensive: {}, wifi: {}, cellular: {}, wired: {})", 
                        match status {
                            1 => "satisfied",
                            2 => "unsatisfied", 
                            3 => "satisfiable",
                            _ => "invalid",
                        },
                        is_expensive,
                        uses_wifi,
                        uses_cellular,
                        uses_wired
                    ),
                });
            });

            nw_path_monitor_set_update_handler(monitor, update_block.as_ptr());
            nw_path_monitor_set_queue(monitor, queue);
            nw_path_monitor_start(monitor);

            self.update_block = Some(Box::new(update_block));

            // Return handle that will stop monitoring when dropped
            Box::new(MonitorStopHandle {
                monitor: self.monitor,
            })
        }
    }
}

struct MonitorStopHandle {
    monitor: Option<nw_path_monitor_t>,
}

unsafe impl Send for MonitorStopHandle {}

impl Drop for MonitorStopHandle {
    fn drop(&mut self) {
        if let Some(monitor) = self.monitor {
            unsafe {
                nw_path_monitor_cancel(monitor);
            }
        }
    }
}

fn detect_interface_type(name: &str) -> String {
    match name {
        // Loopback
        "lo0" | "lo" => "loopback".to_string(),

        // WiFi - en0 is typically WiFi on macOS
        "en0" => "wifi".to_string(),

        // Ethernet - other en interfaces
        name if name.starts_with("en") => "ethernet".to_string(),

        // Cellular/Mobile data
        name if name.starts_with("pdp_ip") => "cellular".to_string(),

        // Thunderbolt bridge
        name if name.starts_with("bridge") => "bridge".to_string(),

        // VPN interfaces
        name if name.starts_with("utun") => "vpn".to_string(),
        name if name.starts_with("ipsec") => "vpn".to_string(),
        name if name.starts_with("ppp") => "vpn".to_string(),

        // Bluetooth PAN
        name if name.starts_with("awdl") => "awdl".to_string(), // Apple Wireless Direct Link

        // FireWire
        name if name.starts_with("fw") => "firewire".to_string(),

        // Default
        _ => "unknown".to_string(),
    }
}

fn detect_expensive_interface(name: &str) -> bool {
    // Mark cellular and VPN connections as expensive by default
    matches!(detect_interface_type(name).as_str(), "cellular" | "vpn")
}

pub fn create_platform_impl() -> Result<Box<dyn PlatformMonitor + Send + Sync>, Error> {
    Ok(Box::new(AppleDirectMonitor {
        monitor: None,
        queue: None,
        callback_holder: None,
        update_block: None,
    }))
}
