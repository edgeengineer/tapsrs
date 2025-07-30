//! Windows platform implementation using IP Helper API
//!
//! Uses NotifyUnicastIpAddressChange and GetAdaptersAddresses for monitoring.

use super::*;
use std::collections::HashMap;
use std::ffi::c_void;
use std::net::IpAddr;
use std::sync::Mutex;

use ::windows::Win32::Foundation::{
    ERROR_ADDRESS_NOT_ASSOCIATED, ERROR_BUFFER_OVERFLOW,
    ERROR_INVALID_PARAMETER, ERROR_NOT_ENOUGH_MEMORY, ERROR_NO_DATA, ERROR_SUCCESS,
    NO_ERROR, WIN32_ERROR, BOOLEAN, HANDLE,
};
use ::windows::Win32::NetworkManagement::IpHelper::{
    CancelMibChangeNotify2, GetAdaptersAddresses, NotifyUnicastIpAddressChange,
    MIB_NOTIFICATION_TYPE, MIB_UNICASTIPADDRESS_ROW, GAA_FLAG_SKIP_ANYCAST,
    GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH,
};
use ::windows::Win32::NetworkManagement::Ndis::IfOperStatusDown;
use ::windows::Win32::Networking::WinSock::{
    AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6,
};

// Interface type constants from Windows SDK
const IF_TYPE_ETHERNET_CSMACD: u32 = 6;
const IF_TYPE_IEEE80211: u32 = 71;
const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
const IF_TYPE_WWANPP: u32 = 243;
const IF_TYPE_WWANPP2: u32 = 244;

/// State for tracking interface changes
struct WatchState {
    /// The last known list of interfaces for diffing
    prev_interfaces: Vec<Interface>,
    /// User's callback wrapped for thread safety
    cb: Box<dyn Fn(ChangeEvent) + Send + 'static>,
}

/// Windows-specific monitor implementation
pub struct WindowsMonitor {
    /// Current state for change detection
    state: Option<Arc<Mutex<WatchState>>>,
}

unsafe impl Send for WindowsMonitor {}
unsafe impl Sync for WindowsMonitor {}

impl WindowsMonitor {
    /// List all network interfaces using GetAdaptersAddresses
    fn list_interfaces_internal() -> Result<Vec<Interface>, Error> {
        let mut interfaces = Vec::new();
        
        // Microsoft recommends a 15 KB initial buffer
        let start_size = 15 * 1024;
        let mut buf: Vec<u8> = vec![0; start_size];
        let mut size_pointer: u32 = start_size as u32;

        unsafe {
            loop {
                let bufptr = buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
                let res = GetAdaptersAddresses(
                    AF_UNSPEC.0 as u32,
                    GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST,
                    None,
                    Some(bufptr),
                    &mut size_pointer,
                );
                
                match WIN32_ERROR(res) {
                    ERROR_SUCCESS => break,
                    ERROR_ADDRESS_NOT_ASSOCIATED => {
                        return Err(Error::PlatformError("Address not associated".to_string()))
                    }
                    ERROR_BUFFER_OVERFLOW => {
                        buf.resize(size_pointer as usize, 0);
                        continue;
                    }
                    ERROR_INVALID_PARAMETER => {
                        return Err(Error::PlatformError("Invalid parameter".to_string()))
                    }
                    ERROR_NOT_ENOUGH_MEMORY => {
                        return Err(Error::PlatformError("Not enough memory".to_string()))
                    }
                    ERROR_NO_DATA => return Ok(Vec::new()), // No interfaces
                    _ => {
                        return Err(Error::PlatformError(format!(
                            "GetAdaptersAddresses failed with error: {}",
                            res
                        )))
                    }
                }
            }

            // Parse the adapter list
            let mut adapter_ptr = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
            while !adapter_ptr.is_null() {
                let adapter = &*adapter_ptr;
                
                // Skip interfaces that are down
                if adapter.OperStatus == IfOperStatusDown {
                    adapter_ptr = adapter.Next;
                    continue;
                }

                // Get interface name
                let name = adapter
                    .FriendlyName
                    .to_string()
                    .unwrap_or_else(|_| format!("Unknown{}", adapter.Ipv6IfIndex));

                // Collect IP addresses
                let mut ips = vec![];
                let mut unicast_ptr = adapter.FirstUnicastAddress;
                while !unicast_ptr.is_null() {
                    let unicast = &*unicast_ptr;
                    let sockaddr = &*unicast.Address.lpSockaddr;
                    
                    let ip = match sockaddr.sa_family {
                        AF_INET => {
                            let sockaddr_in = &*(unicast.Address.lpSockaddr as *const SOCKADDR_IN);
                            IpAddr::V4(sockaddr_in.sin_addr.into())
                        }
                        AF_INET6 => {
                            let sockaddr_in6 = &*(unicast.Address.lpSockaddr as *const SOCKADDR_IN6);
                            IpAddr::V6(sockaddr_in6.sin6_addr.into())
                        }
                        _ => {
                            unicast_ptr = unicast.Next;
                            continue;
                        }
                    };
                    
                    ips.push(ip);
                    unicast_ptr = unicast.Next;
                }

                let interface = Interface {
                    name,
                    index: adapter.Ipv6IfIndex, // Use IPv6 index as it's more consistent
                    ips,
                    status: if adapter.OperStatus == IfOperStatusDown {
                        Status::Down
                    } else {
                        Status::Up
                    },
                    interface_type: detect_interface_type(adapter.IfType),
                    is_expensive: false, // TODO: Detect from connection profile API
                };

                interfaces.push(interface);
                adapter_ptr = adapter.Next;
            }
        }

        Ok(interfaces)
    }
}

