//! Connection Groups for Transport Services
//! Based on RFC 9622 Section 7.4 (Connection Groups)

use crate::{TransportProperties, LocalEndpoint, RemoteEndpoint};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Unique identifier for a connection group
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionGroupId(Uuid);

impl ConnectionGroupId {
    /// Create a new unique connection group ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl std::fmt::Display for ConnectionGroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents a group of related connections that share properties
/// RFC Section 7.4: Connection Groups
#[derive(Debug)]
pub struct ConnectionGroup {
    /// Unique identifier for this group
    pub id: ConnectionGroupId,
    /// Shared transport properties for all connections in the group
    pub transport_properties: Arc<RwLock<TransportProperties>>,
    /// Shared local endpoints
    pub local_endpoints: Arc<RwLock<Vec<LocalEndpoint>>>,
    /// Shared remote endpoints
    pub remote_endpoints: Arc<RwLock<Vec<RemoteEndpoint>>>,
    /// Number of active connections in this group
    pub connection_count: Arc<AtomicU64>,
    /// Whether this group supports multistreaming (e.g., QUIC, HTTP/2)
    pub multistreaming_capable: bool,
}

impl ConnectionGroup {
    /// Create a new connection group
    pub fn new(
        transport_properties: TransportProperties,
        local_endpoints: Vec<LocalEndpoint>,
        remote_endpoints: Vec<RemoteEndpoint>,
    ) -> Self {
        Self {
            id: ConnectionGroupId::new(),
            transport_properties: Arc::new(RwLock::new(transport_properties)),
            local_endpoints: Arc::new(RwLock::new(local_endpoints)),
            remote_endpoints: Arc::new(RwLock::new(remote_endpoints)),
            connection_count: Arc::new(AtomicU64::new(0)),
            multistreaming_capable: false, // Will be determined by protocol selection
        }
    }
    
    /// Increment the connection count
    pub fn add_connection(&self) {
        self.connection_count.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Decrement the connection count
    pub fn remove_connection(&self) {
        self.connection_count.fetch_sub(1, Ordering::Relaxed);
    }
    
    /// Get the current number of connections in the group
    pub fn connection_count(&self) -> u64 {
        self.connection_count.load(Ordering::Relaxed)
    }
    
    /// Check if this group has any active connections
    pub fn has_connections(&self) -> bool {
        self.connection_count() > 0
    }
}

impl Clone for ConnectionGroup {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            transport_properties: Arc::clone(&self.transport_properties),
            local_endpoints: Arc::clone(&self.local_endpoints),
            remote_endpoints: Arc::clone(&self.remote_endpoints),
            connection_count: Arc::clone(&self.connection_count),
            multistreaming_capable: self.multistreaming_capable,
        }
    }
}