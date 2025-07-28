//! FFI bindings for Preconnection

use super::*;
use crate::{Preconnection, LocalEndpoint, RemoteEndpoint, TransportProperties, SecurityParameters};
use std::ffi::CStr;
use std::os::raw::c_char;

/// Create a new preconnection
#[no_mangle]
pub extern "C" fn transport_services_preconnection_new() -> *mut TransportServicesHandle {
    let preconn = Preconnection::new(
        vec![],
        vec![],
        TransportProperties::default(),
        SecurityParameters::default(),
    );
    to_handle(Box::new(preconn))
}

/// Add a local endpoint to the preconnection
#[no_mangle]
pub unsafe extern "C" fn transport_services_preconnection_add_local_endpoint(
    handle: *mut TransportServicesHandle,
    endpoint: *const types::TransportServicesEndpoint,
) -> types::TransportServicesError {
    if handle.is_null() || endpoint.is_null() {
        return types::TransportServicesError::InvalidParameters;
    }

    let preconn = handle_mut::<Preconnection>(handle);
    let endpoint = &*endpoint;
    
    let mut local = LocalEndpoint::default();
    
    // Add hostname if provided
    if !endpoint.hostname.is_null() {
        if let Ok(hostname) = CStr::from_ptr(endpoint.hostname).to_str() {
            local.identifiers.push(crate::EndpointIdentifier::HostName(hostname.to_string()));
        }
    }
    
    // Add port if non-zero
    if endpoint.port != 0 {
        local.identifiers.push(crate::EndpointIdentifier::Port(endpoint.port));
    }
    
    // Add interface if provided
    if !endpoint.interface.is_null() {
        if let Ok(interface) = CStr::from_ptr(endpoint.interface).to_str() {
            local.identifiers.push(crate::EndpointIdentifier::Interface(interface.to_string()));
        }
    }
    
    // Use tokio runtime to execute async operation
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(preconn.add_local(local));
    
    types::TransportServicesError::Success
}

/// Add a remote endpoint to the preconnection
#[no_mangle]
pub unsafe extern "C" fn transport_services_preconnection_add_remote_endpoint(
    handle: *mut TransportServicesHandle,
    endpoint: *const types::TransportServicesEndpoint,
) -> types::TransportServicesError {
    if handle.is_null() || endpoint.is_null() {
        return types::TransportServicesError::InvalidParameters;
    }

    let preconn = handle_mut::<Preconnection>(handle);
    let endpoint = &*endpoint;
    
    let mut remote = RemoteEndpoint::default();
    
    // Add hostname if provided
    if !endpoint.hostname.is_null() {
        if let Ok(hostname) = CStr::from_ptr(endpoint.hostname).to_str() {
            remote.identifiers.push(crate::EndpointIdentifier::HostName(hostname.to_string()));
        }
    }
    
    // Add port if non-zero
    if endpoint.port != 0 {
        remote.identifiers.push(crate::EndpointIdentifier::Port(endpoint.port));
    }
    
    // Add service if provided
    if !endpoint.service.is_null() {
        if let Ok(service) = CStr::from_ptr(endpoint.service).to_str() {
            remote.identifiers.push(crate::EndpointIdentifier::Service(service.to_string()));
        }
    }
    
    // Use tokio runtime to execute async operation
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(preconn.add_remote(remote));
    
    types::TransportServicesError::Success
}

/// Set transport properties on the preconnection
#[no_mangle]
pub unsafe extern "C" fn transport_services_preconnection_set_transport_properties(
    handle: *mut TransportServicesHandle,
    properties: *const types::TransportServicesProperties,
) -> types::TransportServicesError {
    if handle.is_null() || properties.is_null() {
        return types::TransportServicesError::InvalidParameters;
    }

    let preconn = handle_mut::<Preconnection>(handle);
    let props = &*properties;
    
    let mut transport_props = TransportProperties::default();
    transport_props.selection_properties.reliability = props.reliability.into();
    transport_props.selection_properties.preserve_msg_boundaries = props.preserve_msg_boundaries.into();
    transport_props.selection_properties.preserve_order = props.preserve_order.into();
    transport_props.selection_properties.congestion_control = props.congestion_control.into();
    
    // Use tokio runtime to execute async operation
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(preconn.set_transport_properties(transport_props));
    
    types::TransportServicesError::Success
}

/// Initiate a connection
#[no_mangle]
pub unsafe extern "C" fn transport_services_preconnection_initiate(
    handle: *mut TransportServicesHandle,
    callback: types::TransportServicesConnectionCallback,
    error_callback: types::TransportServicesErrorCallback,
    user_data: *mut c_void,
) -> types::TransportServicesError {
    if handle.is_null() {
        return types::TransportServicesError::InvalidParameters;
    }

    let preconn = handle_ref::<Preconnection>(handle);
    
    // Clone for moving into async task
    let preconn_clone = preconn.clone();
    
    // Spawn async task to handle connection
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            match preconn_clone.initiate().await {
                Ok(connection) => {
                    let conn_handle = to_handle(Box::new(connection));
                    callback(conn_handle, user_data);
                }
                Err(e) => {
                    let error_code = types::TransportServicesError::from(e);
                    let msg = CString::new("Connection initiation failed").unwrap();
                    error_callback(error_code, msg.as_ptr(), user_data);
                }
            }
        });
    });
    
    types::TransportServicesError::Success
}

/// Free a preconnection handle
#[no_mangle]
pub unsafe extern "C" fn transport_services_preconnection_free(handle: *mut TransportServicesHandle) {
    if !handle.is_null() {
        let _ = from_handle::<Preconnection>(handle);
    }
}