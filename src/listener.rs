//! Listener implementation for Transport Services
//! Based on RFC 9622 Section 7.2 (Passive Open: Listen)

use crate::{
    Connection, ConnectionState, EndpointIdentifier, LocalEndpoint, Preconnection, RemoteEndpoint,
    Result, TransportServicesError,
};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};

/// Event types that can be emitted by listeners
#[derive(Debug)]
pub enum ListenerEvent {
    /// A new connection was received
    ConnectionReceived(Connection),
    /// Listener stopped
    Stopped,
    /// Error occurred
    Error(String),
}

/// A Listener waits for incoming Connections from Remote Endpoints
pub struct Listener {
    inner: Arc<RwLock<ListenerInner>>,
    event_receiver: mpsc::UnboundedReceiver<ListenerEvent>,
    stop_sender: tokio::sync::broadcast::Sender<()>,
    active: Arc<AtomicBool>,
    connection_limit: Arc<AtomicUsize>,
}

struct ListenerInner {
    preconnection: Preconnection,
    event_sender: mpsc::UnboundedSender<ListenerEvent>,
    local_addr: Option<SocketAddr>,
}

impl Listener {
    /// Create a new Listener (internal use)
    pub(crate) fn new(preconnection: Preconnection) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let (stop_sender, _stop_receiver) = tokio::sync::broadcast::channel(1);
        let active = Arc::new(AtomicBool::new(true));
        let connection_limit = Arc::new(AtomicUsize::new(usize::MAX));

        let inner = Arc::new(RwLock::new(ListenerInner {
            preconnection: preconnection.clone(),
            event_sender: event_sender.clone(),
            local_addr: None,
        }));

