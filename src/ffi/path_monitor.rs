//! FFI bindings for Network Path Monitoring
//!
//! Provides C-compatible bindings for cross-platform network interface monitoring

use super::*;
use crate::path_monitor::{ChangeEvent, Interface, NetworkMonitor, Status};
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::{Arc, Mutex};

/// FFI representation of network interface status
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum TransportServicesInterfaceStatus {
    Up = 0,
    Down = 1,
    Unknown = 2,
}

impl From<Status> for TransportServicesInterfaceStatus {
    fn from(status: Status) -> Self {
        match status {
            Status::Up => TransportServicesInterfaceStatus::Up,
            Status::Down => TransportServicesInterfaceStatus::Down,
            Status::Unknown => TransportServicesInterfaceStatus::Unknown,
        }
    }
}

/// FFI representation of a network interface
#[repr(C)]
pub struct TransportServicesInterface {
    pub name: *mut c_char,
    pub index: u32,
    pub ips: *mut *mut c_char,
    pub ip_count: usize,
    pub status: TransportServicesInterfaceStatus,
    pub interface_type: *mut c_char,
    pub is_expensive: bool,
}

/// FFI representation of change event type
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum TransportServicesChangeEventType {
    Added = 0,
    Removed = 1,
    Modified = 2,
    PathChanged = 3,
}

/// FFI representation of a change event
#[repr(C)]
pub struct TransportServicesChangeEvent {
    pub event_type: TransportServicesChangeEventType,
    pub interface: *mut TransportServicesInterface,
    pub old_interface: *mut TransportServicesInterface, // For Modified events
    pub description: *mut c_char,                       // For PathChanged events
}

/// Callback type for network change events
pub type TransportServicesPathMonitorCallback =
    extern "C" fn(*const TransportServicesChangeEvent, *mut c_void);

/// Opaque handle for the monitor watcher
pub struct PathMonitorHandle {
    _handle: crate::path_monitor::MonitorHandle,
    _callback_data: Arc<Mutex<CallbackData>>,
}

struct CallbackData {
    callback: TransportServicesPathMonitorCallback,
    user_data: *mut c_void,
}

unsafe impl Send for CallbackData {}
unsafe impl Sync for CallbackData {}

/// Create a new network path monitor
#[no_mangle]
pub extern "C" fn transport_services_path_monitor_create() -> *mut TransportServicesHandle {
    match NetworkMonitor::new() {
        Ok(monitor) => to_handle(Box::new(monitor)),
        Err(e) => {
            error::set_last_error_string(&format!("Failed to create network monitor: {}", e));
            std::ptr::null_mut()
        }
    }
}

/// Destroy a network path monitor
#[no_mangle]
pub unsafe extern "C" fn transport_services_path_monitor_destroy(
    handle: *mut TransportServicesHandle,
) {
    if !handle.is_null() {
        let _ = from_handle::<NetworkMonitor>(handle);
    }
}

/// List all network interfaces
#[no_mangle]
pub unsafe extern "C" fn transport_services_path_monitor_list_interfaces(
    handle: *mut TransportServicesHandle,
    interfaces: *mut *mut TransportServicesInterface,
    count: *mut usize,
) -> c_int {
    if handle.is_null() || interfaces.is_null() || count.is_null() {
        return -1;
    }

    let monitor = handle_ref::<NetworkMonitor>(handle);

    match monitor.list_interfaces() {
        Ok(ifaces) => {
            let iface_count = ifaces.len();
            *count = iface_count;

            if iface_count == 0 {
                *interfaces = std::ptr::null_mut();
                return 0;
            }

            // Allocate array of interface pointers
            let iface_array = libc::calloc(
                iface_count,
                std::mem::size_of::<*mut TransportServicesInterface>(),
            ) as *mut *mut TransportServicesInterface;

            if iface_array.is_null() {
                return -1;
            }

            // Convert each interface
            for (i, iface) in ifaces.into_iter().enumerate() {
                let ffi_iface = interface_to_ffi(iface);
                *iface_array.add(i) = ffi_iface;
            }

            *interfaces = iface_array;
            0
        }
        Err(e) => {
            error::set_last_error_string(&format!("Failed to list interfaces: {}", e));
            -1
        }
    }
}

/// Free an array of interfaces returned by list_interfaces
#[no_mangle]
pub unsafe extern "C" fn transport_services_path_monitor_free_interfaces(
    interfaces: *mut *mut TransportServicesInterface,
    count: usize,
) {
    if interfaces.is_null() || count == 0 {
        return;
    }

    // Free each interface
    for i in 0..count {
        let iface_ptr = *interfaces.add(i);
        if !iface_ptr.is_null() {
            free_ffi_interface(iface_ptr);
        }
    }

    // Free the array itself
    libc::free(interfaces as *mut c_void);
}

