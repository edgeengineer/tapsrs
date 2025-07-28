//! Connection implementation for Transport Services
//! Based on RFC 9622 Section 3 (API Summary) and Section 8 (Managing Connections)

use crate::{
    Preconnection, ConnectionState, ConnectionEvent, Message, MessageContext,
    TransportProperties, LocalEndpoint, RemoteEndpoint, Result, TransportServicesError,
    EndpointIdentifier, ConnectionGroup, ConnectionGroupId, FramerStack,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::time::timeout;

/// A receive request queued for processing
struct ReceiveRequest {
    min_incomplete_length: Option<usize>,
    max_length: Option<usize>,
    response_channel: tokio::sync::oneshot::Sender<Result<(Message, MessageContext)>>,
}

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
    // Queue of pending receive requests
    receive_queue: Vec<ReceiveRequest>,
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
                connection_group: None,
                batch_mode: false,
                batched_messages: Vec::new(),
                next_message_id: Arc::new(AtomicU64::new(1)),
                framers: FramerStack::new(),
                receive_buffer: Vec::new(),
                receive_queue: Vec::new(),
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
                connection_group: None,
                batch_mode: false,
                batched_messages: Vec::new(),
                next_message_id: Arc::new(AtomicU64::new(1)),
                framers: FramerStack::new(), // Will be populated from preconnection async
                receive_buffer: Vec::new(),
                receive_queue: Vec::new(),
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
        min_incomplete_length: Option<usize>, 
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
    
    /// Process the receive queue with available data
    async fn process_receive_queue(&self) -> Result<()> {
        loop {
            let (has_request, has_data) = {
                let inner = self.inner.read().await;
                (!inner.receive_queue.is_empty(), !inner.receive_buffer.is_empty())
            };
            
            if !has_request || !has_data {
                break;
            }
            
            let mut inner = self.inner.write().await;
            if inner.receive_queue.is_empty() {
                break;
            }
            
            // Try to parse complete messages from buffer using framers
            let parsed_messages = if !inner.framers.is_empty() {
                inner.framers.parse_data(&inner.receive_buffer).await?
            } else {
                // No framers - treat entire buffer as one message if we have data
                if !inner.receive_buffer.is_empty() {
                    let message = Message::from_bytes(&inner.receive_buffer);
                    let context = MessageContext::new();
                    inner.receive_buffer.clear();
                    vec![(message, context)]
                } else {
                    Vec::new()
                }
            };
            
            if parsed_messages.is_empty() {
                // No complete messages yet, check if we can deliver partial data
                if let Some(request) = inner.receive_queue.first() {
                    if let Some(min_len) = request.min_incomplete_length {
                        if inner.receive_buffer.len() >= min_len {
                            // Deliver partial message
                            let request = inner.receive_queue.remove(0);
                            let max_len = request.max_length.unwrap_or(inner.receive_buffer.len());
                            let data_len = std::cmp::min(max_len, inner.receive_buffer.len());
                            
                            let data = inner.receive_buffer.drain(..data_len).collect::<Vec<u8>>();
                            let message = Message::from_bytes(&data);
                            let context = MessageContext::new();
                            
                            // Send ReceivedPartial event
                            let _ = self.event_sender.send(ConnectionEvent::ReceivedPartial {
                                message_data: data.clone(),
                                message_context: context.clone(),
                                end_of_message: false,
                            });
                            
                            let _ = request.response_channel.send(Ok((message, context)));
                        }
                    }
                }
                break;
            }
            
            // Deliver complete messages
            for (message, context) in parsed_messages {
                if inner.receive_queue.is_empty() {
                    break;
                }
                
                let request = inner.receive_queue.remove(0);
                
                // Check max_length constraint
                if let Some(max_len) = request.max_length {
                    if message.data().len() > max_len {
                        let _ = request.response_channel.send(Err(
                            TransportServicesError::MessageTooLarge(format!(
                                "Message size {} exceeds max length {}", 
                                message.data().len(), 
                                max_len
                            ))
                        ));
                        continue;
                    }
                }
                
                // Send Received event
                let _ = self.event_sender.send(ConnectionEvent::Received {
                    message_data: message.data().to_vec(),
                    message_context: context.clone(),
                });
                
                let _ = request.response_channel.send(Ok((message, context)));
            }
        }
        
        Ok(())
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
                
                // TODO: Implement graceful close
                inner.state = ConnectionState::Closed;
                let _ = self.event_sender.send(ConnectionEvent::Closed);
                Ok(())
            }
            ConnectionState::Closing => {
                // Wait for close to complete
                Ok(())
            }
            ConnectionState::Closed => Ok(()), // Already closed
        }
    }

    /// Abort the connection immediately
    pub async fn abort(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        
        // Only decrement if we're moving from a non-closed state
        let was_not_closed = inner.state != ConnectionState::Closed;
        inner.state = ConnectionState::Closed;
        
        // If this connection is part of a group and wasn't already closed
        if was_not_closed {
            if let Some(ref group) = inner.connection_group {
                group.remove_connection();
            }
        }
        
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
                
                Ok(new_conn)
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
        // TODO: Implement group-wide close
        // For now, just close this connection
        self.close().await
    }
    
    /// Abort all connections in the group
    pub async fn abort_group(&self) -> Result<()> {
        // TODO: Implement group-wide abort
        // For now, just abort this connection
        self.abort().await
    }
    
    /// Start the background TCP reading task
    async fn start_reading_task(&self) -> Result<()> {
        let connection = self.clone();
        
        tokio::spawn(async move {
            let mut buffer = [0u8; 8192];
            
            loop {
                // Check if connection is still active
                let (has_stream, is_active) = {
                    let inner = connection.inner.read().await;
                    (inner.tcp_stream.is_some(), 
                     inner.state == ConnectionState::Established || inner.state == ConnectionState::Establishing)
                };
                
                if !has_stream || !is_active {
                    break;
                }
                
                // Read data from TCP stream - clone the stream to avoid holding the lock
                let stream_clone = {
                    let inner = connection.inner.read().await;
                    if let Some(ref stream) = inner.tcp_stream {
                        // Create a reference we can use - we need to restructure this to avoid deadlock
                        true
                    } else {
                        false
                    }
                };
                
                if !stream_clone {
                    break;
                }
                
                // We need to approach this differently to avoid deadlocks
                // For now, we'll use a simpler approach with a timeout to avoid holding locks
                let read_result = tokio::time::timeout(Duration::from_millis(100), async {
                    let mut inner = connection.inner.write().await;
                    if let Some(ref mut stream) = inner.tcp_stream {
                        stream.read(&mut buffer).await
                    } else {
                        Ok(0)
                    }
                }).await;
                
                let read_result = match read_result {
                    Ok(result) => result,
                    Err(_) => continue, // Timeout, try again
                };
                
                match read_result {
                    Ok(0) => {
                        // Connection closed by peer
                        let _ = connection.event_sender.send(ConnectionEvent::Closed);
                        break;
                    }
                    Ok(n) => {
                        // Add data to receive buffer
                        {
                            let mut inner = connection.inner.write().await;
                            inner.receive_buffer.extend_from_slice(&buffer[..n]);
                        }
                        
                        // Process any pending receive requests
                        if let Err(e) = connection.process_receive_queue().await {
                            let _ = connection.event_sender.send(ConnectionEvent::ReceiveError {
                                error: e.to_string(),
                            });
                        }
                    }
                    Err(e) => {
                        // Read error
                        let _ = connection.event_sender.send(ConnectionEvent::ReceiveError {
                            error: e.to_string(),
                        });
                        break;
                    }
                }
            }
        });
        
        Ok(())
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
