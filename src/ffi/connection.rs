//! FFI bindings for Connection

use super::*;
use crate::{Connection, ConnectionEvent, Message};
use std::os::raw::c_int;
use std::slice;

/// Get the state of a connection
#[no_mangle]
pub unsafe extern "C" fn transport_services_connection_get_state(
    handle: *mut TransportServicesHandle,
) -> types::TransportServicesConnectionState {
    if handle.is_null() {
        return types::TransportServicesConnectionState::Closed;
    }

    let conn = handle_ref::<Connection>(handle);

    // Use tokio runtime to execute async operation
    let rt = tokio::runtime::Runtime::new().unwrap();
    let state = rt.block_on(conn.state());

    state.into()
}

/// Send a message on a connection
#[no_mangle]
pub unsafe extern "C" fn transport_services_connection_send(
    handle: *mut TransportServicesHandle,
    message: *const types::TransportServicesMessage,
    callback: types::TransportServicesErrorCallback,
    user_data: *mut c_void,
) -> types::TransportServicesError {
    if handle.is_null() || message.is_null() {
        return types::TransportServicesError::InvalidParameters;
    }

    let conn = handle_ref::<Connection>(handle);
    let msg = &*message;

    // Create message from FFI data
    if msg.data.is_null() || msg.length == 0 {
        return types::TransportServicesError::InvalidParameters;
    }

    let data = slice::from_raw_parts(msg.data, msg.length).to_vec();
    let mut rust_msg = Message::new(data);

    // Set message properties
    if msg.lifetime_ms > 0 {
        rust_msg = rust_msg.with_lifetime(std::time::Duration::from_millis(msg.lifetime_ms));
    }
    if msg.priority != 0 {
        rust_msg = rust_msg.with_priority(msg.priority);
    }
    if msg.idempotent {
        rust_msg = rust_msg.safely_replayable();
    }
    if msg.final_message {
        rust_msg = rust_msg.final_message();
    }

    // Clone for moving into async task
    let conn_clone = conn.clone();

    // Wrap user_data in a type that is Send
    struct CallbackData {
        callback: types::TransportServicesErrorCallback,
        user_data: usize,
    }

    let callback_data = CallbackData {
        callback,
        user_data: user_data as usize,
    };

    // Spawn async task to handle send using the global runtime
    match runtime::spawn(async move {
        match conn_clone.send(rust_msg).await {
            Ok(()) => {
                (callback_data.callback)(
                    types::TransportServicesError::Success,
                    std::ptr::null(),
                    callback_data.user_data as *mut c_void,
                );
            }
            Err(e) => {
                error::set_last_error(&e);
                let error_code = types::TransportServicesError::from(e);
                (callback_data.callback)(
                    error_code,
                    error::transport_services_get_last_error(),
                    callback_data.user_data as *mut c_void,
                );
            }
        }
    }) {
        Ok(_) => {}
        Err(e) => {
            error::set_last_error_string(&e);
            return types::TransportServicesError::RuntimeError;
        }
    }

    types::TransportServicesError::Success
}

/// Close a connection gracefully
#[no_mangle]
pub unsafe extern "C" fn transport_services_connection_close(
    handle: *mut TransportServicesHandle,
) -> types::TransportServicesError {
    if handle.is_null() {
        return types::TransportServicesError::InvalidParameters;
    }

    let conn = handle_ref::<Connection>(handle);

    // Use tokio runtime to execute async operation
    let rt = tokio::runtime::Runtime::new().unwrap();
    match rt.block_on(conn.close()) {
        Ok(()) => types::TransportServicesError::Success,
        Err(e) => {
            error::set_last_error(&e);
            types::TransportServicesError::from(e)
        }
    }
}