/// Start watching for network changes
#[no_mangle]
pub unsafe extern "C" fn transport_services_path_monitor_start_watching(
    handle: *mut TransportServicesHandle,
    callback: TransportServicesPathMonitorCallback,
    user_data: *mut c_void,
) -> *mut TransportServicesHandle {
    if handle.is_null() {
        return std::ptr::null_mut();
    }

    let monitor = handle_ref::<NetworkMonitor>(handle);

    let callback_data = Arc::new(Mutex::new(CallbackData {
        callback,
        user_data,
    }));

    let callback_data_clone = callback_data.clone();

    let monitor_handle = monitor.watch_changes(move |event| {
        let data = callback_data_clone.lock().unwrap();
        let ffi_event = change_event_to_ffi(event);
        (data.callback)(&ffi_event, data.user_data);
        free_ffi_change_event(ffi_event);
    });

    to_handle(Box::new(PathMonitorHandle {
        _handle: monitor_handle,
        _callback_data: callback_data,
    }))
}

/// Stop watching for network changes
#[no_mangle]
pub unsafe extern "C" fn transport_services_path_monitor_stop_watching(
    handle: *mut TransportServicesHandle,
) {
    if !handle.is_null() {
        let _ = from_handle::<PathMonitorHandle>(handle);
    }
}

// Helper functions

unsafe fn interface_to_ffi(iface: Interface) -> *mut TransportServicesInterface {
    let ffi_iface = Box::new(TransportServicesInterface {
        name: CString::new(iface.name).unwrap().into_raw(),
        index: iface.index,
        ips: std::ptr::null_mut(),
        ip_count: 0,
        status: iface.status.into(),
        interface_type: CString::new(iface.interface_type).unwrap().into_raw(),
        is_expensive: iface.is_expensive,
    });

    // Convert IP addresses
    if !iface.ips.is_empty() {
        let ip_count = iface.ips.len();
        let ip_array =
            libc::calloc(ip_count, std::mem::size_of::<*mut c_char>()) as *mut *mut c_char;

        if !ip_array.is_null() {
            for (i, ip) in iface.ips.iter().enumerate() {
                let ip_str = CString::new(ip.to_string()).unwrap();
                *ip_array.add(i) = ip_str.into_raw();
            }

            let mut boxed_iface = ffi_iface;
            boxed_iface.ips = ip_array;
            boxed_iface.ip_count = ip_count;
            return Box::into_raw(boxed_iface);
        }
    }

    Box::into_raw(ffi_iface)
}

unsafe fn free_ffi_interface(iface: *mut TransportServicesInterface) {
    if iface.is_null() {
        return;
    }

    let iface = &*iface;

    // Free name
    if !iface.name.is_null() {
        let _ = CString::from_raw(iface.name);
    }

    // Free interface type
    if !iface.interface_type.is_null() {
        let _ = CString::from_raw(iface.interface_type);
    }

    // Free IP addresses
    if !iface.ips.is_null() && iface.ip_count > 0 {
        for i in 0..iface.ip_count {
            let ip_ptr = *iface.ips.add(i);
            if !ip_ptr.is_null() {
                let _ = CString::from_raw(ip_ptr);
            }
        }
        libc::free(iface.ips as *mut c_void);
    }

    // Free the interface struct itself
    let _ = Box::from_raw(iface as *mut TransportServicesInterface);
}

fn change_event_to_ffi(event: ChangeEvent) -> TransportServicesChangeEvent {
    unsafe {
        match event {
            ChangeEvent::Added(iface) => TransportServicesChangeEvent {
                event_type: TransportServicesChangeEventType::Added,
                interface: interface_to_ffi(iface),
                old_interface: std::ptr::null_mut(),
                description: std::ptr::null_mut(),
            },
            ChangeEvent::Removed(iface) => TransportServicesChangeEvent {
                event_type: TransportServicesChangeEventType::Removed,
                interface: interface_to_ffi(iface),
                old_interface: std::ptr::null_mut(),
                description: std::ptr::null_mut(),
            },
            ChangeEvent::Modified { old, new } => TransportServicesChangeEvent {
                event_type: TransportServicesChangeEventType::Modified,
                interface: interface_to_ffi(new),
                old_interface: interface_to_ffi(old),
                description: std::ptr::null_mut(),
            },
            ChangeEvent::PathChanged { description } => TransportServicesChangeEvent {
                event_type: TransportServicesChangeEventType::PathChanged,
                interface: std::ptr::null_mut(),
                old_interface: std::ptr::null_mut(),
                description: CString::new(description).unwrap().into_raw(),
            },
        }
    }
}

unsafe fn free_ffi_change_event(event: TransportServicesChangeEvent) {
    if !event.interface.is_null() {
        free_ffi_interface(event.interface);
    }
    if !event.old_interface.is_null() {
        free_ffi_interface(event.old_interface);
    }
    if !event.description.is_null() {
        let _ = CString::from_raw(event.description);
    }
}

