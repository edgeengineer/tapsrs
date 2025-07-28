//! Error types for TAPS

use std::fmt;
use std::error::Error;
use std::io;

/// Result type alias for TAPS operations
pub type Result<T> = std::result::Result<T, TapsError>;

/// Main error type for TAPS operations
#[derive(Debug)]
pub enum TapsError {
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
}

impl fmt::Display for TapsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TapsError::EstablishmentFailed(msg) => write!(f, "Connection establishment failed: {}", msg),
            TapsError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            TapsError::SendFailed(msg) => write!(f, "Send failed: {}", msg),
            TapsError::ReceiveFailed(msg) => write!(f, "Receive failed: {}", msg),
            TapsError::CloneFailed(msg) => write!(f, "Clone failed: {}", msg),
            TapsError::MessageExpired => write!(f, "Message expired before sending"),
            TapsError::InvalidParameters(msg) => write!(f, "Invalid parameters: {}", msg),
            TapsError::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
            TapsError::SecurityError(msg) => write!(f, "Security error: {}", msg),
            TapsError::Io(err) => write!(f, "I/O error: {}", err),
            TapsError::NotSupported(msg) => write!(f, "Operation not supported: {}", msg),
            TapsError::Timeout => write!(f, "Operation timed out"),
        }
    }
}

impl Error for TapsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            TapsError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for TapsError {
    fn from(err: io::Error) -> Self {
        TapsError::Io(err)
    }
}
