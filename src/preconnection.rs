//! Preconnection implementation for Transport Services
//! Based on RFC 9622 Section 6 (Preestablishment Phase)

use crate::{
    LocalEndpoint, RemoteEndpoint, TransportProperties, SecurityParameters,
    Connection, Listener, Result, TransportServicesError, EndpointIdentifier,
    Message, Framer, FramerStack,
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
    framers: FramerStack,
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
                framers: FramerStack::new(),
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

    /// Add a Message Framer to this Preconnection
    /// RFC Section 9.1.2.1: Preconnection.AddFramer(framer)
    pub async fn add_framer(&self, framer: Box<dyn Framer>) {
        let mut inner = self.inner.write().await;
        inner.framers.add_framer(framer);
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
    
    /// Initiate an active connection and send a message
    /// RFC Section 9.2.5: Send on Active Open: InitiateWithSend
    pub async fn initiate_with_send(&self, message: Message) -> Result<Connection> {
        let connection = self.initiate().await?;
        
        // Queue the message to be sent once established
        connection.send(message).await?;
        
        Ok(connection)
    }
    
    /// Initiate an active connection with timeout and send a message
    /// RFC Section 9.2.5: Send on Active Open: InitiateWithSend
    pub async fn initiate_with_send_timeout(&self, message: Message, timeout: Option<Duration>) -> Result<Connection> {
        let connection = self.initiate_with_timeout(timeout).await?;
        
        // Queue the message to be sent once established
        connection.send(message).await?;
        
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
        
        // Resolve endpoints to get all candidates
        drop(inner); // Release lock before calling resolve
        let (local_candidates, remote_candidates) = self.resolve().await?;
        
        // Create listener on local endpoints
        let listener = Listener::new(self.clone());
        listener.start().await?;
        
        // Create connection that will attempt to connect to remote endpoints
        let connection = Connection::new_with_data(
            self.clone(),
            crate::ConnectionState::Establishing,
            local_candidates.first().cloned(),
            remote_candidates.first().cloned(),
            self.transport_properties().await,
        );
        
        // Get the listener's actual bound address
        let _listen_addr = listener.local_addr().await;
        
        // Spawn tasks for simultaneous connect attempts
        let conn_clone = connection.clone();
        let remote_endpoints = remote_candidates.clone();
        
        tokio::spawn(async move {
            // Try to connect to each remote endpoint
            for remote in remote_endpoints {
                if let Some(socket_addr) = extract_socket_addr(&remote) {
                    // Attempt connection with short timeout for rendezvous
                    match tokio::time::timeout(
                        Duration::from_secs(5),
                        tokio::net::TcpStream::connect(socket_addr)
                    ).await {
                        Ok(Ok(stream)) => {
                            // Connection succeeded - update connection state
                            let mut conn = conn_clone;
                            conn.set_tcp_stream(stream).await;
                            return;
                        }
                        _ => {
                            // Try next endpoint
                            continue;
                        }
                    }
                }
            }
            
            // If all connection attempts failed, rely on incoming connection
            // The listener will handle incoming connections
        });
        
        Ok((connection, listener))
    }

    /// Resolve endpoints early (for NAT traversal, etc.)
    /// Returns resolved local and remote endpoints
    pub async fn resolve(&self) -> Result<(Vec<LocalEndpoint>, Vec<RemoteEndpoint>)> {
        let inner = self.inner.read().await;
        
        let mut resolved_locals = Vec::new();
        let mut resolved_remotes = Vec::new();
        
        // Resolve local endpoints
        for local in &inner.local_endpoints {
            // If only a port is specified (no other identifiers), expand to any addresses
            if local.identifiers.len() == 1 && 
               local.identifiers.iter().any(|id| matches!(id, EndpointIdentifier::Port(_))) {
                // Add default IPv4 and IPv6 endpoints
                let port = local.identifiers.iter()
                    .find_map(|id| match id {
                        EndpointIdentifier::Port(p) => Some(*p),
                        _ => None,
                    })
                    .unwrap_or(0);
                
                // Add IPv4 any address
                resolved_locals.push(LocalEndpoint {
                    identifiers: vec![
                        EndpointIdentifier::IpAddress("0.0.0.0".parse().unwrap()),
                        EndpointIdentifier::Port(port),
                    ],
                });
                
                // Add IPv6 any address
                resolved_locals.push(LocalEndpoint {
                    identifiers: vec![
                        EndpointIdentifier::IpAddress("::".parse().unwrap()),
                        EndpointIdentifier::Port(port),
                    ],
                });
            } else {
                resolved_locals.push(local.clone());
            }
        }
        
        // Resolve remote endpoints (hostname resolution, etc.)
        for remote in &inner.remote_endpoints {
            let resolved = remote.clone();
            
            // Try to resolve hostnames to IP addresses
            for identifier in &remote.identifiers {
                if let EndpointIdentifier::HostName(hostname) = identifier {
                    // Find associated port
                    let port = remote.identifiers.iter()
                        .find_map(|id| match id {
                            EndpointIdentifier::Port(p) => Some(*p),
                            _ => None,
                        });
                    
                    if let Some(port) = port {
                        use std::net::ToSocketAddrs;
                        let addr_string = format!("{}:{}", hostname, port);
                        
                        if let Ok(addrs) = addr_string.to_socket_addrs() {
                            for addr in addrs {
                                let mut new_identifiers = resolved.identifiers.clone();
                                new_identifiers.retain(|id| !matches!(id, EndpointIdentifier::HostName(_)));
                                new_identifiers.push(EndpointIdentifier::SocketAddress(addr));
                                
                                resolved_remotes.push(RemoteEndpoint {
                                    identifiers: new_identifiers,
                                    protocol: resolved.protocol.clone(),
                                });
                            }
                            continue;
                        }
                    }
                }
            }
            
            // If no hostname resolution was needed/successful, add as-is
            if !resolved_remotes.iter().any(|r| r.identifiers == resolved.identifiers) {
                resolved_remotes.push(resolved);
            }
        }
        
        // If no locals were resolved, use the originals
        if resolved_locals.is_empty() {
            resolved_locals = inner.local_endpoints.clone();
        }
        
        // If no remotes were resolved, use the originals
        if resolved_remotes.is_empty() {
            resolved_remotes = inner.remote_endpoints.clone();
        }
        
        Ok((resolved_locals, resolved_remotes))
    }
    
    /// Get transport properties (for internal use)
    pub(crate) async fn transport_properties(&self) -> TransportProperties {
        let inner = self.inner.read().await;
        inner.transport_properties.clone()
    }
}

/// Helper function to extract socket address from remote endpoint
fn extract_socket_addr(endpoint: &RemoteEndpoint) -> Option<std::net::SocketAddr> {
    use std::net::{IpAddr, SocketAddr};
    
    let mut ip_addr: Option<IpAddr> = None;
    let mut port: Option<u16> = None;
    
    for identifier in &endpoint.identifiers {
        match identifier {
            EndpointIdentifier::IpAddress(addr) => ip_addr = Some(*addr),
            EndpointIdentifier::Port(p) => port = Some(*p),
            EndpointIdentifier::SocketAddress(addr) => return Some(*addr),
            _ => {}
        }
    }
    
    // Try to construct socket address from IP and port
    if let (Some(ip), Some(p)) = (ip_addr, port) {
        return Some(SocketAddr::new(ip, p));
    }
    
    None
}