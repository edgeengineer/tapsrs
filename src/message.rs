//! Message implementation for Transport Services
//! Based on RFC 9622 Section 9.1 (Messages and Framers)

use crate::{MessageProperties, MessageCapacityProfile, LocalEndpoint, RemoteEndpoint};
use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::mpsc;

/// A Message is the unit of data transfer in TAPS
#[derive(Debug, Clone)]
pub struct Message {
    /// The actual data payload
    data: Vec<u8>,
    
    /// Properties specific to this message
    properties: MessageProperties,
    
    /// Optional message identifier
    id: Option<u64>,
    
    /// Whether this message completes the application-layer message
    /// RFC Section 9.2.3: Partial Sends
    end_of_message: bool,
    
    /// Optional context for sending
    send_context: Option<SendContext>,
}

/// Context for sending messages
#[derive(Debug, Clone)]
pub struct SendContext {
    /// Expiry time for the message
    pub expiry: Option<Instant>,
    
    /// Whether to bundle this message with others
    pub bundle: bool,
    
    /// Event notifier for send completion
    pub completion_notifier: Option<Arc<mpsc::UnboundedSender<SendEvent>>>,
}

/// Events related to message sending
#[derive(Debug, Clone)]
pub enum SendEvent {
    /// Message was successfully sent
    Sent { message_id: Option<u64> },
    
    /// Message expired before it could be sent
    Expired { message_id: Option<u64> },
    
    /// An error occurred while sending
    SendError { message_id: Option<u64>, error: String },
}

impl Message {
    /// Create a new message with data
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            properties: MessageProperties::default(),
            id: None,
            end_of_message: true,
            send_context: None,
        }
    }

    /// Create a new message from a byte slice
    pub fn from_bytes(data: &[u8]) -> Self {
        Self::new(data.to_vec())
    }

    /// Create a new message from a string
    pub fn from_string(s: &str) -> Self {
        Self::new(s.as_bytes().to_vec())
    }

    /// Get the message data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get mutable access to the message data
    pub fn data_mut(&mut self) -> &mut Vec<u8> {
        &mut self.data
    }

    /// Get the message length
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the message is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Set message properties
    pub fn with_properties(mut self, properties: MessageProperties) -> Self {
        self.properties = properties;
        self
    }

    /// Get message properties
    pub fn properties(&self) -> &MessageProperties {
        &self.properties
    }

    /// Get mutable message properties
    pub fn properties_mut(&mut self) -> &mut MessageProperties {
        &mut self.properties
    }

    /// Set message lifetime
    pub fn with_lifetime(mut self, lifetime: Duration) -> Self {
        self.properties.lifetime = Some(lifetime);
        self
    }

    /// Set message priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.properties.priority = Some(priority);
        self
    }

    /// Mark message as safely replayable (idempotent)
    /// RFC Section 9.1.3.4
    pub fn safely_replayable(mut self) -> Self {
        self.properties.safely_replayable = true;
        self
    }
    
    /// Deprecated: Use safely_replayable() instead
    #[deprecated(note = "Use safely_replayable() instead")]
    pub fn idempotent(mut self) -> Self {
        self.properties.safely_replayable = true;
        #[allow(deprecated)]
        {
            self.properties.idempotent = true;
        }
        self
    }

    /// Mark as final message
    pub fn final_message(mut self) -> Self {
        self.properties.final_message = true;
        self
    }
    
    /// Set whether this is the final message (allows setting to true or false)
    pub fn with_final(mut self, is_final: bool) -> Self {
        self.properties.final_message = is_final;
        self
    }

    /// Set message ID
    pub fn with_id(mut self, id: u64) -> Self {
        self.id = Some(id);
        self
    }

    /// Get message ID
    pub fn id(&self) -> Option<u64> {
        self.id
    }
    
    /// Set whether this message completes the application message
    /// RFC Section 9.2.3: Partial Sends
    pub fn with_end_of_message(mut self, end_of_message: bool) -> Self {
        self.end_of_message = end_of_message;
        self
    }
    
    /// Check if this message completes the application message
    pub fn is_end_of_message(&self) -> bool {
        self.end_of_message
    }
    
    /// Set send context
    pub fn with_send_context(mut self, context: SendContext) -> Self {
        self.send_context = Some(context);
        self
    }
    
    /// Get send context
    pub fn send_context(&self) -> Option<&SendContext> {
        self.send_context.as_ref()
    }
    
    /// Take send context (consumes it)
    pub fn take_send_context(&mut self) -> Option<SendContext> {
        self.send_context.take()
    }
    
    /// Create a partial message (not end of message)
    pub fn partial(data: Vec<u8>) -> Self {
        Self::new(data).with_end_of_message(false)
    }
    
    /// Set whether message ordering should be preserved
    /// RFC Section 9.1.3.3
    pub fn with_ordered(mut self, ordered: bool) -> Self {
        self.properties.ordered = Some(ordered);
        self
    }
    
    /// Set checksum coverage length
    /// RFC Section 9.1.3.6
    pub fn with_checksum_length(mut self, length: usize) -> Self {
        self.properties.checksum_length = Some(length);
        self
    }
    
    /// Set whether reliable delivery is required
    /// RFC Section 9.1.3.7
    pub fn with_reliable(mut self, reliable: bool) -> Self {
        self.properties.reliable = Some(reliable);
        self
    }
    
    /// Set capacity profile for this message
    /// RFC Section 9.1.3.8
    pub fn with_capacity_profile(mut self, profile: MessageCapacityProfile) -> Self {
        self.properties.capacity_profile = Some(profile);
        self
    }
    
    /// Disable network-layer fragmentation
    /// RFC Section 9.1.3.9
    pub fn no_fragmentation(mut self) -> Self {
        self.properties.no_fragmentation = true;
        self
    }
    
    /// Disable transport-layer segmentation
    /// RFC Section 9.1.3.10
    pub fn no_segmentation(mut self) -> Self {
        self.properties.no_segmentation = true;
        self
    }
    
    /// Builder for creating a message with specific properties
    pub fn builder(data: Vec<u8>) -> MessageBuilder {
        MessageBuilder::new(data)
    }
}

