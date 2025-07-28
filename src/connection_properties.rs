//! Connection Properties implementation for Transport Services
//! Based on RFC 9622 Section 8.1

use crate::ConnectionState;
use std::collections::HashMap;
use std::time::Duration;

/// Generic Connection Properties as defined in RFC 9622 Section 8.1
#[derive(Debug, Clone)]
pub enum ConnectionProperty {
    /// Required Minimum Corruption Protection Coverage for Receiving (8.1.1)
    /// Specifies the minimum number of bytes in a received Message that need to be covered by a checksum
    RecvChecksumLen(ChecksumCoverage),

    /// Connection Priority (8.1.2)  
    /// Priority of this Connection relative to other Connections in the same Connection Group
    /// Lower numeric value = higher priority
    ConnPriority(u32),

    /// Timeout for Aborting Connection (8.1.3)
    /// How long to wait before deciding that an active Connection has failed
    ConnTimeout(TimeoutValue),

    /// Timeout for Keep-Alive Packets (8.1.4)
    /// Maximum length of time an idle Connection waits before sending a keep-alive packet
    KeepAliveTimeout(TimeoutValue),

    /// Connection Group Transmission Scheduler (8.1.5)
    /// Which scheduler is used among Connections within a Connection Group
    ConnScheduler(SchedulerType),

    /// Capacity Profile (8.1.6)
    /// Desired network treatment for traffic sent by the application
    ConnCapacityProfile(CapacityProfile),

    /// Policy for Using Multipath Transports (8.1.7)
    /// Local policy for transferring data across multiple paths
    MultipathPolicy(MultipathPolicy),

    /// Bounds on Send Rate (8.1.8)
    MinSendRate(Option<u64>), // bits per second, None = Unlimited
    MaxSendRate(Option<u64>), // bits per second, None = Unlimited

    /// Bounds on Receive Rate (8.1.8)
    MinRecvRate(Option<u64>), // bits per second, None = Unlimited
    MaxRecvRate(Option<u64>), // bits per second, None = Unlimited

    /// Group Connection Limit (8.1.9)
    /// Number of Connections that can be accepted from a peer as new members of the Connection's group
    GroupConnLimit(Option<u32>), // None = Unlimited

    /// Isolate Session (8.1.10)
    /// When true, use as little cached information as possible from previous Connections
    IsolateSession(bool),

    // Read-only properties (8.1.11)
    /// Connection State (8.1.11.1)
    ConnState(ConnectionState),

    /// Can Send Data (8.1.11.2)
    CanSend(bool),

    /// Can Receive Data (8.1.11.3)
    CanReceive(bool),

    /// Maximum Message Size Before Fragmentation (8.1.11.4)
    SingularTransmissionMsgMaxLen(Option<usize>),

    /// Maximum Message Size on Send (8.1.11.5)
    SendMsgMaxLen(Option<usize>),

    /// Maximum Message Size on Receive (8.1.11.6)
    RecvMsgMaxLen(Option<usize>),

    // TCP-specific properties (8.2)
    /// Advertised User Timeout (8.2.1)
    TcpUserTimeoutValue(Option<Duration>),

    /// User Timeout Enabled (8.2.2)
    TcpUserTimeoutEnabled(bool),

    /// Timeout Changeable (8.2.3)
    TcpUserTimeoutChangeable(bool),
}

/// Checksum coverage specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum ChecksumCoverage {
    /// Full message coverage
    #[default]
    FullCoverage,
    /// Minimum number of bytes to be covered
    MinBytes(usize),
}

/// Timeout value specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum TimeoutValue {
    /// Timeout is disabled
    #[default]
    Disabled,
    /// Timeout duration
    Duration(Duration),
}

/// Connection scheduler types (8.1.5)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum SchedulerType {
    /// Weighted Fair Queueing (default)
    #[default]
    WeightedFairQueueing,
    /// First-In-First-Out
    Fifo,
    /// Round Robin
    RoundRobin,
    /// Proportional Rate Reduction
    ProportionalRate,
}

/// Capacity profile types (8.1.6)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum CapacityProfile {
    /// Default (Best Effort)
    #[default]
    Default,
    /// Low Latency/Interactive
    LowLatencyInteractive,
    /// Low Latency/Non-Interactive  
    LowLatencyNonInteractive,
    /// Constant-Rate Streaming
    ConstantRateStreaming,
    /// Capacity-Seeking
    CapacitySeeking,
}

