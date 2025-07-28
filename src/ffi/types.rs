//! FFI type definitions

use std::os::raw::{c_char, c_int, c_void};

/// FFI representation of Preference
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum TransportServicesPreference {
    Require = 0,
    Prefer = 1,
    NoPreference = 2,
    Avoid = 3,
    Prohibit = 4,
}

impl From<crate::Preference> for TransportServicesPreference {
    fn from(pref: crate::Preference) -> Self {
        match pref {
            crate::Preference::Require => TransportServicesPreference::Require,
            crate::Preference::Prefer => TransportServicesPreference::Prefer,
            crate::Preference::NoPreference => TransportServicesPreference::NoPreference,
            crate::Preference::Avoid => TransportServicesPreference::Avoid,
            crate::Preference::Prohibit => TransportServicesPreference::Prohibit,
        }
    }
}

impl From<TransportServicesPreference> for crate::Preference {
    fn from(pref: TransportServicesPreference) -> Self {
        match pref {
            TransportServicesPreference::Require => crate::Preference::Require,
            TransportServicesPreference::Prefer => crate::Preference::Prefer,
            TransportServicesPreference::NoPreference => crate::Preference::NoPreference,
            TransportServicesPreference::Avoid => crate::Preference::Avoid,
            TransportServicesPreference::Prohibit => crate::Preference::Prohibit,
        }
    }
}

/// FFI representation of endpoint
#[repr(C)]
pub struct TransportServicesEndpoint {
    pub hostname: *const c_char,
    pub port: u16,
    pub service: *const c_char,
    pub interface: *const c_char,
}

/// FFI representation of transport properties
#[repr(C)]
pub struct TransportServicesTransportProperties {
    pub reliability: TransportServicesPreference,
    pub preserve_msg_boundaries: TransportServicesPreference,
    pub preserve_order: TransportServicesPreference,
    pub congestion_control: TransportServicesPreference,
    pub multipath: c_int,
}

/// FFI representation of security parameters
#[repr(C)]
pub struct TransportServicesSecurityParameters {
    pub disabled: bool,
    pub opportunistic: bool,
    pub server_certificate: *const c_void,
    pub client_certificate: *const c_void,
}

/// FFI representation of message
#[repr(C)]
pub struct TransportServicesMessage {
    pub data: *const u8,
    pub length: usize,
    pub lifetime_ms: u64,
    pub priority: i32,
    pub idempotent: bool,
    pub final_message: bool,
}

/// Connection state for FFI
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum TransportServicesConnectionState {
    Establishing = 0,
    Established = 1,
    Closing = 2,
    Closed = 3,
}

impl From<crate::ConnectionState> for TransportServicesConnectionState {
    fn from(state: crate::ConnectionState) -> Self {
        match state {
            crate::ConnectionState::Establishing => TransportServicesConnectionState::Establishing,
            crate::ConnectionState::Established => TransportServicesConnectionState::Established,
            crate::ConnectionState::Closing => TransportServicesConnectionState::Closing,
            crate::ConnectionState::Closed => TransportServicesConnectionState::Closed,
        }
    }
}

/// Error codes for FFI
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum TransportServicesError {
    Success = 0,
    InvalidParameters = -1,
    EstablishmentFailed = -2,
    ConnectionFailed = -3,
    SendFailed = -4,
    ReceiveFailed = -5,
    NotSupported = -6,
    Timeout = -7,
    InvalidState = -8,
    SecurityError = -9,
    IoError = -10,
    Unknown = -99,
}

impl From<crate::TransportServicesError> for TransportServicesError {
    fn from(err: crate::TransportServicesError) -> Self {
        match err {
            crate::TransportServicesError::InvalidParameters(_) => TransportServicesError::InvalidParameters,
            crate::TransportServicesError::EstablishmentFailed(_) => TransportServicesError::EstablishmentFailed,
            crate::TransportServicesError::ConnectionFailed(_) => TransportServicesError::ConnectionFailed,
            crate::TransportServicesError::SendFailed(_) => TransportServicesError::SendFailed,
            crate::TransportServicesError::ReceiveFailed(_) => TransportServicesError::ReceiveFailed,
            crate::TransportServicesError::NotSupported(_) => TransportServicesError::NotSupported,
            crate::TransportServicesError::Timeout => TransportServicesError::Timeout,
            crate::TransportServicesError::InvalidState(_) => TransportServicesError::InvalidState,
            crate::TransportServicesError::SecurityError(_) => TransportServicesError::SecurityError,
            crate::TransportServicesError::Io(_) => TransportServicesError::IoError,
            _ => TransportServicesError::Unknown,
        }
    }
}

/// Callback function types
pub type TransportServicesConnectionCallback = extern "C" fn(connection: *mut super::TransportServicesHandle, user_data: *mut c_void);
pub type TransportServicesErrorCallback = extern "C" fn(error: TransportServicesError, message: *const c_char, user_data: *mut c_void);
pub type TransportServicesMessageCallback = extern "C" fn(message: *const TransportServicesMessage, user_data: *mut c_void);