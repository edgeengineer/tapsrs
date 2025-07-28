//! Transport Services - A Rust implementation of RFC 9622
//! 
//! This library provides an abstract API for transport protocols that enables
//! the selection of transport protocols and network paths dynamically at runtime.

pub mod types;
pub mod preconnection;
pub mod connection;
pub mod connection_group;
pub mod listener;
pub mod message;
pub mod error;

#[cfg(feature = "ffi")]
pub mod ffi;

pub use types::*;
pub use preconnection::{Preconnection, new_preconnection};
pub use connection::Connection;
pub use connection_group::{ConnectionGroup, ConnectionGroupId};
pub use listener::Listener;
pub use message::{Message, MessageContext};
pub use error::{TransportServicesError, Result};

#[cfg(test)]
mod tests;