//! FFI bindings for Listener

use super::*;
use crate::Listener;

/// Stop a listener
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