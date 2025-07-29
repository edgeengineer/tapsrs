//! Transport Services - A Rust implementation of RFC 9622
//!
//! This library provides an abstract API for transport protocols that enables
//! the selection of transport protocols and network paths dynamically at runtime.

pub mod connection;
pub mod connection_group;
pub mod connection_properties;
pub mod error;
pub mod framer;
pub mod listener;
pub mod message;
pub mod preconnection;
pub mod types;

#[cfg(feature = "ffi")]
pub mod ffi;

pub use connection::Connection;
pub use connection_group::{ConnectionGroup, ConnectionGroupId};
pub use connection_properties::{
    CapacityProfile, ChecksumCoverage, ConnectionProperties, ConnectionProperty, MultipathPolicy,
    SchedulerType, TimeoutValue,
};
pub use error::{Result, TransportServicesError};
pub use framer::{Framer, FramerStack, LengthPrefixFramer};
pub use listener::Listener;
pub use message::{Message, MessageContext};
pub use preconnection::Preconnection;
pub use types::*;

#[cfg(test)]
mod tests;
