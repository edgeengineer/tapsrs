//! Foreign Function Interface (FFI) for Transport Services
//! Provides C-compatible bindings for cross-platform interoperability

pub mod connection;
pub mod error;
pub mod listener;
pub mod message;
pub mod path_monitor;
pub mod preconnection;
pub mod runtime;
pub mod security_parameters;
pub mod transport_properties;
pub mod types;

use std::ffi::CString;
use std::os::raw::{c_char, c_void};

/// Initialize the Transport Services library
/// Should be called once before using any other functions
#[no_mangle]
pub extern "C" fn transport_services_init() -> i32 {
    // Initialize logging
    let _ = env_logger::try_init();

    // Initialize the global runtime
    match runtime::init_runtime() {
        Ok(_) => 0,   // Success
        Err(_) => -1, // Error
    }
}

/// Initialize the Transport Services runtime
/// Alternative to transport_services_init for explicit runtime management
#[no_mangle]
pub extern "C" fn transport_services_init_runtime() -> i32 {
    match runtime::init_runtime() {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Cleanup the Transport Services library
#[no_mangle]
pub extern "C" fn transport_services_cleanup() {
    runtime::shutdown_runtime();
}

/// Shutdown the Transport Services runtime
/// Alternative to transport_services_cleanup for explicit runtime management
#[no_mangle]
pub extern "C" fn transport_services_shutdown_runtime() {
    runtime::shutdown_runtime();
}

/// Get the version string of the Transport Services library
#[no_mangle]
pub extern "C" fn transport_services_version() -> *const c_char {
    let version = CString::new(env!("CARGO_PKG_VERSION")).unwrap();
    version.into_raw()
}

/// Free a string returned by the Transport Services library
#[no_mangle]
pub unsafe extern "C" fn transport_services_free_string(s: *mut c_char) {
    if !s.is_null() {
        let _ = CString::from_raw(s);
    }
}

/// Opaque handle type for FFI
#[repr(C)]
pub struct TransportServicesHandle {
    _private: [u8; 0],
}

/// Convert a Rust object to an opaque handle
pub fn to_handle<T>(obj: Box<T>) -> *mut TransportServicesHandle {
    Box::into_raw(obj) as *mut TransportServicesHandle
}

/// Convert an opaque handle back to a Rust object
pub unsafe fn from_handle<T>(handle: *mut TransportServicesHandle) -> Box<T> {
    Box::from_raw(handle as *mut T)
}

/// Get a reference from an opaque handle
pub unsafe fn handle_ref<'a, T>(handle: *const TransportServicesHandle) -> &'a T {
    &*(handle as *const T)
}

/// Get a mutable reference from an opaque handle
pub unsafe fn handle_mut<'a, T>(handle: *mut TransportServicesHandle) -> &'a mut T {
    &mut *(handle as *mut T)
}

// Android-specific FFI functions are defined in path_monitor/android.rs
// and exported with #[no_mangle] so they don't need to be re-exported here