impl PlatformMonitor for WindowsMonitor {
    fn list_interfaces(&self) -> Result<Vec<Interface>, Error> {
        Self::list_interfaces_internal()
    }

    fn start_watching(
        &mut self,
        callback: Box<dyn Fn(ChangeEvent) + Send + 'static>,
    ) -> PlatformHandle {
        // Get initial interface list
        let prev_interfaces = Self::list_interfaces_internal().unwrap_or_default();
        
        // Create the watch state
        let state = Arc::new(Mutex::new(WatchState {
            prev_interfaces,
            cb: callback,
        }));
        
        // Get a raw pointer to pass to the Windows API
        let state_ptr = Arc::as_ptr(&state) as *const c_void;
        
        // Store the state in self to keep it alive
        self.state = Some(state.clone());
        
        let mut handle = HANDLE::default();
        
        unsafe {
            let res = NotifyUnicastIpAddressChange(
                AF_UNSPEC,
                Some(notif_callback),
                Some(state_ptr),
                BOOLEAN(0), // Not initial notification
                &mut handle,
            );
            
            match res {
                NO_ERROR => {
                    // Trigger an initial update to establish baseline
                    if let Ok(new_list) = Self::list_interfaces_internal() {
                        handle_notif(&mut state.lock().unwrap(), new_list);
                    }
                    
                    Box::new(WindowsWatchHandle {
                        handle,
                        _state: state,
                    })
                }
                _ => {
                    // Return a dummy handle that does nothing
                    Box::new(WindowsWatchHandle {
                        handle: HANDLE::default(),
                        _state: state,
                    })
                }
            }
        }
    }
}

/// Handle for canceling the network change notifications
struct WindowsWatchHandle {
    handle: HANDLE,
    _state: Arc<Mutex<WatchState>>, // Keep state alive
}

unsafe impl Send for WindowsWatchHandle {}

impl Drop for WindowsWatchHandle {
    fn drop(&mut self) {
        unsafe {
            if !self.handle.is_invalid() {
                let _ = CancelMibChangeNotify2(self.handle);
            }
        }
    }
}

/// Callback invoked by Windows when network changes occur
unsafe extern "system" fn notif_callback(
    ctx: *const c_void,
    _row: *const MIB_UNICASTIPADDRESS_ROW,
    _notification_type: MIB_NOTIFICATION_TYPE,
) {
    if ctx.is_null() {
        return;
    }
    
    let state_ptr = ctx as *const Mutex<WatchState>;
    let state_mutex = &*state_ptr;
    
    if let Ok(mut state_guard) = state_mutex.lock() {
        if let Ok(new_list) = WindowsMonitor::list_interfaces_internal() {
            handle_notif(&mut state_guard, new_list);
        }
    }
}

/// Handle a notification by comparing old and new interface lists
fn handle_notif(state: &mut WatchState, new_interfaces: Vec<Interface>) {
    // Create maps for efficient comparison
    let old_map: HashMap<u32, &Interface> = state.prev_interfaces
        .iter()
        .map(|iface| (iface.index, iface))
        .collect();
    
    let new_map: HashMap<u32, &Interface> = new_interfaces
        .iter()
        .map(|iface| (iface.index, iface))
        .collect();
    
    // Find additions
    for (index, new_iface) in &new_map {
        if !old_map.contains_key(index) {
            (state.cb)(ChangeEvent::Added((*new_iface).clone()));
        }
    }
    
    // Find removals
    for (index, old_iface) in &old_map {
        if !new_map.contains_key(index) {
            (state.cb)(ChangeEvent::Removed((*old_iface).clone()));
        }
    }
    
    // Find modifications
    for (index, new_iface) in &new_map {
        if let Some(old_iface) = old_map.get(index) {
            if !interfaces_equal(old_iface, new_iface) {
                (state.cb)(ChangeEvent::Modified {
                    old: (*old_iface).clone(),
                    new: (*new_iface).clone(),
                });
            }
        }
    }
    
    // Update the stored state
    state.prev_interfaces = new_interfaces;
}

/// Compare two interfaces for equality
fn interfaces_equal(a: &Interface, b: &Interface) -> bool {
    a.name == b.name
        && a.index == b.index
        && a.status == b.status
        && a.interface_type == b.interface_type
        && a.is_expensive == b.is_expensive
        && ips_equal(&a.ips, &b.ips)
}

/// Compare two IP lists for equality (order-independent)
fn ips_equal(a: &[IpAddr], b: &[IpAddr]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    
    let mut a_sorted = a.to_vec();
    let mut b_sorted = b.to_vec();
    a_sorted.sort();
    b_sorted.sort();
    
    a_sorted == b_sorted
}

/// Detect interface type from Windows interface type constant
fn detect_interface_type(if_type: u32) -> String {
    match if_type {
        IF_TYPE_ETHERNET_CSMACD => "ethernet".to_string(),
        IF_TYPE_IEEE80211 => "wifi".to_string(),
        IF_TYPE_WWANPP | IF_TYPE_WWANPP2 => "cellular".to_string(),
        IF_TYPE_SOFTWARE_LOOPBACK => "loopback".to_string(),
        _ => "unknown".to_string(),
    }
}

/// Create the platform implementation
pub fn create_platform_impl() -> Result<Box<dyn PlatformMonitor + Send + Sync>, Error> {
    Ok(Box::new(WindowsMonitor {
        state: None,
    }))
}