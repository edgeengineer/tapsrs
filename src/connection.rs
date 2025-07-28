//! Connection implementation for Transport Services
//! Based on RFC 9622 Section 3 (API Summary) and Section 8 (Managing Connections)

use crate::{
    Preconnection, ConnectionState, ConnectionEvent, Message, MessageContext,
    TransportProperties, LocalEndpoint, RemoteEndpoint, Result, TransportServicesError,
    EndpointIdentifier, ConnectionGroup, ConnectionGroupId, FramerStack,
    ConnectionProperties, ConnectionProperty, TimeoutValue, CommunicationDirection,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::time::timeout;
use socket2::Socket;


/// A Connection represents an instance of a transport Protocol Stack
/// on which data can be sent to and/or received from a Remote Endpoint
pub struct Connection {
    inner: Arc<RwLock<ConnectionInner>>,
    event_sender: mpsc::UnboundedSender<ConnectionEvent>,
    event_receiver: Arc<RwLock<mpsc::UnboundedReceiver<ConnectionEvent>>>,
}

pub(crate) struct ConnectionInner {
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
    // Connection group this connection belongs to
    connection_group: Option<Arc<ConnectionGroup>>,
    // Batching state
    batch_mode: bool,
    batched_messages: Vec<Message>,
    // Message ID counter
    next_message_id: Arc<AtomicU64>,
    // Message framers for this connection
    framers: FramerStack,
    // Receive buffer for incoming data
    receive_buffer: Vec<u8>,
    // Connection properties
    properties: ConnectionProperties,
    // Track if a Final message was sent
    final_message_sent: bool,
    // Track if a Final message was received
    final_message_received: bool,
}

impl Clone for Connection {
    fn clone(&self) -> Self {
        // When cloning a Connection, we don't want to affect the connection count
        // This is just cloning the handle, not creating a new connection
        Self {
            inner: Arc::clone(&self.inner),
            event_sender: self.event_sender.clone(),
            event_receiver: Arc::clone(&self.event_receiver),
        }
    }
}

impl Connection {
    
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
                connection_group: None,
                batch_mode: false,
                batched_messages: Vec::new(),
                next_message_id: Arc::new(AtomicU64::new(1)),
                framers: FramerStack::new(), // Will be populated from preconnection async
                receive_buffer: Vec::new(),
                properties: ConnectionProperties::new(),
                final_message_sent: false,
                final_message_received: false,
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
    pub async fn send(&self, mut message: Message) -> Result<()> {
        // Assign message ID if not already set
        if message.id().is_none() {
            let id = self.get_next_message_id().await;
            message = message.with_id(id);
        }
        
        // Check if message has expired
        if let Some(context) = message.send_context() {
            if let Some(expiry) = context.expiry {
                if Instant::now() >= expiry {
                    // Notify about expiration
                    let _ = self.event_sender.send(ConnectionEvent::Expired { 
                        message_id: message.id() 
                    });
                    return Err(TransportServicesError::MessageExpired);
                }
            }
        }
        
        let mut inner = self.inner.write().await;
        
        match inner.state {
            ConnectionState::Established => {
                if inner.batch_mode {
                    // Add to batch
                    inner.batched_messages.push(message);
                    Ok(())
                } else {
                    // Send immediately
                    drop(inner);
                    self.send_message_internal(message).await
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
    
    /// Internal method to actually send a message
    async fn send_message_internal(&self, message: Message) -> Result<()> {
        let mut inner = self.inner.write().await;
        
        // Check if this is a Final message
        if message.properties().final_message {
            inner.final_message_sent = true;
        }
        
        // Frame the message if framers are available
        let data_to_send = if !inner.framers.is_empty() {
            let context = MessageContext::new(); // Use MessageContext for framing
            inner.framers.frame_message(&message, &context).await?
        } else {
            message.data().to_vec()
        };
        
        if let Some(ref mut stream) = inner.tcp_stream {
            let message_id = message.id();
            let event_sender = self.event_sender.clone();
            
            // Send the message
            match stream.write_all(&data_to_send).await {
                Ok(_) => {
                    match stream.flush().await {
                        Ok(_) => {
                            // Notify successful send
                            let _ = event_sender.send(ConnectionEvent::Sent { 
                                message_id 
                            });
                            Ok(())
                        }
                        Err(e) => {
                            let _ = event_sender.send(ConnectionEvent::SendError { 
                                message_id,
                                error: e.to_string()
                            });
                            Err(TransportServicesError::SendFailed(e.to_string()))
                        }
                    }
                }
                Err(e) => {
                    let _ = event_sender.send(ConnectionEvent::SendError { 
                        message_id,
                        error: e.to_string()
                    });
                    Err(TransportServicesError::SendFailed(e.to_string()))
                }
            }
        } else {
            Err(TransportServicesError::InvalidState(
                "No active stream".to_string()
            ))
        }
    }
    
    /// Start batching messages
    /// RFC Section 9.2.4
    pub async fn start_batch(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.batch_mode = true;
        Ok(())
    }
    
    /// End batching and send all batched messages
    /// RFC Section 9.2.4
    pub async fn end_batch(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.batch_mode = false;
        let messages = inner.batched_messages.drain(..).collect::<Vec<_>>();
        drop(inner);
        
        // Send all batched messages
        for message in messages {
            self.send_message_internal(message).await?;
        }
        
        Ok(())
    }
    
    /// Get the next message ID
    async fn get_next_message_id(&self) -> u64 {
        let inner = self.inner.read().await;
        inner.next_message_id.fetch_add(1, Ordering::SeqCst)
    }
    
    /// Use length-prefix framer for messages
    pub async fn use_length_prefix_framer(&self) -> Result<()> {
        use crate::LengthPrefixFramer;
        let mut inner = self.inner.write().await;
        inner.framers.add_framer(Box::new(LengthPrefixFramer::new()));
        Ok(())
    }

    /// Receive messages from the connection
    /// RFC Section 9.3.1 - Enqueuing Receives
    pub async fn receive(&self) -> Result<(Message, MessageContext)> {
        self.receive_with_params(None, None).await
    }
    
    /// Receive messages with buffer management parameters
    /// RFC Section 9.3.1 - Enqueuing Receives
    /// 
    /// minIncompleteLength: Minimum number of bytes to deliver for a partial message
    /// maxLength: Maximum number of bytes to accept for a single message
    pub async fn receive_with_params(
        &self, 
        _min_incomplete_length: Option<usize>, 
        max_length: Option<usize>
    ) -> Result<(Message, MessageContext)> {
        let state = {
            let inner = self.inner.read().await;
            inner.state
        };
        
        match state {
            ConnectionState::Established | ConnectionState::Establishing => {
                // Keep reading until we have a complete message
                let mut buffer = [0u8; 8192];
                
                loop {
                    // Check if we have a complete message in the buffer already
                    let (has_complete_message, result) = {
                        let mut inner = self.inner.write().await;
                        
                        if inner.receive_buffer.is_empty() {
                            (false, None)
                        } else if !inner.framers.is_empty() {
                            // Use the framer to parse - we need to manually check for complete messages
                            // Since we have length-prefix framing, let's implement simple length-prefix parsing here
                            if inner.receive_buffer.len() >= 4 {
                                let len_bytes = &inner.receive_buffer[0..4];
                                let expected_len = u32::from_be_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;
                                
                                if inner.receive_buffer.len() >= 4 + expected_len {
                                    // We have a complete message
                                    let message_data = &inner.receive_buffer[4..4 + expected_len];
                                    let message = Message::from_bytes(message_data);
                                    let mut context = MessageContext::new();
                                    // Set remote endpoint if available
                                    context.remote_endpoint = inner.remote_endpoint.clone();
                                    
                                    // Remove the processed message from buffer
                                    inner.receive_buffer.drain(..4 + expected_len);
                                    
                                    // Check max_length constraint
                                    if let Some(max_len) = max_length {
                                        if message.data().len() > max_len {
                                            (true, Some(Err(TransportServicesError::MessageTooLarge(format!(
                                                "Message size {} exceeds max length {}", 
                                                message.data().len(), 
                                                max_len
                                            )))))
                                        } else {
                                            (true, Some(Ok((message, context))))
                                        }
                                    } else {
                                        (true, Some(Ok((message, context))))
                                    }
                                } else {
                                    (false, None)
                                }
                            } else {
                                (false, None)
                            }
                        } else {
                            // No framers - return all buffered data as one message
                            let message = Message::from_bytes(&inner.receive_buffer);
                            let mut context = MessageContext::new();
                            // Set remote endpoint if available
                            context.remote_endpoint = inner.remote_endpoint.clone();
                            inner.receive_buffer.clear();
                            (true, Some(Ok((message, context))))
                        }
                    };
                    
                    if has_complete_message {
                        if let Some(result) = result {
                            match result {
                                Ok((message, context)) => {
                                    // Send Received event
                                    let _ = self.event_sender.send(ConnectionEvent::Received {
                                        message_data: message.data().to_vec(),
                                        message_context: context.clone(),
                                    });
                                    
                                    return Ok((message, context));
                                }
                                Err(e) => return Err(e),
                            }
                        }
                    }
                    
                    // No complete message yet - read more data
                    let read_result = {
                        let mut inner = self.inner.write().await;
                        if let Some(ref mut stream) = inner.tcp_stream {
                            stream.read(&mut buffer).await
                        } else {
                            return Err(TransportServicesError::InvalidState("No active stream".to_string()));
                        }
                    };
                    
                    match read_result {
                        Ok(0) => {
                            // Connection closed by peer
                            let mut inner = self.inner.write().await;
                            inner.state = ConnectionState::Closed;
                            let _ = self.event_sender.send(ConnectionEvent::Closed);
                            return Err(TransportServicesError::ConnectionFailed("Connection closed by peer".to_string()));
                        }
                        Ok(n) => {
                            // Add data to receive buffer
                            let mut inner = self.inner.write().await;
                            inner.receive_buffer.extend_from_slice(&buffer[..n]);
                            // Continue loop to try parsing again
                        }
                        Err(e) => {
                            let _ = self.event_sender.send(ConnectionEvent::ReceiveError {
                                error: e.to_string(),
                            });
                            return Err(TransportServicesError::ReceiveFailed(e.to_string()));
                        }
                    }
                }
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
                
                // If this connection is part of a group, decrement the connection count
                if let Some(ref group) = inner.connection_group {
                    group.remove_connection();
                }
                
                // Send any pending batched messages before closing
                let batched_messages = inner.batched_messages.drain(..).collect::<Vec<_>>();
                
                // Perform graceful close on TCP stream
                if let Some(ref mut stream) = inner.tcp_stream {
                    // Flush any buffered data
                    let _ = stream.flush().await;
                    
                    // Shutdown the write side to signal we're done sending
                    // This sends a TCP FIN packet
                    let _ = stream.shutdown().await;
                }
                
                // Drop the write lock to send batched messages
                drop(inner);
                
                // Send any remaining batched messages
                for message in batched_messages {
                    let _ = self.send_message_internal(message).await;
                }
                
                // Re-acquire lock to update state
                let mut inner = self.inner.write().await;
                inner.state = ConnectionState::Closed;
                
                // Clear any remaining state
                inner.pending_messages.clear();
                inner.receive_buffer.clear();
                inner.tcp_stream = None;
                
                let _ = self.event_sender.send(ConnectionEvent::Closed);
                Ok(())
            }
            ConnectionState::Closing => {
                // Already closing, just wait
                Ok(())
            }
            ConnectionState::Closed => Ok(()), // Already closed
        }
    }

    /// Abort the connection immediately
    /// RFC Section 10 - Connection Termination
    /// 
    /// Unlike close(), abort() immediately terminates the connection without
    /// attempting to deliver any outstanding data.
    pub async fn abort(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        
        // Only proceed if we're not already closed
        let was_not_closed = inner.state != ConnectionState::Closed;
        if !was_not_closed {
            return Ok(());
        }
        
        // Immediately set state to Closed
        inner.state = ConnectionState::Closed;
        
        // Force close the TCP stream if it exists
        if let Some(stream) = inner.tcp_stream.take() {
            // Drop the stream to force immediate closure
            // This will send a TCP RST instead of graceful FIN
            drop(stream);
        }
        
        // Clear any pending messages since we're aborting
        inner.pending_messages.clear();
        inner.batched_messages.clear();
        inner.receive_buffer.clear();
        
        // If this connection is part of a group, decrement the connection count
        if let Some(ref group) = inner.connection_group {
            group.remove_connection();
        }
        
        // Send ConnectionError event for abort (as per RFC Section 10)
        let _ = self.event_sender.send(ConnectionEvent::ConnectionError(
            "Connection aborted".to_string()
        ));
        
        Ok(())
    }

    /// Clone the connection to create a new connection in the same group
    /// RFC Section 7.4
    pub async fn clone_connection(&self) -> Result<Connection> {
        let inner = self.inner.read().await;
        
        match inner.state {
            ConnectionState::Established => {
                // Get or create connection group
                let (group, was_not_grouped) = if let Some(ref group) = inner.connection_group {
                    (Arc::clone(group), false)
                } else {
                    // Create a new connection group for this connection
                    let new_group = Arc::new(ConnectionGroup::new(
                        inner.transport_properties.clone(),
                        inner.local_endpoint.as_ref().map(|e| vec![e.clone()]).unwrap_or_default(),
                        inner.remote_endpoint.as_ref().map(|e| vec![e.clone()]).unwrap_or_default(),
                    ));
                    (new_group, true)
                };
                
                // Get preconnection before dropping inner
                let preconn = inner.preconnection.clone();
                drop(inner);
                
                // Update the original connection to be part of the group if it wasn't already
                if was_not_grouped {
                    let mut inner_mut = self.inner.write().await;
                    inner_mut.connection_group = Some(Arc::clone(&group));
                    drop(inner_mut);
                    // Add the original connection to the group
                    group.add_connection();
                    // Register this connection with the group
                    group.register_connection(Arc::downgrade(&self.inner)).await;
                }
                
                // Create a new connection in the same group
                let new_conn = preconn.initiate().await?;
                
                // Set the connection group on the new connection
                {
                    let mut new_inner = new_conn.inner.write().await;
                    new_inner.connection_group = Some(Arc::clone(&group));
                    
                    // Share transport properties from the group
                    let shared_props = group.transport_properties.read().await;
                    new_inner.transport_properties = shared_props.clone();
                }
                
                // Increment connection count for the new connection
                group.add_connection();
                // Register the new connection with the group
                group.register_connection(Arc::downgrade(&new_conn.inner)).await;
                
                Ok(new_conn)
            }
            _ => Err(TransportServicesError::InvalidState(
                "Can only clone established connections".to_string()
            )),
        }
    }

    /// Add a remote endpoint to the connection
    /// RFC Section 7.5
    pub async fn add_remote(&self, endpoint: RemoteEndpoint) -> Result<()> {
        let mut inner = self.inner.write().await;
        
        match inner.state {
            ConnectionState::Established | ConnectionState::Establishing => {
                // For single-path TCP connections, we can only have one remote endpoint
                // In a real implementation with multipath support (like MPTCP or QUIC),
                // we would add this to a list of available endpoints
                
                // Check if this is the same endpoint we already have
                if let Some(ref current_remote) = inner.remote_endpoint {
                    // Check if any identifiers match
                    for new_id in &endpoint.identifiers {
                        for existing_id in &current_remote.identifiers {
                            if new_id == existing_id {
                                // Endpoint already known, ignore as per RFC
                                return Ok(());
                            }
                        }
                    }
                }
                
                // For now, since we only support single-path TCP, we can only
                // update the remote endpoint if we don't have an established connection yet
                if inner.state == ConnectionState::Establishing && inner.tcp_stream.is_none() {
                    // Update the remote endpoint for future connection attempts
                    inner.remote_endpoint = Some(endpoint);
                    Ok(())
                } else {
                    // Log that we received the endpoint but can't use it with current transport
                    // In a multipath implementation, we would:
                    // 1. Store this endpoint in a list
                    // 2. Potentially establish a new subflow to this endpoint
                    // 3. Update routing tables
                    
                    // For now, we just acknowledge receipt but don't use it
                    Ok(())
                }
            }
            _ => Err(TransportServicesError::InvalidState(
                "Cannot add endpoints to a closed connection".to_string()
            )),
        }
    }

    /// Add a local endpoint to the connection
    pub async fn add_local(&self, endpoint: LocalEndpoint) -> Result<()> {
        let mut inner = self.inner.write().await;
        
        match inner.state {
            ConnectionState::Established | ConnectionState::Establishing => {
                // For single-path TCP connections, we can only have one local endpoint
                // In a real implementation with multipath support (like MPTCP or QUIC),
                // we would add this to a list of available endpoints
                
                // Check if this is the same endpoint we already have
                if let Some(ref current_local) = inner.local_endpoint {
                    // Check if any identifiers match
                    for new_id in &endpoint.identifiers {
                        for existing_id in &current_local.identifiers {
                            if new_id == existing_id {
                                // Endpoint already known, ignore
                                return Ok(());
                            }
                        }
                    }
                }
                
                // For now, since we only support single-path TCP, we can only
                // update the local endpoint if we don't have an established connection yet
                if inner.state == ConnectionState::Establishing && inner.tcp_stream.is_none() {
                    // Update the local endpoint for future connection attempts
                    inner.local_endpoint = Some(endpoint);
                    Ok(())
                } else {
                    // In a multipath implementation, we would:
                    // 1. Store this endpoint in a list
                    // 2. Potentially bind a new socket to this endpoint
                    // 3. Use it for new subflows
                    
                    // For now, we just acknowledge receipt but don't use it
                    Ok(())
                }
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
                    // Use send_message_internal to avoid re-queuing
                    self.send_message_internal(msg).await?;
                }
                
                // TODO: Start background reading task (disabled for now to avoid deadlocks)
                // self.start_reading_task().await?;
                
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

    /// Set a connection property
    /// RFC Section 8: Connection.SetProperty(property, value)
    pub async fn set_property(&self, key: &str, value: ConnectionProperty) -> Result<()> {
        let mut inner = self.inner.write().await;
        
        // For properties in a connection group, update all connections
        if let Some(ref group) = inner.connection_group {
            // connPriority is not shared across the group (per RFC)
            if key != "connPriority" {
                // Clone the group reference and value to avoid holding locks
                let group_clone = Arc::clone(group);
                let key_clone = key.to_string();
                let value_clone = value.clone();
                
                // Release the lock before updating other connections
                drop(inner);
                
                // Get all connections in the group
                let connections = group_clone.get_connections().await;
                
                // Update property on all connections in parallel
                let mut update_tasks = Vec::new();
                for conn_inner in connections {
                    let key = key_clone.clone();
                    let val = value_clone.clone();
                    let task = tokio::spawn(async move {
                        let mut inner = conn_inner.write().await;
                        let _ = inner.properties.set(&key, val);
                    });
                    update_tasks.push(task);
                }
                
                // Wait for all updates
                for task in update_tasks {
                    let _ = task.await;
                }
                
                // Re-acquire lock to update this connection
                inner = self.inner.write().await;
            }
        }
        
        // Set the property on this connection
        inner.properties.set(key, value.clone())?;
        
        // Apply property changes that need immediate action
        match key {
            "connTimeout" => {
                // RFC 8.2.3: tcp.userTimeoutChangeable becomes false when connTimeout is used
                if matches!(value, ConnectionProperty::ConnTimeout(TimeoutValue::Duration(_))) {
                    inner.properties.set("tcp.userTimeoutChangeable", 
                        ConnectionProperty::TcpUserTimeoutChangeable(false))?;
                }
                
                // Apply timeout to underlying TCP stream
                if let (Some(ref _stream), ConnectionProperty::ConnTimeout(timeout_val)) = (&inner.tcp_stream, &value) {
                    match timeout_val {
                        TimeoutValue::Duration(duration) => {
                            // TCP connection timeout is handled at the application level
                            // Store for future operations
                            log::debug!("Connection timeout set to {:?}", duration);
                        }
                        TimeoutValue::Disabled => {
                            log::debug!("Connection timeout disabled");
                        }
                    }
                }
            }
            "keepAliveTimeout" => {
                // Configure keep-alive on TCP stream
                if let Some(ref stream) = inner.tcp_stream {
                    if let ConnectionProperty::KeepAliveTimeout(timeout_val) = &value {
                        // Get the raw socket to set keep-alive options
                        #[cfg(unix)]
                        {
                            use std::os::unix::io::{AsRawFd, FromRawFd};
                            use socket2::{Socket, TcpKeepalive};
                            
                            let fd = stream.as_raw_fd();
                            let socket = unsafe { Socket::from_raw_fd(fd) };
                            
                            match timeout_val {
                                TimeoutValue::Duration(duration) => {
                                    // Enable keep-alive with the specified interval
                                    let keepalive = TcpKeepalive::new()
                                        .with_time(*duration)
                                        .with_interval(*duration);
                                    
                                    if let Err(e) = socket.set_tcp_keepalive(&keepalive) {
                                        log::warn!("Failed to set TCP keep-alive: {}", e);
                                    } else {
                                        log::debug!("TCP keep-alive set to {:?}", duration);
                                    }
                                }
                                TimeoutValue::Disabled => {
                                    // Disable keep-alive
                                    if let Err(e) = socket.set_keepalive(false) {
                                        log::warn!("Failed to disable TCP keep-alive: {}", e);
                                    } else {
                                        log::debug!("TCP keep-alive disabled");
                                    }
                                }
                            }
                            
                            // Important: forget the socket to prevent double-close
                            std::mem::forget(socket);
                        }
                        
                        #[cfg(not(unix))]
                        {
                            log::warn!("TCP keep-alive configuration not supported on this platform");
                        }
                    }
                }
            }
            "tcp.userTimeoutEnabled" => {
                // Configure TCP User Timeout Option if supported
                if let Some(ref _stream) = inner.tcp_stream {
                    if let ConnectionProperty::TcpUserTimeoutEnabled(enabled) = &value {
                        // Note: Actually setting TCP_USER_TIMEOUT socket option would require
                        // platform-specific code and may not be available on all platforms
                        log::debug!("TCP User Timeout enabled: {}", enabled);
                    }
                }
            }
            "tcp.userTimeoutValue" => {
                // Set the TCP User Timeout value
                if let Some(ref _stream) = inner.tcp_stream {
                    if let ConnectionProperty::TcpUserTimeoutValue(Some(duration)) = &value {
                        // Note: Actually setting TCP_USER_TIMEOUT socket option would require
                        // platform-specific code and may not be available on all platforms
                        log::debug!("TCP User Timeout value set to: {:?}", duration);
                    }
                }
            }
            _ => {}
        }
        
        Ok(())
    }

    /// Get all connection properties
    /// RFC Section 8: ConnectionProperties := Connection.GetProperties()
    pub async fn get_properties(&self) -> ConnectionProperties {
        let inner = self.inner.read().await;
        let mut props = inner.properties.clone();
        
        // Get the transport properties to check direction
        let direction = inner.transport_properties.selection_properties.direction;
        
        // RFC 8.1.11.2: Can Send Data
        // Check against direction Selection Property and Final message state
        let can_send = match inner.state {
            ConnectionState::Established => {
                // Can't send if:
                // 1. Direction is unidirectional receive, or
                // 2. A Final message was already sent
                match direction {
                    CommunicationDirection::UnidirectionalReceive => false,
                    _ => !inner.final_message_sent,
                }
            }
            ConnectionState::Establishing => false, // Could buffer, but say false for now
            _ => false,
        };
        
        // RFC 8.1.11.3: Can Receive Data
        // Check against direction Selection Property and Final message state
        let can_receive = match inner.state {
            ConnectionState::Established => {
                // Can't receive if:
                // 1. Direction is unidirectional send, or
                // 2. A Final message was already received (implementation specific)
                match direction {
                    CommunicationDirection::UnidirectionalSend => false,
                    _ => !inner.final_message_received,
                }
            }
            _ => false,
        };
        
        // Update the basic read-only properties
        props.update_readonly(inner.state, can_send, can_receive);
        
        // Update MTU-related properties if we have a TCP stream
        if let Some(ref stream) = inner.tcp_stream {
            // RFC 8.1.11.4: Maximum Message Size Before Fragmentation
            // Query actual MSS from socket
            let mss = self.get_tcp_mss(stream).await.unwrap_or(1460); // Default to typical value if query fails
            
            props.properties.insert("singularTransmissionMsgMaxLen".to_string(),
                ConnectionProperty::SingularTransmissionMsgMaxLen(Some(mss)));
            
            // RFC 8.1.11.5: Maximum Message Size on Send
            // For TCP, there's no inherent limit (streaming protocol)
            // Return 0 if sending is not possible
            let send_msg_max = if can_send {
                None // No limit for TCP
            } else {
                Some(0) // Cannot send
            };
            props.properties.insert("sendMsgMaxLen".to_string(),
                ConnectionProperty::SendMsgMaxLen(send_msg_max));
            
            // RFC 8.1.11.6: Maximum Message Size on Receive  
            // For TCP, there's no inherent limit (streaming protocol)
            // Return 0 if receiving is not possible
            let recv_msg_max = if can_receive {
                None // No limit for TCP
            } else {
                Some(0) // Cannot receive
            };
            props.properties.insert("recvMsgMaxLen".to_string(),
                ConnectionProperty::RecvMsgMaxLen(recv_msg_max));
        } else {
            // No stream - set appropriate values based on connection state
            if inner.state == ConnectionState::Establishing {
                // Don't add properties for connections that haven't been established yet
                // This matches the expectation of test_mss_property_not_set_before_connection
            } else {
                // For closed connections or other states, add properties with 0 values
                props.properties.insert("singularTransmissionMsgMaxLen".to_string(),
                    ConnectionProperty::SingularTransmissionMsgMaxLen(None)); // Not applicable
                props.properties.insert("sendMsgMaxLen".to_string(),
                    ConnectionProperty::SendMsgMaxLen(Some(0))); // Cannot send without stream
                props.properties.insert("recvMsgMaxLen".to_string(),
                    ConnectionProperty::RecvMsgMaxLen(Some(0))); // Cannot receive without stream
            }
        }
        
        props
    }
    
    /// Get a specific connection property value
    pub async fn get_property(&self, key: &str) -> Option<ConnectionProperty> {
        let props = self.get_properties().await;
        props.get(key).cloned()
    }
    
    /// Get the connection group ID if this connection is part of a group
    pub async fn connection_group_id(&self) -> Option<ConnectionGroupId> {
        let inner = self.inner.read().await;
        inner.connection_group.as_ref().map(|g| g.id)
    }
    
    /// Check if this connection is part of a connection group
    pub async fn is_grouped(&self) -> bool {
        let inner = self.inner.read().await;
        inner.connection_group.is_some()
    }
    
    /// Get the number of connections in this connection's group
    pub async fn group_connection_count(&self) -> Option<u64> {
        let inner = self.inner.read().await;
        inner.connection_group.as_ref().map(|g| g.connection_count())
    }
    
    /// Close all connections in the group
    /// RFC Section 10
    pub async fn close_group(&self) -> Result<()> {
        let inner = self.inner.read().await;
        
        if let Some(ref group) = inner.connection_group {
            // Get all connections in the group
            let connections = group.get_connections().await;
            drop(inner); // Release lock before closing connections
            
            // Close all connections in parallel
            let mut close_tasks = Vec::new();
            for conn_inner in connections {
                let task = tokio::spawn(async move {
                    let mut inner = conn_inner.write().await;
                    
                    match inner.state {
                        ConnectionState::Established | ConnectionState::Establishing => {
                            inner.state = ConnectionState::Closing;
                            
                            // Clear any pending batched messages before closing
                            inner.batched_messages.clear();
                            
                            // Perform graceful close on TCP stream
                            if let Some(ref mut stream) = inner.tcp_stream {
                                let _ = stream.flush().await;
                                let _ = stream.shutdown().await;
                            }
                            
                            inner.state = ConnectionState::Closed;
                            inner.pending_messages.clear();
                            inner.receive_buffer.clear();
                            inner.tcp_stream = None;
                            
                            // Note: We don't decrement connection count here as it's handled by each connection
                        }
                        _ => {} // Already closing or closed
                    }
                });
                close_tasks.push(task);
            }
            
            // Wait for all connections to close
            for task in close_tasks {
                let _ = task.await;
            }
            
            // Send Closed event for this connection
            let _ = self.event_sender.send(ConnectionEvent::Closed);
            Ok(())
        } else {
            // No group, just close this connection
            self.close().await
        }
    }
    
    /// Abort all connections in the group
    pub async fn abort_group(&self) -> Result<()> {
        let inner = self.inner.read().await;
        
        if let Some(ref group) = inner.connection_group {
            // Get all connections in the group
            let connections = group.get_connections().await;
            drop(inner); // Release lock before aborting connections
            
            // Abort all connections in parallel
            let mut abort_tasks = Vec::new();
            for conn_inner in connections {
                let task = tokio::spawn(async move {
                    let mut inner = conn_inner.write().await;
                    
                    let was_not_closed = inner.state != ConnectionState::Closed;
                    if was_not_closed {
                        // Immediately set state to Closed
                        inner.state = ConnectionState::Closed;
                        
                        // Force close the TCP stream
                        if let Some(stream) = inner.tcp_stream.take() {
                            drop(stream); // This sends TCP RST
                        }
                        
                        // Clear all buffers
                        inner.pending_messages.clear();
                        inner.batched_messages.clear();
                        inner.receive_buffer.clear();
                    }
                });
                abort_tasks.push(task);
            }
            
            // Wait for all connections to abort
            for task in abort_tasks {
                let _ = task.await;
            }
            
            // Send ConnectionError event for this connection
            let _ = self.event_sender.send(ConnectionEvent::ConnectionError(
                "Connection group aborted".to_string()
            ));
            Ok(())
        } else {
            // No group, just abort this connection
            self.abort().await
        }
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
    
    // Internal method to set TCP stream (for listener)
    pub(crate) async fn set_tcp_stream(&mut self, stream: TcpStream) {
        let mut inner = self.inner.write().await;
        inner.tcp_stream = Some(stream);
        inner.state = ConnectionState::Established;
        drop(inner);
        
        // TODO: Start background reading task (disabled for now to avoid deadlocks)
        // let _ = self.start_reading_task().await;
        
        let _ = self.event_sender.send(ConnectionEvent::Ready);
    }
    
    /// Get the TCP Maximum Segment Size (MSS) from a TcpStream
    async fn get_tcp_mss(&self, stream: &TcpStream) -> Result<usize> {
        #[cfg(unix)]
        {
            use std::os::unix::io::{AsRawFd, FromRawFd};
            
            // Get the raw file descriptor from the TcpStream
            let fd = stream.as_raw_fd();
            
            // Create a Socket from the raw fd
            // SAFETY: We're borrowing the fd from TcpStream, not taking ownership
            let socket = unsafe { Socket::from_raw_fd(fd) };
            
            // Get the MSS value
            #[cfg(not(target_os = "redox"))]
            let mss_result = socket.mss();
            
            #[cfg(target_os = "redox")]
            let mss_result = Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "TCP MSS query not supported on Redox"
            ));
            
            // Important: We must forget the socket to prevent it from closing the fd
            // when it goes out of scope (since we don't own the fd)
            std::mem::forget(socket);
            
            match mss_result {
                Ok(mss) => Ok(mss as usize),
                Err(e) => {
                    // Log the error but don't fail
                    log::debug!("Failed to get TCP MSS: {}", e);
                    Err(TransportServicesError::NotSupported(format!("Failed to get TCP MSS: {}", e)))
                }
            }
        }
        
        #[cfg(not(unix))]
        {
            // On non-Unix platforms, return a typical default value
            Ok(1460)
        }
    }
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection")
            .field("state", &"<async>")
            .finish()
    }
}

// Note: We don't implement Drop for Connection because Connection can be cloned
// (it's just a handle to the actual connection). The connection count should only
// be decremented when the actual connection is closed, not when a handle is dropped.
