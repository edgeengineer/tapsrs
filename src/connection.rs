//! Connection implementation for Transport Services
//! Based on RFC 9622 Section 3 (API Summary) and Section 8 (Managing Connections)

use crate::{
    Preconnection, ConnectionState, ConnectionEvent, Message, MessageContext,
    TransportProperties, LocalEndpoint, RemoteEndpoint, Result, TransportServicesError
};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};

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
    _transport_properties: TransportProperties,
}

impl Connection {
    /// Create a new Connection (internal use)
    pub(crate) fn new(preconnection: Preconnection, state: ConnectionState) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        Self {
            inner: Arc::new(RwLock::new(ConnectionInner {
                preconnection,
                state,
                local_endpoint: None,
                remote_endpoint: None,
                _transport_properties: TransportProperties::default(),
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
    pub async fn send(&self, _message: Message) -> Result<()> {
        let inner = self.inner.read().await;
        
        match inner.state {
            ConnectionState::Established => {
                // TODO: Implement actual message sending
                Ok(())
            }
            ConnectionState::Establishing => {
                // Queue message for sending after establishment
                // TODO: Implement message queueing
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