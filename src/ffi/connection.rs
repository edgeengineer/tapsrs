//! FFI bindings for Connection

use super::*;
use crate::{Connection, Message, ConnectionEvent};
use std::slice;
use std::ffi::CString;

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
        rust_msg = rust_msg.idempotent();
    }
    if msg.final_message {
        rust_msg = rust_msg.final_message();
    }
    
    // Clone for moving into async task
    let conn_clone = conn.clone();
    
    // Spawn async task to handle send
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            match conn_clone.send(rust_msg).await {
                Ok(()) => {
                    callback(types::TransportServicesError::Success, std::ptr::null(), user_data);
                }
                Err(e) => {
                    error::set_last_error(&e);
                    let error_code = types::TransportServicesError::from(e);
                    callback(error_code, error::transport_services_get_last_error(), user_data);
                }
            }
        });
    });
    
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

/// Connection event types for FFI
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum TransportServicesConnectionEventType {
    Ready = 0,
    EstablishmentError = 1,
    ConnectionError = 2,
    PathChange = 3,
    SoftError = 4,
    Closed = 5,
}

/// FFI callback for connection events
pub type TransportServicesConnectionEventCallback = extern "C" fn(
    connection: *mut TransportServicesHandle,
    event_type: TransportServicesConnectionEventType,
    message: *const c_char,
    user_data: *mut c_void,
);

/// Poll for the next event on a connection (non-blocking)
#[no_mangle]
pub unsafe extern "C" fn transport_services_connection_poll_event(
    handle: *mut TransportServicesHandle,
    event_type: *mut TransportServicesConnectionEventType,
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
                ConnectionEvent::Ready => (TransportServicesConnectionEventType::Ready, "Connection established"),
                ConnectionEvent::EstablishmentError(ref m) => {
                    (TransportServicesConnectionEventType::EstablishmentError, m.as_str())
                }
                ConnectionEvent::ConnectionError(ref m) => {
                    (TransportServicesConnectionEventType::ConnectionError, m.as_str())
                }
                ConnectionEvent::PathChange => {
                    (TransportServicesConnectionEventType::PathChange, "Path changed")
                }
                ConnectionEvent::SoftError(ref m) => {
                    (TransportServicesConnectionEventType::SoftError, m.as_str())
                }
                ConnectionEvent::Closed => {
                    (TransportServicesConnectionEventType::Closed, "Connection closed")
                }
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