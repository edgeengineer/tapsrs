//! Message implementation for TAPS
//! Based on RFC 9622 Section 9.1 (Messages and Framers)

use crate::{MessageProperties, LocalEndpoint, RemoteEndpoint};
use std::time::{Duration, Instant};

/// A Message is the unit of data transfer in TAPS
#[derive(Debug, Clone)]
pub struct Message {
    /// The actual data payload
    data: Vec<u8>,
    
    /// Properties specific to this message
    properties: MessageProperties,
    
    /// Optional message identifier
    id: Option<u64>,
}

impl Message {
    /// Create a new message with data
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            properties: MessageProperties::default(),
            id: None,
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

    /// Mark message as idempotent (safe to replay)
    pub fn idempotent(mut self) -> Self {
        self.properties.idempotent = true;
        self
    }

    /// Mark as final message
    pub fn final_message(mut self) -> Self {
        self.properties.final_message = true;
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