//! FFI bindings for Message

use super::*;
use crate::Message;
use std::slice;

/// Create a new message
#[no_mangle]
pub unsafe extern "C" fn transport_services_message_new(
    data: *const u8,
    length: usize,
) -> *mut TransportServicesHandle {
    if data.is_null() || length == 0 {
        return std::ptr::null_mut();
    }

    let data_slice = slice::from_raw_parts(data, length);
    let message = Message::new(data_slice.to_vec());

    to_handle(Box::new(message))
}

/// Get message data
#[no_mangle]
pub unsafe extern "C" fn transport_services_message_get_data(
    handle: *mut TransportServicesHandle,
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
pub unsafe extern "C" fn transport_services_message_set_priority(
    handle: *mut TransportServicesHandle,
    priority: i32,
) -> types::TransportServicesError {
    if handle.is_null() {
        return types::TransportServicesError::InvalidParameters;
    }

    let message = handle_mut::<Message>(handle);
    message.properties_mut().priority = Some(priority);

    types::TransportServicesError::Success
}

/// Set message lifetime
#[no_mangle]
pub unsafe extern "C" fn transport_services_message_set_lifetime(
    handle: *mut TransportServicesHandle,
    lifetime_ms: u64,
) -> types::TransportServicesError {
    if handle.is_null() {
        return types::TransportServicesError::InvalidParameters;
    }

    let message = handle_mut::<Message>(handle);
    message.properties_mut().lifetime = Some(std::time::Duration::from_millis(lifetime_ms));

    types::TransportServicesError::Success
}

/// Mark message as idempotent
#[no_mangle]
pub unsafe extern "C" fn transport_services_message_set_idempotent(
    handle: *mut TransportServicesHandle,
    idempotent: bool,
) -> types::TransportServicesError {
    if handle.is_null() {
        return types::TransportServicesError::InvalidParameters;
    }

    let message = handle_mut::<Message>(handle);
    message.properties_mut().safely_replayable = idempotent;

    types::TransportServicesError::Success
}

/// Free a message handle
#[no_mangle]
pub unsafe extern "C" fn transport_services_message_free(handle: *mut TransportServicesHandle) {
    if !handle.is_null() {
        let _ = from_handle::<Message>(handle);
    }
}
