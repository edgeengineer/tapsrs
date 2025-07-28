//! FFI bindings for Message

use super::*;
use crate::Message;
use std::slice;

/// Create a new message
#[no_mangle]
pub unsafe extern "C" fn taps_message_new(
    data: *const u8,
    length: usize,
) -> *mut TapsHandle {
    if data.is_null() || length == 0 {
        return std::ptr::null_mut();
    }
    
    let data_slice = slice::from_raw_parts(data, length);
    let message = Message::new(data_slice.to_vec());
    
    to_handle(Box::new(message))
}

/// Get message data
#[no_mangle]
pub unsafe extern "C" fn taps_message_get_data(
    handle: *mut TapsHandle,
    length: *mut usize,
) -> *const u8 {
    if handle.is_null() || length.is_null() {
        return std::ptr::null();
    }
    
    let message = handle_ref::<Message>(handle);
    let data = message.data();
    
    *length = data.len();
    data.as_ptr()
}

/// Set message priority
#[no_mangle]
pub unsafe extern "C" fn taps_message_set_priority(
    handle: *mut TapsHandle,
    priority: i32,
) -> types::TapsError {
    if handle.is_null() {
        return types::TapsError::InvalidParameters;
    }
    
    let message = handle_mut::<Message>(handle);
    message.properties_mut().priority = Some(priority);
    
    types::TapsError::Success
}

/// Set message lifetime
#[no_mangle]
pub unsafe extern "C" fn taps_message_set_lifetime(
    handle: *mut TapsHandle,
    lifetime_ms: u64,
) -> types::TapsError {
    if handle.is_null() {
        return types::TapsError::InvalidParameters;
    }
    
    let message = handle_mut::<Message>(handle);
    message.properties_mut().lifetime = Some(std::time::Duration::from_millis(lifetime_ms));
    
    types::TapsError::Success
}

/// Mark message as idempotent
#[no_mangle]
pub unsafe extern "C" fn taps_message_set_idempotent(
    handle: *mut TapsHandle,
    idempotent: bool,
) -> types::TapsError {
    if handle.is_null() {
        return types::TapsError::InvalidParameters;
    }
    
    let message = handle_mut::<Message>(handle);
    message.properties_mut().idempotent = idempotent;
    
    types::TapsError::Success
}

/// Free a message handle
#[no_mangle]
pub unsafe extern "C" fn taps_message_free(handle: *mut TapsHandle) {
    if !handle.is_null() {
        let _ = from_handle::<Message>(handle);
    }
}