/// Builder for creating messages with specific properties
pub struct MessageBuilder {
    message: Message,
}

impl MessageBuilder {
    /// Create a new message builder
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            message: Message::new(data),
        }
    }
    
    /// Set message ID
    pub fn id(mut self, id: u64) -> Self {
        self.message = self.message.with_id(id);
        self
    }
    
    /// Set message lifetime
    pub fn lifetime(mut self, lifetime: Duration) -> Self {
        self.message = self.message.with_lifetime(lifetime);
        self
    }
    
    /// Set message priority
    pub fn priority(mut self, priority: i32) -> Self {
        self.message = self.message.with_priority(priority);
        self
    }
    
    /// Set whether message is safely replayable
    pub fn safely_replayable(mut self, replayable: bool) -> Self {
        if replayable {
            self.message = self.message.safely_replayable();
        }
        self
    }
    
    /// Set whether this is the final message
    pub fn final_message(mut self, is_final: bool) -> Self {
        if is_final {
            self.message = self.message.final_message();
        }
        self
    }
    
    /// Set whether message is ordered
    pub fn ordered(mut self, ordered: bool) -> Self {
        self.message = self.message.with_ordered(ordered);
        self
    }
    
    /// Set checksum length
    pub fn checksum_length(mut self, length: usize) -> Self {
        self.message = self.message.with_checksum_length(length);
        self
    }
    
    /// Set whether reliable delivery is required
    pub fn reliable(mut self, reliable: bool) -> Self {
        self.message = self.message.with_reliable(reliable);
        self
    }
    
    /// Set capacity profile
    pub fn capacity_profile(mut self, profile: MessageCapacityProfile) -> Self {
        self.message = self.message.with_capacity_profile(profile);
        self
    }
    
    /// Disable fragmentation
    pub fn no_fragmentation(mut self) -> Self {
        self.message = self.message.no_fragmentation();
        self
    }
    
    /// Disable segmentation
    pub fn no_segmentation(mut self) -> Self {
        self.message = self.message.no_segmentation();
        self
    }
    
    /// Set whether this completes the application message
    pub fn end_of_message(mut self, end: bool) -> Self {
        self.message = self.message.with_end_of_message(end);
        self
    }
    
    /// Set send context
    pub fn send_context(mut self, context: SendContext) -> Self {
        self.message = self.message.with_send_context(context);
        self
    }
    
    /// Build the message
    pub fn build(self) -> Message {
        self.message
    }
}

