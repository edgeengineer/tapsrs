//! FFI error handling utilities

use std::ffi::CString;
use std::os::raw::c_char;

thread_local! {
    static LAST_ERROR: std::cell::RefCell<Option<CString>> = std::cell::RefCell::new(None);
}

/// Set the last error message
pub fn set_last_error(err: &crate::TransportServicesError) {
    let msg = format!("{}", err);
    let c_msg = CString::new(msg).unwrap_or_else(|_| CString::new("Unknown error").unwrap());
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = Some(c_msg);
    });
}

/// Set the last error message from a string
pub fn set_last_error_string(err: &str) {
    let c_msg = CString::new(err).unwrap_or_else(|_| CString::new("Unknown error").unwrap());
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = Some(c_msg);
    });
}

/// Get the last error message
#[no_mangle]
pub extern "C" fn transport_services_get_last_error() -> *const c_char {
    LAST_ERROR.with(|e| {
        e.borrow()
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(std::ptr::null())
    })
}

/// Clear the last error message
#[no_mangle]
pub extern "C" fn transport_services_clear_last_error() {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = None;
    });
}
