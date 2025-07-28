//! Listener implementation for TAPS
//! Based on RFC 9622 Section 7.2 (Passive Open: Listen)

use crate::{
    Preconnection, Connection, Result, TapsError
};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};

/// A Listener waits for incoming Connections from Remote Endpoints
pub struct Listener {
    inner: Arc<RwLock<ListenerInner>>,
    connection_receiver: mpsc::UnboundedReceiver<Connection>,
    stop_sender: mpsc::Sender<()>,
}

struct ListenerInner {
    preconnection: Preconnection,
    active: bool,
    connection_sender: mpsc::UnboundedSender<Connection>,
}

impl Listener {
    /// Create a new Listener (internal use)
    pub(crate) fn new(preconnection: Preconnection) -> Self {
        let (connection_sender, connection_receiver) = mpsc::unbounded_channel();
        let (stop_sender, mut stop_receiver) = mpsc::channel(1);
        
        let inner = Arc::new(RwLock::new(ListenerInner {
            preconnection: preconnection.clone(),
            active: true,
            connection_sender,
        }));

        // Spawn a task to handle incoming connections
        let inner_clone = inner.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = stop_receiver.recv() => {
                        break;
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                        // TODO: Check for incoming connections
                        // This is where we would accept connections from the underlying transport
                    }
                }
            }
            
            // Mark as inactive when stopped
            let mut inner = inner_clone.write().await;
            inner.active = false;
        });

        Self {
            inner,
            connection_receiver,
            stop_sender,
        }
    }

    /// Accept the next incoming connection
    pub async fn accept(&mut self) -> Result<Connection> {
        match self.connection_receiver.recv().await {
            Some(connection) => Ok(connection),
            None => Err(TapsError::InvalidState("Listener closed".to_string())),
        }
    }

    /// Stop listening for new connections
    pub async fn stop(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        if inner.active {
            inner.active = false;
            let _ = self.stop_sender.send(()).await;
            Ok(())
        } else {
            Ok(())
        }
    }

    /// Check if the listener is still active
    pub async fn is_active(&self) -> bool {
        let inner = self.inner.read().await;
        inner.active
    }

    /// Get the preconnection this listener was created from
    pub async fn preconnection(&self) -> Preconnection {
        let inner = self.inner.read().await;
        inner.preconnection.clone()
    }

    // Internal method to inject a new connection (for testing/implementation)
    #[doc(hidden)]
    pub async fn inject_connection(&self, connection: Connection) -> Result<()> {
        let inner = self.inner.read().await;
        if inner.active {
            inner.connection_sender.send(connection)
                .map_err(|_| TapsError::InvalidState("Listener channel closed".to_string()))?;
            Ok(())
        } else {
            Err(TapsError::InvalidState("Listener is not active".to_string()))
        }
    }
}