/// Context information about a received message
/// RFC Section 9.1.1
#[derive(Debug, Clone)]
pub struct MessageContext {
    /// When the message was received
    pub received_at: Instant,
    
    /// The local endpoint that received the message
    pub local_endpoint: Option<LocalEndpoint>,
    
    /// The remote endpoint that sent the message
    pub remote_endpoint: Option<RemoteEndpoint>,
    
    /// Whether this was received on the primary path
    pub primary_path: bool,
    
    /// ECN (Explicit Congestion Notification) marking
    pub ecn: Option<EcnMarking>,
    
    /// Whether this message was received as early data (0-RTT)
    pub early_data: bool,
    
    /// Reception timestamp from the network interface
    pub interface_timestamp: Option<Instant>,
}

impl MessageContext {
    /// Create a new message context
    pub fn new() -> Self {
        Self {
            received_at: Instant::now(),
            local_endpoint: None,
            remote_endpoint: None,
            primary_path: true,
            ecn: None,
            early_data: false,
            interface_timestamp: None,
        }
    }

    /// Set the local endpoint
    pub fn with_local_endpoint(mut self, endpoint: LocalEndpoint) -> Self {
        self.local_endpoint = Some(endpoint);
        self
    }

    /// Set the remote endpoint
    pub fn with_remote_endpoint(mut self, endpoint: RemoteEndpoint) -> Self {
        self.remote_endpoint = Some(endpoint);
        self
    }

    /// Set ECN marking
    pub fn with_ecn(mut self, ecn: EcnMarking) -> Self {
        self.ecn = Some(ecn);
        self
    }

    /// Mark as early data
    pub fn as_early_data(mut self) -> Self {
        self.early_data = true;
        self
    }
}

impl Default for MessageContext {
    fn default() -> Self {
        Self::new()
    }
}

/// ECN marking values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EcnMarking {
    /// Not ECN-Capable Transport
    NotEct,
    /// ECN-Capable Transport (0)
    Ect0,
    /// ECN-Capable Transport (1)
    Ect1,
    /// Congestion Experienced
    Ce,
}

/// Message Framer trait for handling message boundaries
/// RFC Section 9.1.2
pub trait MessageFramer: Send + Sync {
    /// Frame a message for sending
    fn frame(&self, message: &Message) -> Vec<u8>;
    
    /// Parse received data into messages
    fn deframe(&mut self, data: &[u8]) -> Vec<Message>;
    
    /// Reset framer state
    fn reset(&mut self);
}

/// A simple length-prefixed message framer
pub struct LengthPrefixFramer {
    buffer: Vec<u8>,
}

impl LengthPrefixFramer {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
        }
    }
}

impl MessageFramer for LengthPrefixFramer {
    fn frame(&self, message: &Message) -> Vec<u8> {
        let len = message.len() as u32;
        let mut framed = len.to_be_bytes().to_vec();
        framed.extend_from_slice(message.data());
        framed
    }
    
    fn deframe(&mut self, data: &[u8]) -> Vec<Message> {
        self.buffer.extend_from_slice(data);
        let mut messages = Vec::new();
        
        while self.buffer.len() >= 4 {
            let len_bytes: [u8; 4] = self.buffer[..4].try_into().unwrap();
            let len = u32::from_be_bytes(len_bytes) as usize;
            
            if self.buffer.len() >= 4 + len {
                let msg_data = self.buffer[4..4 + len].to_vec();
                messages.push(Message::new(msg_data));
                self.buffer.drain(..4 + len);
            } else {
                break;
            }
        }
        
        messages
    }
    
    fn reset(&mut self) {
        self.buffer.clear();
    }
}

impl Default for LengthPrefixFramer {
    fn default() -> Self {
        Self::new()
    }
}