/// Abort a connection immediately
#[no_mangle]
pub unsafe extern "C" fn transport_services_connection_abort(
    handle: *mut TransportServicesHandle,
) -> types::TransportServicesError {
    if handle.is_null() {
        return types::TransportServicesError::InvalidParameters;
    }

    let conn = handle_ref::<Connection>(handle);

    // Use tokio runtime to execute async operation
    let rt = tokio::runtime::Runtime::new().unwrap();
    match rt.block_on(conn.abort()) {
        Ok(()) => types::TransportServicesError::Success,
        Err(e) => {
            error::set_last_error(&e);
            types::TransportServicesError::from(e)
        }
    }
}

/// Free a connection handle
#[no_mangle]
pub unsafe extern "C" fn transport_services_connection_free(handle: *mut TransportServicesHandle) {
    if !handle.is_null() {
        let _ = from_handle::<Connection>(handle);
    }
}

/// FFI callback for connection events
pub type TransportServicesConnectionEventCallback = extern "C" fn(
    connection: *mut TransportServicesHandle,
    event_type: types::TransportServicesConnectionEventType,
    message: *const c_char,
    user_data: *mut c_void,
);

/// Poll for the next event on a connection (non-blocking)
#[no_mangle]
pub unsafe extern "C" fn transport_services_connection_poll_event(
    handle: *mut TransportServicesHandle,
    event_type: *mut types::TransportServicesConnectionEventType,
    message_buffer: *mut c_char,
    message_buffer_size: usize,
) -> c_int {
    if handle.is_null() || event_type.is_null() || message_buffer.is_null() {
        return -1;
    }

    let conn = handle_ref::<Connection>(handle);

    // Try to get the next event without blocking
    let rt = tokio::runtime::Runtime::new().unwrap();
    match rt.block_on(async {
        // Use try_recv equivalent by checking with timeout
        tokio::time::timeout(std::time::Duration::from_millis(0), conn.next_event()).await
    }) {
        Ok(Some(event)) => {
            let (evt_type, msg) = match event {
                ConnectionEvent::Ready => (
                    types::TransportServicesConnectionEventType::Ready,
                    "Connection established",
                ),
                ConnectionEvent::EstablishmentError(ref m) => (
                    types::TransportServicesConnectionEventType::EstablishmentError,
                    m.as_str(),
                ),
                ConnectionEvent::ConnectionError(ref m) => (
                    types::TransportServicesConnectionEventType::ConnectionError,
                    m.as_str(),
                ),
                ConnectionEvent::PathChange => (
                    types::TransportServicesConnectionEventType::PathChange,
                    "Path changed",
                ),
                ConnectionEvent::SoftError(ref m) => (
                    types::TransportServicesConnectionEventType::SoftError,
                    m.as_str(),
                ),
                ConnectionEvent::Closed => (
                    types::TransportServicesConnectionEventType::Closed,
                    "Connection closed",
                ),
                ConnectionEvent::Sent { .. } => (
                    types::TransportServicesConnectionEventType::Sent,
                    "Message sent",
                ),
                ConnectionEvent::Expired { .. } => (
                    types::TransportServicesConnectionEventType::Expired,
                    "Message expired",
                ),
                ConnectionEvent::SendError { .. } => (
                    types::TransportServicesConnectionEventType::SendError,
                    "Send error",
                ),
                ConnectionEvent::Received { .. } => (
                    types::TransportServicesConnectionEventType::Received,
                    "Message received",
                ),
                ConnectionEvent::ReceivedPartial { .. } => (
                    types::TransportServicesConnectionEventType::ReceivedPartial,
                    "Partial message received",
                ),
                ConnectionEvent::ReceiveError { ref error, .. } => (
                    types::TransportServicesConnectionEventType::Received,
                    error.as_str(),
                ),
            };

            *event_type = evt_type;

            // Copy message to buffer
            let msg_bytes = msg.as_bytes();
            let copy_len = std::cmp::min(msg_bytes.len(), message_buffer_size - 1);
            std::ptr::copy_nonoverlapping(msg_bytes.as_ptr(), message_buffer as *mut u8, copy_len);
            *message_buffer.add(copy_len) = 0; // Null terminate

            0 // Success
        }
        _ => 1, // No event available
    }
}
