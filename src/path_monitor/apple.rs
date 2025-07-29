//! Apple platform implementation using Network.framework
//! 
//! Uses NWPathMonitor for monitoring network path changes and
//! combines with system calls for interface enumeration.

use super::*;
use objc::{msg_send, sel, sel_impl, class};
use objc::runtime::Object;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::ptr;
use std::sync::Arc;
use libc::{getifaddrs, freeifaddrs, ifaddrs, AF_INET, AF_INET6, if_nametoindex};

#[link(name = "Network", kind = "framework")]
extern "C" {}

pub struct AppleMonitor {
    monitor: Option<*mut Object>,
    queue: Option<*mut Object>,
    callback_holder: Option<Arc<Mutex<Box<dyn Fn(ChangeEvent) + Send + 'static>>>>,
}

unsafe impl Send for AppleMonitor {}
unsafe impl Sync for AppleMonitor {}

impl PlatformMonitor for AppleMonitor {
    fn list_interfaces(&self) -> Result<Vec<Interface>, Error> {
        // Use getifaddrs to get interface information
        unsafe {
            let mut ifap: *mut ifaddrs = ptr::null_mut();
            if getifaddrs(&mut ifap) != 0 {
                return Err(Error::PlatformError("Failed to get interfaces".into()));
            }
            
            let mut interfaces_map: HashMap<String, Interface> = HashMap::new();
            let mut current = ifap;
            
            // Get current path info if available
            let is_expensive_map = self.get_expensive_interfaces();
            
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
                        is_expensive: is_expensive_map.get(&name_str).copied().unwrap_or(false),
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
            
            freeifaddrs(ifap);
            Ok(interfaces_map.into_values().collect())
        }
    }

    fn start_watching(&mut self, callback: Box<dyn Fn(ChangeEvent) + Send + 'static>) -> PlatformHandle {
        self.callback_holder = Some(Arc::new(Mutex::new(callback)));
        
        unsafe {
            // Create NWPathMonitor
            let monitor_class = class!(NWPathMonitor);
            let monitor: *mut Object = msg_send![monitor_class, alloc];
            let monitor: *mut Object = msg_send![monitor, init];
            self.monitor = Some(monitor);
            
            // Create dispatch queue
            let queue_name = CString::new("com.tapsrs.pathmonitor").unwrap();
            let queue = dispatch_queue_create(queue_name.as_ptr(), ptr::null());
            self.queue = Some(queue as *mut Object);
            
            // Set up path update handler
            let callback_holder = self.callback_holder.as_ref().unwrap().clone();
            let handler_block = create_path_update_handler(callback_holder);
            
            let _: () = msg_send![monitor, setPathUpdateHandler: handler_block];
            let _: () = msg_send![monitor, startWithQueue: queue];
            
            // Return handle that will stop monitoring when dropped
            Box::new(MonitorStopHandle {
                monitor: self.monitor.unwrap(),
            })
        }
    }
}

impl AppleMonitor {
    /// Get expensive interfaces from NWPath
    unsafe fn get_expensive_interfaces(&self) -> HashMap<String, bool> {
        let mut expensive_map = HashMap::new();
        
        // If we have a current path from the monitor, use it
        if let Some(monitor) = self.monitor {
            let path: *mut Object = msg_send![monitor, currentPath];
            if !path.is_null() {
                self.parse_path_expensive_info(path, &mut expensive_map);
            }
        }
        
        // Also check using default path monitor
        let monitor_class = class!(NWPathMonitor);
        let temp_monitor: *mut Object = msg_send![monitor_class, alloc];
        let temp_monitor: *mut Object = msg_send![temp_monitor, init];
        let current_path: *mut Object = msg_send![temp_monitor, currentPath];
        
        if !current_path.is_null() {
            self.parse_path_expensive_info(current_path, &mut expensive_map);
        }
        
        let _: () = msg_send![temp_monitor, cancel];
        let _: () = msg_send![temp_monitor, release];
        
        expensive_map
    }
    
    /// Parse NWPath to extract expensive interface information
    unsafe fn parse_path_expensive_info(&self, path: *mut Object, expensive_map: &mut HashMap<String, bool>) {
        // Check if path is expensive overall
        let is_expensive: bool = msg_send![path, isExpensive];
        
        // Get available interfaces from the path
        let interfaces: *mut Object = msg_send![path, availableInterfaces];
        if !interfaces.is_null() {
            // Enumerate through the interfaces
            let count: usize = msg_send![interfaces, count];
            for i in 0..count {
                let interface: *mut Object = msg_send![interfaces, objectAtIndex: i];
                if !interface.is_null() {
                    // Get interface name
                    let name: *const c_char = msg_send![interface, name];
                    if !name.is_null() {
                        let name_str = CStr::from_ptr(name).to_string_lossy().to_string();
                        
                        // Check if this specific interface is expensive
                        // For now, we'll use the overall path expense status
                        // In a more complete implementation, we'd check per-interface
                        expensive_map.insert(name_str, is_expensive);
                    }
                }
            }
        }
        
        // Also check if constrained (low data mode)
        let is_constrained: bool = msg_send![path, isConstrained];
        if is_constrained {
            // Mark all interfaces as expensive if in constrained mode
            for (_, expensive) in expensive_map.iter_mut() {
                *expensive = true;
            }
        }
    }
}

struct MonitorStopHandle {
    monitor: *mut Object,
}

unsafe impl Send for MonitorStopHandle {}

impl Drop for MonitorStopHandle {
    fn drop(&mut self) {
        unsafe {
            let _: () = msg_send![self.monitor, cancel];
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

pub fn create_platform_impl() -> Result<Box<dyn PlatformMonitor + Send + Sync>, Error> {
    Ok(Box::new(AppleMonitor {
        monitor: None,
        queue: None,
        callback_holder: None,
    }))
}

// FFI declarations for dispatch and blocks
#[link(name = "System", kind = "dylib")]
extern "C" {
    fn dispatch_queue_create(label: *const c_char, attr: *const c_void) -> *mut c_void;
}

// Helper to create path update handler block
unsafe fn create_path_update_handler(
    _callback_holder: Arc<Mutex<Box<dyn Fn(ChangeEvent) + Send + 'static>>>
) -> *mut Object {
    // This is a simplified version - in reality, we'd need to properly
    // create an Objective-C block that captures the callback
    // For now, return a placeholder
    ptr::null_mut()
}