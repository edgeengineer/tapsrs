//! Connection implementation for Transport Services
//! Based on RFC 9622 Section 3 (API Summary) and Section 8 (Managing Connections)

use crate::{
    Preconnection, ConnectionState, ConnectionEvent, Message, MessageContext,
    TransportProperties, LocalEndpoint, RemoteEndpoint, Result, TransportServicesError,
    EndpointIdentifier,
};
use std::sync::Arc;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

/// A Connection represents an instance of a transport Protocol Stack
/// on which data can be sent to and/or received from a Remote Endpoint
pub struct Connection {
    inner: Arc<RwLock<ConnectionInner>>,
    event_sender: mpsc::UnboundedSender<ConnectionEvent>,
    event_receiver: Arc<RwLock<mpsc::UnboundedReceiver<ConnectionEvent>>>,
}

struct ConnectionInner {
    preconnection: Preconnection,
    state: ConnectionState,
    local_endpoint: Option<LocalEndpoint>,
    remote_endpoint: Option<RemoteEndpoint>,
    #[allow(dead_code)]
    transport_properties: TransportProperties,
    // Actual network stream (for now just TCP)
    tcp_stream: Option<TcpStream>,
    // Message queue for messages sent before connection is established
    pending_messages: Vec<Message>,
}

impl Connection {
    /// Clone the connection for internal use
    pub(crate) fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            event_sender: self.event_sender.clone(),
            event_receiver: Arc::clone(&self.event_receiver),
        }
    }
    /// Create a new Connection (internal use)
    pub(crate) fn new(preconnection: Preconnection, state: ConnectionState) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        Self {
            inner: Arc::new(RwLock::new(ConnectionInner {
                preconnection,
                state,
                local_endpoint: None,
                remote_endpoint: None,
                transport_properties: TransportProperties::default(),
                tcp_stream: None,
                pending_messages: Vec::new(),
            })),
            event_sender,
            event_receiver: Arc::new(RwLock::new(event_receiver)),
        }
    }
    
    /// Create a new Connection with pre-populated data (for initiate)
    pub(crate) fn new_with_data(
        preconnection: Preconnection,
        state: ConnectionState,
        local_endpoint: Option<LocalEndpoint>,
        remote_endpoint: Option<RemoteEndpoint>,
        transport_properties: TransportProperties,
    ) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        Self {
            inner: Arc::new(RwLock::new(ConnectionInner {
                preconnection,
                state,
                local_endpoint,
                remote_endpoint,
                transport_properties,
                tcp_stream: None,
                pending_messages: Vec::new(),
            })),
            event_sender,
            event_receiver: Arc::new(RwLock::new(event_receiver)),
        }
    }

    /// Get the current state of the connection
    pub async fn state(&self) -> ConnectionState {
        let inner = self.inner.read().await;
        inner.state
    }

    /// Send a message on the connection
    /// RFC Section 9.2
    pub async fn send(&self, message: Message) -> Result<()> {
        let mut inner = self.inner.write().await;
        
        match inner.state {
            ConnectionState::Established => {
                if let Some(ref mut stream) = inner.tcp_stream {
                    stream.write_all(message.data()).await
                        .map_err(|e| TransportServicesError::SendFailed(e.to_string()))?;
                    stream.flush().await
                        .map_err(|e| TransportServicesError::SendFailed(e.to_string()))?;
                    Ok(())
                } else {
                    Err(TransportServicesError::InvalidState(
                        "No active stream".to_string()
                    ))
                }
            }
            ConnectionState::Establishing => {
                // Queue message for sending after establishment
                inner.pending_messages.push(message);
                Ok(())
            }
            _ => Err(TransportServicesError::InvalidState(
                "Cannot send on a closed connection".to_string()
            )),
        }
    }

    /// Receive messages from the connection
    /// RFC Section 9.3
    pub async fn receive(&self) -> Result<(Message, MessageContext)> {
        let inner = self.inner.read().await;
        
        match inner.state {
            ConnectionState::Established | ConnectionState::Establishing => {
                // TODO: Implement actual message receiving
                Err(TransportServicesError::NotSupported("Receive not yet implemented".to_string()))
            }
            _ => Err(TransportServicesError::InvalidState(
                "Cannot receive on a closed connection".to_string()
            )),
        }
    }

    /// Close the connection gracefully
    /// RFC Section 10
    pub async fn close(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        
        match inner.state {
            ConnectionState::Established | ConnectionState::Establishing => {
                inner.state = ConnectionState::Closing;
                // TODO: Implement graceful close
                inner.state = ConnectionState::Closed;
                let _ = self.event_sender.send(ConnectionEvent::Closed);
                Ok(())
            }
            _ => Ok(()), // Already closed
        }
    }

    /// Abort the connection immediately
    pub async fn abort(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.state = ConnectionState::Closed;
        let _ = self.event_sender.send(ConnectionEvent::Closed);
        // TODO: Implement immediate abort
        Ok(())
    }

    /// Clone the connection to create a new connection in the same group
    /// RFC Section 7.4
    pub async fn clone_connection(&self) -> Result<Connection> {
        let inner = self.inner.read().await;
        
        match inner.state {
            ConnectionState::Established => {
                // Create a new connection using the same preconnection
                inner.preconnection.initiate().await
            }
            _ => Err(TransportServicesError::InvalidState(
                "Can only clone established connections".to_string()
            )),
        }
    }

    /// Add a remote endpoint to the connection
    /// RFC Section 7.5
    pub async fn add_remote(&self, _endpoint: RemoteEndpoint) -> Result<()> {
        let inner = self.inner.read().await;
        
        match inner.state {
            ConnectionState::Established | ConnectionState::Establishing => {
                // TODO: Implement adding remote endpoints to active connection
                Ok(())
            }
            _ => Err(TransportServicesError::InvalidState(
                "Cannot add endpoints to a closed connection".to_string()
            )),
        }
    }

    /// Add a local endpoint to the connection
    pub async fn add_local(&self, _endpoint: LocalEndpoint) -> Result<()> {
        let inner = self.inner.read().await;
        
        match inner.state {
            ConnectionState::Established | ConnectionState::Establishing => {
                // TODO: Implement adding local endpoints to active connection
                Ok(())
            }
            _ => Err(TransportServicesError::InvalidState(
                "Cannot add endpoints to a closed connection".to_string()
            )),
        }
    }

    /// Get the next event from the connection
    pub async fn next_event(&self) -> Option<ConnectionEvent> {
        let mut receiver = self.event_receiver.write().await;
        receiver.recv().await
    }
    
    /// Internal method to establish TCP connection
    pub(crate) async fn establish_tcp(
        &self,
        addr: SocketAddr,
        connection_timeout: Option<Duration>,
    ) -> Result<()> {
        let timeout_duration = connection_timeout.unwrap_or(Duration::from_secs(30));
        
        match timeout(timeout_duration, TcpStream::connect(addr)).await {
            Ok(Ok(stream)) => {
                let mut inner = self.inner.write().await;
                inner.tcp_stream = Some(stream);
                inner.state = ConnectionState::Established;
                
                // Set local endpoint based on actual connection
                if let Ok(local_addr) = inner.tcp_stream.as_ref().unwrap().local_addr() {
                    inner.local_endpoint = Some(LocalEndpoint {
                        identifiers: vec![EndpointIdentifier::SocketAddress(local_addr)],
                    });
                }
                
                // Send any pending messages
                let pending = inner.pending_messages.drain(..).collect::<Vec<_>>();
                drop(inner); // Release lock before sending
                
                for msg in pending {
                    self.send(msg).await?;
                }
                
                // Signal Ready event
                let _ = self.event_sender.send(ConnectionEvent::Ready);
                Ok(())
            }
            Ok(Err(e)) => {
                let mut inner = self.inner.write().await;
                inner.state = ConnectionState::Closed;
                let _ = self.event_sender.send(ConnectionEvent::EstablishmentError(
                    format!("Failed to connect: {}", e)
                ));
                Err(TransportServicesError::EstablishmentFailed(e.to_string()))
            }
            Err(_) => {
                let mut inner = self.inner.write().await;
                inner.state = ConnectionState::Closed;
                let _ = self.event_sender.send(ConnectionEvent::EstablishmentError(
                    "Connection timeout".to_string()
                ));
                Err(TransportServicesError::Timeout)
            }
        }
    }

    /// Get local endpoint information
    pub async fn local_endpoint(&self) -> Option<LocalEndpoint> {
        let inner = self.inner.read().await;
        inner.local_endpoint.clone()
    }

    /// Get remote endpoint information
    pub async fn remote_endpoint(&self) -> Option<RemoteEndpoint> {
        let inner = self.inner.read().await;
        inner.remote_endpoint.clone()
    }

    /// Update connection properties
    pub async fn set_property(&self, _key: &str, _value: &str) -> Result<()> {
        // TODO: Implement property setting based on RFC Section 8.1
        Ok(())
    }

    /// Get connection properties
    pub async fn get_property(&self, _key: &str) -> Result<Option<String>> {
        // TODO: Implement property getting
        Ok(None)
    }

    // Internal method to update state
    #[allow(dead_code)]
    pub(crate) async fn set_state(&self, state: ConnectionState) {
        let mut inner = self.inner.write().await;
        inner.state = state;
        
        if state == ConnectionState::Established {
            let _ = self.event_sender.send(ConnectionEvent::Ready);
        }
    }
}
