use crate::{Message, MessageContext, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Message Framer trait as defined in RFC 9622 Section 9.1.2
/// 
/// Message Framers allow extending a Connection's Protocol Stack to define how to 
/// encapsulate or encode outbound Messages and how to decapsulate or decode inbound 
/// data into Messages.
#[async_trait]
pub trait Framer: Send + Sync {
    /// Frame outbound messages for transmission
    async fn frame_message(&self, message: &Message, context: &MessageContext) -> Result<Vec<u8>>;
    
    /// Parse inbound data into messages
    async fn parse_data(&self, data: &[u8]) -> Result<Vec<(Message, MessageContext)>>;
    
    /// Get the name of this framer for identification
    fn name(&self) -> &str;
    
    /// Called when the framer is attached to a connection
    async fn on_attach(&self) -> Result<()> {
        Ok(())
    }
    
    /// Called when the connection is being closed
    async fn on_detach(&self) -> Result<()> {
        Ok(())
    }
}

/// Length-prefix framer implementation
/// 
/// This framer adds a 4-byte length prefix to each message for framing
pub struct LengthPrefixFramer {
    buffer: Arc<RwLock<Vec<u8>>>,
}

impl LengthPrefixFramer {
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl Framer for LengthPrefixFramer {
    async fn frame_message(&self, message: &Message, _context: &MessageContext) -> Result<Vec<u8>> {
        let data = message.data();
        let len = data.len() as u32;
        
        let mut framed = Vec::with_capacity(4 + data.len());
        framed.extend_from_slice(&len.to_be_bytes());
        framed.extend_from_slice(data);
        
        Ok(framed)
    }
    
    async fn parse_data(&self, data: &[u8]) -> Result<Vec<(Message, MessageContext)>> {
        let mut buffer = self.buffer.write().await;
        buffer.extend_from_slice(data);
        
        let mut messages = Vec::new();
        let mut pos = 0;
        
        while buffer.len() >= pos + 4 {
            // Read length prefix
            let len_bytes = &buffer[pos..pos + 4];
            let len = u32::from_be_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;
            
            // Check if we have the complete message
            if buffer.len() >= pos + 4 + len {
                let message_data = &buffer[pos + 4..pos + 4 + len];
                let message = Message::from_bytes(message_data);
                let context = MessageContext::new();
                
                messages.push((message, context));
                pos += 4 + len;
            } else {
                // Not enough data for complete message
                break;
            }
        }
        
        // Remove processed data from buffer
        if pos > 0 {
            buffer.drain(..pos);
        }
        
        Ok(messages)
    }
    
    fn name(&self) -> &str {
        "length-prefix"
    }
}

impl Default for LengthPrefixFramer {
    fn default() -> Self {
        Self::new()
    }
}

/// Stack of framers that can be applied to a connection
pub struct FramerStack {
    framers: Vec<Box<dyn Framer>>,
}

impl FramerStack {
    pub fn new() -> Self {
        Self {
            framers: Vec::new(),
        }
    }
    
    pub fn add_framer(&mut self, framer: Box<dyn Framer>) {
        self.framers.push(framer);
    }
    
    pub async fn frame_message(&self, message: &Message, context: &MessageContext) -> Result<Vec<u8>> {
        let mut data = message.data().to_vec();
        
        // Apply framers in reverse order (last added runs first for outbound)
        for framer in self.framers.iter().rev() {
            let temp_message = Message::from_bytes(&data);
            data = framer.frame_message(&temp_message, context).await?;
        }
        
        Ok(data)
    }
    
    pub async fn parse_data(&self, data: &[u8]) -> Result<Vec<(Message, MessageContext)>> {
        let mut current_data = data;
        let mut temp_buffer = Vec::new();
        
        // Apply framers in order (first added runs first for inbound parsing)
        for framer in &self.framers {
            let parsed = framer.parse_data(current_data).await?;
            
            if parsed.is_empty() {
                // No complete messages yet
                return Ok(Vec::new());
            }
            
            // For now, handle single message case
            if let Some((message, _context)) = parsed.first() {
                temp_buffer = message.data().to_vec();
                current_data = &temp_buffer;
            }
        }
        
        // If we got here, parsing was successful
        if let Some(framer) = self.framers.last() {
            framer.parse_data(current_data).await
        } else {
            // No framers, return original data as message
            let message = Message::from_bytes(current_data);
            let context = MessageContext::new();
            Ok(vec![(message, context)])
        }
    }
    
    pub fn is_empty(&self) -> bool {
        self.framers.is_empty()
    }
    
    pub async fn on_attach(&self) -> Result<()> {
        for framer in &self.framers {
            framer.on_attach().await?;
        }
        Ok(())
    }
    
    pub async fn on_detach(&self) -> Result<()> {
        for framer in &self.framers {
            framer.on_detach().await?;
        }
        Ok(())
    }
}

impl Default for FramerStack {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for FramerStack {
    fn clone(&self) -> Self {
        // Note: This is a simplified clone that creates a new empty stack
        // In a full implementation, we'd need to clone the framers themselves
        Self::new()
    }
}