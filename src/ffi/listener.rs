//! FFI bindings for Listener

use super::*;
use crate::Listener;

/// Stop a listener
#[no_mangle]
pub unsafe extern "C" fn taps_listener_stop(
    handle: *mut TapsHandle,
) -> types::TapsError {
    if handle.is_null() {
        return types::TapsError::InvalidParameters;
    }

    let listener = handle_ref::<Listener>(handle);
    
    // Use tokio runtime to execute async operation
    let rt = tokio::runtime::Runtime::new().unwrap();
    match rt.block_on(listener.stop()) {
        Ok(()) => types::TapsError::Success,
        Err(e) => {
            error::set_last_error(&e);
            types::TapsError::from(e)
        }
    }
}

/// Check if a listener is active
#[no_mangle]
pub unsafe extern "C" fn taps_listener_is_active(
    handle: *mut TapsHandle,
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
pub unsafe extern "C" fn taps_listener_free(handle: *mut TapsHandle) {
    if !handle.is_null() {
        let _ = from_handle::<Listener>(handle);
    }
}