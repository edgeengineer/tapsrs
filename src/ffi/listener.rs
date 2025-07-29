//! FFI bindings for Listener

use super::*;
use crate::{Listener, ListenerEvent};
use std::ffi::CString;

/// Set callbacks for a listener to handle incoming connections
#[no_mangle]
pub unsafe extern "C" fn transport_services_listener_set_callbacks(
    handle: *mut TransportServicesHandle,
    connection_received_callback: types::TransportServicesConnectionCallback,
    error_callback: types::TransportServicesErrorCallback,
    user_data: *mut c_void,
) -> types::TransportServicesError {
    if handle.is_null() {
        return types::TransportServicesError::InvalidParameters;
    }

    let listener = handle_ref::<Listener>(handle);
    let listener_clone = listener.clone();

    // Wrap user_data in a type that is Send
    struct CallbackData {
        connection_received_callback: types::TransportServicesConnectionCallback,
        error_callback: types::TransportServicesErrorCallback,
        user_data: usize,
    }

    let callback_data = CallbackData {
        connection_received_callback,
        error_callback,
        user_data: user_data as usize,
    };

    // Spawn async task to handle listener events using the global runtime
    match runtime::spawn(async move {
        // Start receiving events in a loop
        loop {
            match listener_clone.next_event().await {
                Some(ListenerEvent::ConnectionReceived(connection)) => {
                    // Convert connection to handle
                    let conn_handle = to_handle(Box::new(connection));

                    // Call the connection received callback
                    (callback_data.connection_received_callback)(
                        conn_handle,
                        callback_data.user_data as *mut c_void,
                    );
                }
                Some(ListenerEvent::Error(ref error_msg)) => {
                    let c_msg = CString::new(error_msg.as_str())
                        .unwrap_or_else(|_| CString::new("").unwrap());
                    (callback_data.error_callback)(
                        types::TransportServicesError::EstablishmentFailed,
                        c_msg.as_ptr(),
                        callback_data.user_data as *mut c_void,
                    );
                }
                Some(ListenerEvent::Stopped) => {
                    // Listener stopped, exit the loop
                    break;
                }
                None => {
                    // No more events
                    break;
                }
            }
        }
    }) {
        Ok(_) => types::TransportServicesError::Success,
        Err(e) => {
            error::set_last_error_string(&e);
            types::TransportServicesError::RuntimeError
        }
    }
}

/// Stop a listener asynchronously
#[no_mangle]
pub unsafe extern "C" fn transport_services_listener_stop_async(
    handle: *mut TransportServicesHandle,
    callback: types::TransportServicesCompletionCallback,
    user_data: *mut c_void,
) -> types::TransportServicesError {
    if handle.is_null() {
        return types::TransportServicesError::InvalidParameters;
    }

    let listener = handle_ref::<Listener>(handle);
    let listener_clone = listener.clone();

    // Wrap user_data in a type that is Send
    struct CallbackData {
        callback: types::TransportServicesCompletionCallback,
        user_data: usize,
    }

    let callback_data = CallbackData {
        callback,
        user_data: user_data as usize,
    };

    // Spawn async task to handle stop using the global runtime
    match runtime::spawn(async move {
        match listener_clone.stop().await {
            Ok(()) => {
                (callback_data.callback)(
                    types::TransportServicesError::Success,
                    callback_data.user_data as *mut c_void,
                );
            }
            Err(e) => {
                error::set_last_error(&e);
                let error_code = types::TransportServicesError::from(e);
                (callback_data.callback)(error_code, callback_data.user_data as *mut c_void);
            }
        }
    }) {
        Ok(_) => types::TransportServicesError::Success,
        Err(e) => {
            error::set_last_error_string(&e);
            types::TransportServicesError::RuntimeError
        }
    }
}

/// Stop a listener (blocking version for backward compatibility)
#[no_mangle]
pub unsafe extern "C" fn transport_services_listener_stop(
    handle: *mut TransportServicesHandle,
) -> types::TransportServicesError {
    if handle.is_null() {
        return types::TransportServicesError::InvalidParameters;
    }

    let listener = handle_ref::<Listener>(handle);

    // Use tokio runtime to execute async operation
    let rt = tokio::runtime::Runtime::new().unwrap();
    match rt.block_on(listener.stop()) {
        Ok(()) => types::TransportServicesError::Success,
        Err(e) => {
            error::set_last_error(&e);
            types::TransportServicesError::from(e)
        }
    }
}

/// Check if a listener is active
#[no_mangle]
pub unsafe extern "C" fn transport_services_listener_is_active(
    handle: *mut TransportServicesHandle,
) -> bool {
    if handle.is_null() {
        return false;
    }

    let listener = handle_ref::<Listener>(handle);

    // Use tokio runtime to execute async operation
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(listener.is_active())
}

/// Free a listener handle
#[no_mangle]
pub unsafe extern "C" fn transport_services_listener_free(handle: *mut TransportServicesHandle) {
    if !handle.is_null() {
        let _ = from_handle::<Listener>(handle);
    }
}
