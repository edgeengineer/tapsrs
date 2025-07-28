//! FFI bindings for SecurityParameters

use super::*;
use crate::{SecurityParameters, SecurityParameter, SecurityParameterValue, SecurityProtocol, Certificate, CertificateChain, PreSharedKey};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::slice;

/// FFI callback type for trust verification
/// Returns 1 for trusted, 0 for not trusted
pub type TransportServicesTrustVerificationCallback = extern "C" fn(
    certificate_chain_data: *const u8,
    certificate_chain_len: usize,
    user_data: *mut c_void,
) -> c_int;

/// FFI callback type for identity challenge
/// The callback should write the response to the provided buffer
/// Returns the number of bytes written, or -1 on error
pub type TransportServicesIdentityChallengeCallback = extern "C" fn(
    challenge_data: *const u8,
    challenge_len: usize,
    response_buffer: *mut u8,
    response_buffer_len: usize,
    user_data: *mut c_void,
) -> c_int;

/// FFI representation of security parameters with callbacks
#[repr(C)]
pub struct TransportServicesSecurityParamsFFI {
    pub handle: *mut TransportServicesHandle,
    pub trust_verification_callback: Option<TransportServicesTrustVerificationCallback>,
    pub trust_verification_user_data: *mut c_void,
    pub identity_challenge_callback: Option<TransportServicesIdentityChallengeCallback>,
    pub identity_challenge_user_data: *mut c_void,
}

/// Create a new SecurityParameters object
#[no_mangle]
pub extern "C" fn transport_services_new_security_parameters() -> *mut TransportServicesHandle {
    let params = Box::new(SecurityParameters::new());
    to_handle(params)
}

/// Create disabled security parameters
#[no_mangle]
pub extern "C" fn transport_services_new_disabled_security_parameters() -> *mut TransportServicesHandle {
    let params = Box::new(SecurityParameters::new_disabled());
    to_handle(params)
}

/// Create opportunistic security parameters
#[no_mangle]
pub extern "C" fn transport_services_new_opportunistic_security_parameters() -> *mut TransportServicesHandle {
    let params = Box::new(SecurityParameters::new_opportunistic());
    to_handle(params)
}

/// Free a SecurityParameters object
#[no_mangle]
pub unsafe extern "C" fn transport_services_free_security_parameters(handle: *mut TransportServicesHandle) {
    if !handle.is_null() {
        let _ = from_handle::<SecurityParameters>(handle);
    }
}

/// Set allowed security protocols
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_allowed_protocols(
    handle: *mut TransportServicesHandle,
    protocols: *const c_int,
    count: usize,
) -> c_int {
    if handle.is_null() || protocols.is_null() {
        return -1;
    }
    
    let params = handle_mut::<SecurityParameters>(handle);
    let protocols_slice = slice::from_raw_parts(protocols, count);
    
    let mut allowed_protocols = Vec::new();
    for &proto in protocols_slice {
        let protocol = match proto {
            0 => SecurityProtocol::TLS12,
            1 => SecurityProtocol::TLS13,
            2 => SecurityProtocol::DTLS12,
            3 => SecurityProtocol::DTLS13,
            _ => continue,
        };
        allowed_protocols.push(protocol);
    }
    
    params.set(SecurityParameter::AllowedProtocols, SecurityParameterValue::Protocols(allowed_protocols));
    0
}

/// Set ALPN protocols
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_alpn(
    handle: *mut TransportServicesHandle,
    protocols: *const *const c_char,
    count: usize,
) -> c_int {
    if handle.is_null() || protocols.is_null() {
        return -1;
    }
    
    let params = handle_mut::<SecurityParameters>(handle);
    let protocols_slice = slice::from_raw_parts(protocols, count);
    
    let mut alpn_protocols = Vec::new();
    for &proto_ptr in protocols_slice {
        if !proto_ptr.is_null() {
            match CStr::from_ptr(proto_ptr).to_str() {
                Ok(s) => alpn_protocols.push(s.to_string()),
                Err(_) => return -1,
            }
        }
    }
    
    params.set(SecurityParameter::Alpn, SecurityParameterValue::Strings(alpn_protocols));
    0
}

