//! Windows platform implementation using IP Helper API
//! 
//! Uses NotifyIpInterfaceChange and GetAdaptersAddresses for monitoring.

use super::*;
use std::ptr;
use std::mem;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows_sys::Win32::NetworkManagement::IpHelper::*;
use windows_sys::Win32::Foundation::{HANDLE, ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS};
use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};

pub struct WindowsMonitor {
    notify_handle: Option<HANDLE>,
    callback_holder: Option<Arc<Mutex<Box<dyn Fn(ChangeEvent) + Send + 'static>>>>,
}

unsafe impl Send for WindowsMonitor {}
unsafe impl Sync for WindowsMonitor {}

impl PlatformMonitor for WindowsMonitor {
    fn list_interfaces(&self) -> Result<Vec<Interface>, Error> {
        unsafe {
            let mut buffer_size: u32 = 15000; // Initial buffer size
            let mut adapters_buffer = vec![0u8; buffer_size as usize];
            
            let family = AF_UNSPEC;
            let flags = GAA_FLAG_INCLUDE_PREFIX;
            
            loop {
                let result = GetAdaptersAddresses(
                    family as u32,
                    flags,
                    ptr::null_mut(),
                    adapters_buffer.as_mut_ptr() as *mut _,
                    &mut buffer_size,
                );
                
                match result {
                    ERROR_SUCCESS => break,
                    ERROR_BUFFER_OVERFLOW => {
                        adapters_buffer.resize(buffer_size as usize, 0);
                        continue;
                    }
                    _ => return Err(Error::PlatformError(format!("GetAdaptersAddresses failed: {}", result))),
                }
            }
            
            let mut interfaces = Vec::new();
            let mut current = adapters_buffer.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
            
            while !current.is_null() {
                let adapter = &*current;
                
                // Convert friendly name from wide string
                let name_len = (0..).position(|i| *adapter.FriendlyName.offset(i) == 0).unwrap_or(0);
                let name_slice = std::slice::from_raw_parts(adapter.FriendlyName, name_len);
                let name = OsString::from_wide(name_slice).to_string_lossy().to_string();
                
                let mut interface = Interface {
                    name,
                    index: adapter.IfIndex,
                    ips: Vec::new(),
                    status: if adapter.OperStatus == 1 { Status::Up } else { Status::Down },
                    interface_type: detect_interface_type(adapter.IfType),
                    is_expensive: false, // TODO: Detect from connection profile
                };
                
                // Collect IP addresses
                let mut unicast = adapter.FirstUnicastAddress;
                while !unicast.is_null() {
                    let addr = &*unicast;
                    let sockaddr = &*addr.Address.lpSockaddr;
                    
                    match sockaddr.sa_family {
                        AF_INET => {
                            let sockaddr_in = addr.Address.lpSockaddr as *const windows_sys::Win32::Networking::WinSock::SOCKADDR_IN;
                            let ip = Ipv4Addr::from((*sockaddr_in).sin_addr.S_un.S_addr.to_be());
                            interface.ips.push(IpAddr::V4(ip));
                        }
                        AF_INET6 => {
                            let sockaddr_in6 = addr.Address.lpSockaddr as *const windows_sys::Win32::Networking::WinSock::SOCKADDR_IN6;
                            let ip = Ipv6Addr::from((*sockaddr_in6).sin6_addr.u.Byte);
                            interface.ips.push(IpAddr::V6(ip));
                        }
                        _ => {}
                    }
                    
                    unicast = addr.Next;
                }
                
                interfaces.push(interface);
                current = adapter.Next;
            }
            
            Ok(interfaces)
        }
    }

    fn start_watching(&mut self, callback: Box<dyn Fn(ChangeEvent) + Send + 'static>) -> PlatformHandle {
        self.callback_holder = Some(Arc::new(Mutex::new(callback)));
        let callback_holder = self.callback_holder.as_ref().unwrap().clone();
        
        unsafe {
            let mut handle: HANDLE = 0;
            let context = Box::into_raw(Box::new(callback_holder)) as *mut _;
            
            let result = NotifyIpInterfaceChange(
                AF_UNSPEC as u16,
                Some(ip_interface_change_callback),
                context,
                false as u8,
                &mut handle,
            );
            
            if result != 0 {
                Box::from_raw(context as *mut Arc<Mutex<Box<dyn Fn(ChangeEvent) + Send + 'static>>>);
                return Box::new(WindowsMonitorHandle { handle: 0 });
            }
            
            self.notify_handle = Some(handle);
            Box::new(WindowsMonitorHandle { handle })
        }
    }
}

struct WindowsMonitorHandle {
    handle: HANDLE,
}

impl Drop for WindowsMonitorHandle {
    fn drop(&mut self) {
        unsafe {
            if self.handle != 0 {
                CancelMibChangeNotify2(self.handle);
            }
        }
    }
}

unsafe extern "system" fn ip_interface_change_callback(
    _context: *mut std::ffi::c_void,
    _row: *mut MIB_IPINTERFACE_ROW,
    _notification_type: u32,
) {
    // In a real implementation, we would:
    // 1. Cast context back to the callback holder
    // 2. Determine what changed
    // 3. Call the callback with appropriate ChangeEvent
}

fn detect_interface_type(if_type: u32) -> String {
    match if_type {
        IF_TYPE_ETHERNET_CSMACD => "ethernet".to_string(),
        IF_TYPE_IEEE80211 => "wifi".to_string(),
        IF_TYPE_WWANPP | IF_TYPE_WWANPP2 => "cellular".to_string(),
        IF_TYPE_SOFTWARE_LOOPBACK => "loopback".to_string(),
        _ => "unknown".to_string(),
    }
}

pub fn create_platform_impl() -> Result<Box<dyn PlatformMonitor + Send + Sync>, Error> {
    Ok(Box::new(WindowsMonitor {
        notify_handle: None,
        callback_holder: None,
    }))
}