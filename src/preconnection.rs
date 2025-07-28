//! Preconnection implementation for Transport Services
//! Based on RFC 9622 Section 6 (Preestablishment Phase)

use crate::{
    LocalEndpoint, RemoteEndpoint, TransportProperties, SecurityParameters,
    Connection, Listener, Result, TransportServicesError
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Create a new Preconnection as defined in RFC Section 6
/// This is the primary way to create a Preconnection object
pub fn new_preconnection(
    local_endpoints: Vec<LocalEndpoint>,
    remote_endpoints: Vec<RemoteEndpoint>,
    transport_properties: TransportProperties,
    security_parameters: SecurityParameters,
) -> Preconnection {
    Preconnection::new(
        local_endpoints,
        remote_endpoints,
        transport_properties,
        security_parameters,
    )
}

/// A Preconnection represents a potential Connection
/// It is a passive object that maintains the state describing
/// the properties of a Connection that might exist in the future
#[derive(Clone)]
pub struct Preconnection {
    inner: Arc<RwLock<PreconnectionInner>>,
}

struct PreconnectionInner {
    local_endpoints: Vec<LocalEndpoint>,
    remote_endpoints: Vec<RemoteEndpoint>,
    transport_properties: TransportProperties,
    security_parameters: SecurityParameters,
}

impl Preconnection {
    /// Create a new Preconnection
    pub fn new(
        local_endpoints: Vec<LocalEndpoint>,
        remote_endpoints: Vec<RemoteEndpoint>,
        transport_properties: TransportProperties,
        security_parameters: SecurityParameters,
    ) -> Self {
        Self {
            inner: Arc::new(RwLock::new(PreconnectionInner {
                local_endpoints,
                remote_endpoints,
                transport_properties,
                security_parameters,
            })),
        }
    }

    /// Create a Preconnection with a single local endpoint
    pub fn with_local_endpoint(local: LocalEndpoint) -> Self {
        Self::new(
            vec![local],
            vec![],
            TransportProperties::default(),
            SecurityParameters::default(),
        )
    }

    /// Create a Preconnection with a single remote endpoint
    pub fn with_remote_endpoint(remote: RemoteEndpoint) -> Self {
        Self::new(
            vec![],
            vec![remote],
            TransportProperties::default(),
            SecurityParameters::default(),
        )
    }

    /// Add a local endpoint
    pub async fn add_local(&self, endpoint: LocalEndpoint) {
        let mut inner = self.inner.write().await;
        inner.local_endpoints.push(endpoint);
    }

    /// Add a remote endpoint
    pub async fn add_remote(&self, endpoint: RemoteEndpoint) {
        let mut inner = self.inner.write().await;
        inner.remote_endpoints.push(endpoint);
    }

    /// Set transport properties
    pub async fn set_transport_properties(&self, properties: TransportProperties) {
        let mut inner = self.inner.write().await;
        inner.transport_properties = properties;
    }

    /// Set security parameters
    pub async fn set_security_parameters(&self, parameters: SecurityParameters) {
        let mut inner = self.inner.write().await;
        inner.security_parameters = parameters;
    }

    /// Initiate an active connection (client mode)
    /// RFC Section 7.1
    pub async fn initiate(&self) -> Result<Connection> {
        self.initiate_with_timeout(None).await
    }
    
    /// Initiate an active connection with timeout
    /// RFC Section 7.1: Connection := Preconnection.Initiate(timeout?)
    pub async fn initiate_with_timeout(&self, timeout: Option<Duration>) -> Result<Connection> {
        let inner = self.inner.read().await;
        
        // Validate that we have at least one remote endpoint
        if inner.remote_endpoints.is_empty() {
            return Err(TransportServicesError::InvalidParameters(
                "No remote endpoints specified for initiate".to_string()
            ));
        }
        
        // Create the connection object
        let connection = Connection::new_with_data(
            self.clone(),
            crate::ConnectionState::Establishing,
            inner.local_endpoints.first().cloned(),
            inner.remote_endpoints.first().cloned(),
            inner.transport_properties.clone(),
        );
        
        // Extract remote endpoint information
        let remote_endpoint = &inner.remote_endpoints[0];
        let socket_addr = self.extract_socket_address(remote_endpoint)?;
        
        // Get connection timeout from transport properties if not specified
        let connection_timeout = timeout.or(inner.transport_properties.connection_properties.connection_timeout);
        
        // Clone connection for the spawned task
        let conn_clone = connection.clone();
        
        // Spawn the connection establishment task
        tokio::spawn(async move {
            let _ = conn_clone.establish_tcp(socket_addr, connection_timeout).await;
        });
        
        Ok(connection)
    }
    
    /// Extract a socket address from an endpoint
    fn extract_socket_address(&self, endpoint: &RemoteEndpoint) -> Result<std::net::SocketAddr> {
        use std::net::{IpAddr, SocketAddr};
        use crate::EndpointIdentifier;
        
        let mut ip_addr: Option<IpAddr> = None;
        let mut port: Option<u16> = None;
        let mut hostname: Option<String> = None;
        
        // Extract components from identifiers
        for identifier in &endpoint.identifiers {
            match identifier {
                EndpointIdentifier::IpAddress(addr) => ip_addr = Some(*addr),
                EndpointIdentifier::Port(p) => port = Some(*p),
                EndpointIdentifier::HostName(h) => hostname = Some(h.clone()),
                EndpointIdentifier::SocketAddress(addr) => return Ok(*addr),
                _ => {}
            }
        }
        
        // Try to construct socket address
        if let (Some(ip), Some(p)) = (ip_addr, port) {
            return Ok(SocketAddr::new(ip, p));
        }
        
        // Try hostname resolution
        if let (Some(host), Some(p)) = (hostname, port) {
            // For now, use blocking resolver in a spawn_blocking task
            // In production, we'd want to use trust-dns or similar async resolver
            use std::net::ToSocketAddrs;
            let addr_string = format!("{}:{}", host, p);
            
            match addr_string.to_socket_addrs() {
                Ok(mut addrs) => {
                    if let Some(addr) = addrs.next() {
                        return Ok(addr);
                    }
                }
                Err(e) => {
                    return Err(TransportServicesError::InvalidParameters(
                        format!("Failed to resolve hostname: {}", e)
                    ));
                }
            }
        }
        
        Err(TransportServicesError::InvalidParameters(
            "No valid socket address could be extracted from endpoint".to_string()
        ))
    }

    /// Listen for incoming connections (server mode)
    /// RFC Section 7.2
    pub async fn listen(&self) -> Result<Listener> {
        let inner = self.inner.read().await;
        
        // Validate that we have at least one local endpoint
        if inner.local_endpoints.is_empty() {
            return Err(TransportServicesError::InvalidParameters(
                "No local endpoints specified for listen".to_string()
            ));
        }

        // Create and start the listener
        let listener = Listener::new(self.clone());
        listener.start().await?;
        
        Ok(listener)
    }

    /// Rendezvous for peer-to-peer connections
    /// RFC Section 7.3
    pub async fn rendezvous(&self) -> Result<(Connection, Listener)> {
        let inner = self.inner.read().await;
        
        // Validate that we have both local and remote endpoints
        if inner.local_endpoints.is_empty() {
            return Err(TransportServicesError::InvalidParameters(
                "No local endpoints specified for rendezvous".to_string()
            ));
        }
        if inner.remote_endpoints.is_empty() {
            return Err(TransportServicesError::InvalidParameters(
                "No remote endpoints specified for rendezvous".to_string()
            ));
        }

        // TODO: Implement actual rendezvous
        // For now, return placeholders
        let connection = Connection::new(
            self.clone(),
            crate::ConnectionState::Establishing,
        );
        let listener = Listener::new(self.clone());
        
        Ok((connection, listener))
    }

    /// Resolve endpoints early (for NAT traversal, etc.)
    /// Returns resolved local and remote endpoints
    pub async fn resolve(&self) -> Result<(Vec<LocalEndpoint>, Vec<RemoteEndpoint>)> {
        let inner = self.inner.read().await;
        
        // TODO: Implement actual endpoint resolution
        // For now, return the existing endpoints
        Ok((
            inner.local_endpoints.clone(),
            inner.remote_endpoints.clone(),
        ))
    }
    
    /// Get transport properties (for internal use)
    pub(crate) async fn transport_properties(&self) -> TransportProperties {
        let inner = self.inner.read().await;
        inner.transport_properties.clone()
    }
}