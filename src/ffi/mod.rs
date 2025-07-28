//! Foreign Function Interface (FFI) for Transport Services
//! Provides C-compatible bindings for cross-platform interoperability

pub mod types;
pub mod preconnection;
pub mod connection;
pub mod listener;
pub mod message;
pub mod error;
pub mod transport_properties;
pub mod security_parameters;

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::ptr;

/// Initialize the Transport Services library
/// Should be called once before using any other functions
#[no_mangle]
pub extern "C" fn transport_services_init() -> i32 {
    // Initialize logging, runtime, etc.
    env_logger::init();
    0 // Success
}

/// Cleanup the Transport Services library
#[no_mangle]
pub extern "C" fn transport_services_cleanup() {
    // Cleanup resources
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
pub unsafe fn handle_ref<T>(handle: *const TransportServicesHandle) -> &T {
    &*(handle as *const T)
}

/// Get a mutable reference from an opaque handle
pub unsafe fn handle_mut<T>(handle: *mut TransportServicesHandle) -> &mut T {
    &mut *(handle as *mut T)
}