/// Multipath policy types (8.1.7)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum MultipathPolicy {
    /// Use only a single path at a time, failover when needed
    #[default]
    Handover,
    /// Simultaneously use multiple paths when possible
    Active,
    /// Use multiple paths redundantly
    Redundant,
}






/// Storage for connection properties
#[derive(Debug, Clone, Default)]
pub struct ConnectionProperties {
    pub(crate) properties: HashMap<String, ConnectionProperty>,
}

impl ConnectionProperties {
    pub fn new() -> Self {
        let mut properties = HashMap::new();

        // Set default values as per RFC
        properties.insert(
            "recvChecksumLen".to_string(),
            ConnectionProperty::RecvChecksumLen(ChecksumCoverage::default()),
        );
        properties.insert(
            "connPriority".to_string(),
            ConnectionProperty::ConnPriority(100),
        ); // Default: 100
        properties.insert(
            "connTimeout".to_string(),
            ConnectionProperty::ConnTimeout(TimeoutValue::default()),
        );
        properties.insert(
            "keepAliveTimeout".to_string(),
            ConnectionProperty::KeepAliveTimeout(TimeoutValue::default()),
        );
        properties.insert(
            "connScheduler".to_string(),
            ConnectionProperty::ConnScheduler(SchedulerType::default()),
        );
        properties.insert(
            "connCapacityProfile".to_string(),
            ConnectionProperty::ConnCapacityProfile(CapacityProfile::default()),
        );
        properties.insert(
            "multipathPolicy".to_string(),
            ConnectionProperty::MultipathPolicy(MultipathPolicy::default()),
        );
        properties.insert(
            "minSendRate".to_string(),
            ConnectionProperty::MinSendRate(None),
        ); // Unlimited
        properties.insert(
            "maxSendRate".to_string(),
            ConnectionProperty::MaxSendRate(None),
        ); // Unlimited
        properties.insert(
            "minRecvRate".to_string(),
            ConnectionProperty::MinRecvRate(None),
        ); // Unlimited
        properties.insert(
            "maxRecvRate".to_string(),
            ConnectionProperty::MaxRecvRate(None),
        ); // Unlimited
        properties.insert(
            "groupConnLimit".to_string(),
            ConnectionProperty::GroupConnLimit(None),
        ); // Unlimited
        properties.insert(
            "isolateSession".to_string(),
            ConnectionProperty::IsolateSession(false),
        ); // Default: false

        // TCP-specific defaults
        // tcp.userTimeoutValue defaults to None (use TCP default)
        properties.insert(
            "tcp.userTimeoutEnabled".to_string(),
            ConnectionProperty::TcpUserTimeoutEnabled(false),
        ); // Default: false
        properties.insert(
            "tcp.userTimeoutChangeable".to_string(),
            ConnectionProperty::TcpUserTimeoutChangeable(true),
        ); // Default: true

        Self { properties }
    }

    /// Set a property value
    pub fn set(&mut self, key: &str, value: ConnectionProperty) -> crate::Result<()> {
        // Check if this is a read-only property
        match key {
            "connState"
            | "canSend"
            | "canReceive"
            | "singularTransmissionMsgMaxLen"
            | "sendMsgMaxLen"
            | "recvMsgMaxLen" => {
                return Err(crate::TransportServicesError::InvalidParameters(format!(
                    "Property '{key}' is read-only"
                )));
            }
            _ => {}
        }

        self.properties.insert(key.to_string(), value);
        Ok(())
    }

    /// Get a property value
    pub fn get(&self, key: &str) -> Option<&ConnectionProperty> {
        self.properties.get(key)
    }

    /// Check if a property exists
    pub fn has(&self, key: &str) -> bool {
        self.properties.contains_key(key)
    }

    /// Get all properties
    pub fn all(&self) -> &HashMap<String, ConnectionProperty> {
        &self.properties
    }

    /// Update read-only properties based on connection state
    pub fn update_readonly(&mut self, state: ConnectionState, can_send: bool, can_receive: bool) {
        self.properties.insert(
            "connState".to_string(),
            ConnectionProperty::ConnState(state),
        );
        self.properties
            .insert("canSend".to_string(), ConnectionProperty::CanSend(can_send));
        self.properties.insert(
            "canReceive".to_string(),
            ConnectionProperty::CanReceive(can_receive),
        );
    }
}
