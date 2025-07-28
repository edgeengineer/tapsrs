//! Error types for Transport Services

use std::error::Error;
use std::fmt;
use std::io;

/// Result type alias for Transport Services operations
pub type Result<T> = std::result::Result<T, TransportServicesError>;

/// Main error type for Transport Services operations
#[derive(Debug)]
pub enum TransportServicesError {
    /// Connection establishment failed (RFC 7.1, 7.2, 7.3).
    /// Corresponds to the `EstablishmentError` event.
    EstablishmentFailed(String),

    /// A terminal error occurred on an established connection (RFC 10).
    /// Corresponds to the `ConnectionError` event.
    ConnectionFailed(String),

    /// An error occurred while trying to send a message (RFC 9.2.2.3).
    /// Corresponds to the `SendError` event.
    SendFailed(String),

    /// An error occurred while trying to receive a message (RFC 9.3.2.3).
    /// Corresponds to the `ReceiveError` event.
    ReceiveFailed(String),

    /// An error occurred while cloning a connection (RFC 7.4).
    /// Corresponds to the `CloneError` event.
    CloneFailed(String),

    /// A message expired before it could be sent (RFC 9.2.2.2).
    /// Corresponds to the `Expired` event.
    MessageExpired,

    /// Invalid parameters were provided during pre-establishment.
    InvalidParameters(String),

    /// The operation is not valid for the current connection state.
    InvalidState(String),

    /// An underlying security or TLS error occurred.
    SecurityError(String),

    /// An underlying I/O error occurred.
    Io(io::Error),

    /// The requested feature is not supported by the implementation or selected protocol.
    NotSupported(String),

    /// A user-specified or internal timeout was reached.
    Timeout,

    /// A message was larger than the specified maximum length.
    MessageTooLarge(String),
}

impl fmt::Display for TransportServicesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportServicesError::EstablishmentFailed(msg) => {
                write!(f, "Connection establishment failed: {}", msg)
            }
            TransportServicesError::ConnectionFailed(msg) => {
                write!(f, "Connection failed: {}", msg)
            }
            TransportServicesError::SendFailed(msg) => write!(f, "Send failed: {}", msg),
            TransportServicesError::ReceiveFailed(msg) => write!(f, "Receive failed: {}", msg),
            TransportServicesError::CloneFailed(msg) => write!(f, "Clone failed: {}", msg),
            TransportServicesError::MessageExpired => write!(f, "Message expired before sending"),
            TransportServicesError::InvalidParameters(msg) => {
                write!(f, "Invalid parameters: {}", msg)
            }
            TransportServicesError::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
            TransportServicesError::SecurityError(msg) => write!(f, "Security error: {}", msg),
            TransportServicesError::Io(err) => write!(f, "I/O error: {}", err),
            TransportServicesError::NotSupported(msg) => {
                write!(f, "Operation not supported: {}", msg)
            }
            TransportServicesError::Timeout => write!(f, "Operation timed out"),
            TransportServicesError::MessageTooLarge(msg) => write!(f, "Message too large: {}", msg),
        }
    }
}

impl Error for TransportServicesError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            TransportServicesError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for TransportServicesError {
    fn from(err: io::Error) -> Self {
        TransportServicesError::Io(err)
    }
}
