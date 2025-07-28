//! FFI type definitions

use std::os::raw::{c_char, c_int, c_void};

/// FFI representation of Preference
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum TapsPreference {
    Require = 0,
    Prefer = 1,
    NoPreference = 2,
    Avoid = 3,
    Prohibit = 4,
}

impl From<crate::Preference> for TapsPreference {
    fn from(pref: crate::Preference) -> Self {
        match pref {
            crate::Preference::Require => TapsPreference::Require,
            crate::Preference::Prefer => TapsPreference::Prefer,
            crate::Preference::NoPreference => TapsPreference::NoPreference,
            crate::Preference::Avoid => TapsPreference::Avoid,
            crate::Preference::Prohibit => TapsPreference::Prohibit,
        }
    }
}

impl From<TapsPreference> for crate::Preference {
    fn from(pref: TapsPreference) -> Self {
        match pref {
            TapsPreference::Require => crate::Preference::Require,
            TapsPreference::Prefer => crate::Preference::Prefer,
            TapsPreference::NoPreference => crate::Preference::NoPreference,
            TapsPreference::Avoid => crate::Preference::Avoid,
            TapsPreference::Prohibit => crate::Preference::Prohibit,
        }
    }
}

/// FFI representation of endpoint
#[repr(C)]
pub struct TapsEndpoint {
    pub hostname: *const c_char,
    pub port: u16,
    pub service: *const c_char,
    pub interface: *const c_char,
}

/// FFI representation of transport properties
#[repr(C)]
pub struct TapsTransportProperties {
    pub reliability: TapsPreference,
    pub preserve_msg_boundaries: TapsPreference,
    pub preserve_order: TapsPreference,
    pub congestion_control: TapsPreference,
    pub multipath: c_int,
}

/// FFI representation of security parameters
#[repr(C)]
pub struct TapsSecurityParameters {
    pub disabled: bool,
    pub opportunistic: bool,
    pub server_certificate: *const c_void,
    pub client_certificate: *const c_void,
}

/// FFI representation of message
#[repr(C)]
pub struct TapsMessage {
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
pub enum TapsConnectionState {
    Establishing = 0,
    Established = 1,
    Closing = 2,
    Closed = 3,
}

impl From<crate::ConnectionState> for TapsConnectionState {
    fn from(state: crate::ConnectionState) -> Self {
        match state {
            crate::ConnectionState::Establishing => TapsConnectionState::Establishing,
            crate::ConnectionState::Established => TapsConnectionState::Established,
            crate::ConnectionState::Closing => TapsConnectionState::Closing,
            crate::ConnectionState::Closed => TapsConnectionState::Closed,
        }
    }
}

/// Error codes for FFI
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum TapsError {
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

impl From<crate::TapsError> for TapsError {
    fn from(err: crate::TapsError) -> Self {
        match err {
            crate::TapsError::InvalidParameters(_) => TapsError::InvalidParameters,
            crate::TapsError::EstablishmentFailed(_) => TapsError::EstablishmentFailed,
            crate::TapsError::ConnectionFailed(_) => TapsError::ConnectionFailed,
            crate::TapsError::SendFailed(_) => TapsError::SendFailed,
            crate::TapsError::ReceiveFailed(_) => TapsError::ReceiveFailed,
            crate::TapsError::NotSupported(_) => TapsError::NotSupported,
            crate::TapsError::Timeout => TapsError::Timeout,
            crate::TapsError::InvalidState(_) => TapsError::InvalidState,
            crate::TapsError::SecurityError(_) => TapsError::SecurityError,
            crate::TapsError::Io(_) => TapsError::IoError,
            _ => TapsError::Unknown,
        }
    }
}

/// Callback function types
pub type TapsConnectionCallback = extern "C" fn(connection: *mut super::TapsHandle, user_data: *mut c_void);
pub type TapsErrorCallback = extern "C" fn(error: TapsError, message: *const c_char, user_data: *mut c_void);
pub type TapsMessageCallback = extern "C" fn(message: *const TapsMessage, user_data: *mut c_void);