/// Set ciphersuites
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_ciphersuites(
    handle: *mut TransportServicesHandle,
    ciphersuites: *const *const c_char,
    count: usize,
) -> c_int {
    if handle.is_null() || ciphersuites.is_null() {
        return -1;
    }
    
    let params = handle_mut::<SecurityParameters>(handle);
    let ciphersuites_slice = slice::from_raw_parts(ciphersuites, count);
    
    let mut suite_list = Vec::new();
    for &suite_ptr in ciphersuites_slice {
        if !suite_ptr.is_null() {
            match CStr::from_ptr(suite_ptr).to_str() {
                Ok(s) => suite_list.push(s.to_string()),
                Err(_) => return -1,
            }
        }
    }
    
    params.set(SecurityParameter::Ciphersuites, SecurityParameterValue::Strings(suite_list));
    0
}

/// Set server certificate
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_server_certificate(
    handle: *mut TransportServicesHandle,
    cert_data: *const u8,
    cert_len: usize,
) -> c_int {
    if handle.is_null() || cert_data.is_null() {
        return -1;
    }
    
    let params = handle_mut::<SecurityParameters>(handle);
    let cert_slice = slice::from_raw_parts(cert_data, cert_len);
    
    let cert = Certificate {
        data: cert_slice.to_vec(),
    };
    
    params.set(SecurityParameter::ServerCertificate, SecurityParameterValue::Certificates(vec![cert]));
    0
}

/// Set client certificate
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_client_certificate(
    handle: *mut TransportServicesHandle,
    cert_data: *const u8,
    cert_len: usize,
) -> c_int {
    if handle.is_null() || cert_data.is_null() {
        return -1;
    }
    
    let params = handle_mut::<SecurityParameters>(handle);
    let cert_slice = slice::from_raw_parts(cert_data, cert_len);
    
    let cert = Certificate {
        data: cert_slice.to_vec(),
    };
    
    params.set(SecurityParameter::ClientCertificate, SecurityParameterValue::Certificates(vec![cert]));
    0
}

/// Set pre-shared key
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_pre_shared_key(
    handle: *mut TransportServicesHandle,
    key_data: *const u8,
    key_len: usize,
    identity: *const c_char,
) -> c_int {
    if handle.is_null() || key_data.is_null() || identity.is_null() {
        return -1;
    }
    
    let params = handle_mut::<SecurityParameters>(handle);
    let key_slice = slice::from_raw_parts(key_data, key_len);
    
    let identity_str = match CStr::from_ptr(identity).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -1,
    };
    
    let psk = PreSharedKey {
        key: key_slice.to_vec(),
        identity: identity_str,
    };
    
    params.set(SecurityParameter::PreSharedKey, SecurityParameterValue::Psk(psk));
    0
}

/// Set max cached sessions
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_max_cached_sessions(
    handle: *mut TransportServicesHandle,
    max_sessions: usize,
) -> c_int {
    if handle.is_null() {
        return -1;
    }
    
    let params = handle_mut::<SecurityParameters>(handle);
    params.set(SecurityParameter::MaxCachedSessions, SecurityParameterValue::Size(max_sessions));
    0
}

/// Set cached session lifetime in seconds
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_cached_session_lifetime(
    handle: *mut TransportServicesHandle,
    lifetime_seconds: u64,
) -> c_int {
    if handle.is_null() {
        return -1;
    }
    
    let params = handle_mut::<SecurityParameters>(handle);
    params.set(SecurityParameter::CachedSessionLifetimeSeconds, SecurityParameterValue::U64(lifetime_seconds));
    0
}

/// Security protocol constants for FFI
pub mod security_protocol_constants {
    pub const TRANSPORT_SERVICES_SECURITY_PROTOCOL_TLS12: i32 = 0;
    pub const TRANSPORT_SERVICES_SECURITY_PROTOCOL_TLS13: i32 = 1;
    pub const TRANSPORT_SERVICES_SECURITY_PROTOCOL_DTLS12: i32 = 2;
    pub const TRANSPORT_SERVICES_SECURITY_PROTOCOL_DTLS13: i32 = 3;
}