        Self {
            inner,
            event_receiver,
            stop_sender,
            active,
            connection_limit,
        }
    }

    /// Start listening on the configured endpoints
    pub(crate) async fn start(&self) -> Result<()> {
        let inner = self.inner.read().await;
        // Access preconnection data through public API
        let (local_endpoints, _) = inner.preconnection.resolve().await?;

        // Get local endpoint to bind to
        let local_endpoint = local_endpoints.first().ok_or_else(|| {
            TransportServicesError::InvalidParameters(
                "No local endpoint specified for listen".to_string(),
            )
        })?;

        // Extract socket address to bind to
        let bind_addr = self.extract_bind_address(local_endpoint)?;

        // Start TCP listener
        let tcp_listener = TcpListener::bind(bind_addr)
            .await
            .map_err(TransportServicesError::Io)?;

        let actual_addr = tcp_listener
            .local_addr()
            .map_err(TransportServicesError::Io)?;

        // Update local address
        drop(inner);
        let mut inner = self.inner.write().await;
        inner.local_addr = Some(actual_addr);
        let event_sender = inner.event_sender.clone();
        let preconnection = inner.preconnection.clone();
        drop(inner);

        // Create a channel to signal when the accept loop is ready
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();

        // Spawn accept loop
        let active = Arc::clone(&self.active);
        let connection_limit = Arc::clone(&self.connection_limit);
        let mut stop_receiver = self.stop_sender.subscribe();

        tokio::spawn(async move {
            // Signal that we're ready to accept connections
            let _ = ready_tx.send(());
            
            loop {
                if !active.load(Ordering::Relaxed) {
                    break;
                }

                tokio::select! {
                    _ = stop_receiver.recv() => {
                        break;
                    }
                    result = tcp_listener.accept() => {
                        match result {
                            Ok((stream, peer_addr)) => {
                                // Check connection limit
                                let current = connection_limit.load(Ordering::Relaxed);
                                if current == 0 {
                                    // Drop connection - limit reached
                                    drop(stream);
                                    continue;
                                }

                                // Decrement limit if not unlimited
                                if current != usize::MAX {
                                    connection_limit.fetch_sub(1, Ordering::Relaxed);
                                }

                                // Create connection from accepted stream
                                let conn = Self::create_connection_from_stream(
                                    stream,
                                    peer_addr,
                                    actual_addr,
                                    &preconnection
                                ).await;

                                let _ = event_sender.send(ListenerEvent::ConnectionReceived(conn));
                            }
                            Err(e) => {
                                let _ = event_sender.send(ListenerEvent::Error(e.to_string()));
                            }
                        }
                    }
                }
            }

            active.store(false, Ordering::Relaxed);
            let _ = event_sender.send(ListenerEvent::Stopped);
        });

        // Wait for the accept loop to be ready
        let _ = ready_rx.await;

        Ok(())
    }

    /// Extract bind address from local endpoint
    fn extract_bind_address(&self, endpoint: &LocalEndpoint) -> Result<SocketAddr> {
        let mut ip_addr = None;
        let mut port = None;

        for identifier in &endpoint.identifiers {
            match identifier {
                EndpointIdentifier::IpAddress(addr) => ip_addr = Some(*addr),
                EndpointIdentifier::Port(p) => port = Some(*p),
                EndpointIdentifier::SocketAddress(addr) => return Ok(*addr),
                _ => {}
            }
        }

        // Default to 0.0.0.0:0 if not specified
        let ip = ip_addr.unwrap_or_else(|| "0.0.0.0".parse().unwrap());
        let p = port.unwrap_or(0);
        Ok(SocketAddr::new(ip, p))
    }

    /// Create a connection from an accepted TCP stream
    async fn create_connection_from_stream(
        stream: TcpStream,
        peer_addr: SocketAddr,
        local_addr: SocketAddr,
        preconnection: &Preconnection,
    ) -> Connection {
        let transport_properties = preconnection.transport_properties().await;

        // Create endpoints
        let local_endpoint = LocalEndpoint {
            identifiers: vec![EndpointIdentifier::SocketAddress(local_addr)],
        };

        let remote_endpoint = RemoteEndpoint {
            identifiers: vec![EndpointIdentifier::SocketAddress(peer_addr)],
            protocol: None,
        };

        // Create connection with established state
        let mut conn = Connection::new_with_data(
            preconnection.clone(),
            ConnectionState::Established,
            Some(local_endpoint),
            Some(remote_endpoint),
            transport_properties,
        );

        // Set the TCP stream
        conn.set_tcp_stream(stream).await;

        conn
    }

    /// Accept the next incoming connection
    pub async fn accept(&mut self) -> Result<Connection> {
        loop {
            match self.event_receiver.recv().await {
                Some(ListenerEvent::ConnectionReceived(connection)) => return Ok(connection),
                Some(ListenerEvent::Stopped) => {
                    return Err(TransportServicesError::InvalidState(
                        "Listener stopped".to_string(),
                    ))
                }
                Some(ListenerEvent::Error(e)) => {
                    // Continue listening after non-fatal errors
                    eprintln!("Listener error: {e}");
                }
                None => {
                    return Err(TransportServicesError::InvalidState(
                        "Listener closed".to_string(),
                    ))
                }
            }
        }
    }

    /// Get the next event without blocking
    pub async fn next_event(&mut self) -> Option<ListenerEvent> {
        self.event_receiver.recv().await
    }

    /// Stop listening for new connections
    pub async fn stop(&self) -> Result<()> {
        if self.active.swap(false, Ordering::Relaxed) {
            let _ = self.stop_sender.send(());
        }
        Ok(())
    }

    /// Check if the listener is still active
    pub async fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Set connection limit
    pub fn set_new_connection_limit(&self, limit: usize) {
        self.connection_limit.store(limit, Ordering::Relaxed);
    }

    /// Get the local address the listener is bound to
    pub async fn local_addr(&self) -> Option<SocketAddr> {
        let inner = self.inner.read().await;
        inner.local_addr
    }

    /// Get the preconnection this listener was created from
    pub async fn preconnection(&self) -> Preconnection {
        let inner = self.inner.read().await;
        inner.preconnection.clone()
    }
}

impl std::fmt::Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listener")
            .field("active", &self.active.load(Ordering::Relaxed))
            .field(
                "connection_limit",
                &self.connection_limit.load(Ordering::Relaxed),
            )
            .finish()
    }
}
