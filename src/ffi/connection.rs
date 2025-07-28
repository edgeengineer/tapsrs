//! FFI bindings for Connection

use super::*;
use crate::{Connection, Message};
use std::slice;

/// Get the state of a connection
#[no_mangle]
pub unsafe extern "C" fn taps_connection_get_state(
    handle: *mut TapsHandle,
) -> types::TapsConnectionState {
    if handle.is_null() {
        return types::TapsConnectionState::Closed;
    }

    let conn = handle_ref::<Connection>(handle);
    
    // Use tokio runtime to execute async operation
    let rt = tokio::runtime::Runtime::new().unwrap();
    let state = rt.block_on(conn.state());
    
    state.into()
}

/// Send a message on a connection
#[no_mangle]
pub unsafe extern "C" fn taps_connection_send(
    handle: *mut TapsHandle,
    message: *const types::TapsMessage,
    callback: types::TapsErrorCallback,
    user_data: *mut c_void,
) -> types::TapsError {
    if handle.is_null() || message.is_null() {
        return types::TapsError::InvalidParameters;
    }

    let conn = handle_ref::<Connection>(handle);
    let msg = &*message;
    
    // Create message from FFI data
    if msg.data.is_null() || msg.length == 0 {
        return types::TapsError::InvalidParameters;
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
                    callback(types::TapsError::Success, std::ptr::null(), user_data);
                }
                Err(e) => {
                    error::set_last_error(&e);
                    let error_code = types::TapsError::from(e);
                    callback(error_code, error::taps_get_last_error(), user_data);
                }
            }
        });
    });
    
    types::TapsError::Success
}

/// Close a connection gracefully
#[no_mangle]
pub unsafe extern "C" fn taps_connection_close(
    handle: *mut TapsHandle,
) -> types::TapsError {
    if handle.is_null() {
        return types::TapsError::InvalidParameters;
    }

    let conn = handle_ref::<Connection>(handle);
    
    // Use tokio runtime to execute async operation
    let rt = tokio::runtime::Runtime::new().unwrap();
    match rt.block_on(conn.close()) {
        Ok(()) => types::TapsError::Success,
        Err(e) => {
            error::set_last_error(&e);
            types::TapsError::from(e)
        }
    }
}

/// Abort a connection immediately
#[no_mangle]
pub unsafe extern "C" fn taps_connection_abort(
    handle: *mut TapsHandle,
) -> types::TapsError {
    if handle.is_null() {
        return types::TapsError::InvalidParameters;
    }

    let conn = handle_ref::<Connection>(handle);
    
    // Use tokio runtime to execute async operation
    let rt = tokio::runtime::Runtime::new().unwrap();
    match rt.block_on(conn.abort()) {
        Ok(()) => types::TapsError::Success,
        Err(e) => {
            error::set_last_error(&e);
            types::TapsError::from(e)
        }
    }
}

/// Free a connection handle
#[no_mangle]
pub unsafe extern "C" fn taps_connection_free(handle: *mut TapsHandle) {
    if !handle.is_null() {
        let _ = from_handle::<Connection>(handle);
    }
}