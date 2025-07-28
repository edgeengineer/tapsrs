//! Core data types and enumerations for Transport Services
//! Based on RFC 9622 Section 1.1 (Terminology and Notation)

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

/// Preference levels for Selection Properties (RFC Section 1.2)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preference {
    /// Select only protocols/paths providing the Property; otherwise, fail
    Require,
    /// Prefer protocols/paths providing the Property; otherwise, proceed
    Prefer,
    /// No preference
    NoPreference,
    /// Prefer protocols/paths not providing the Property; otherwise, proceed  
    Avoid,
    /// Select only protocols/paths not providing the Property; otherwise, fail
    Prohibit,
}

/// Endpoint identifier that can represent various forms of network endpoints
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointIdentifier {
    /// Hostname (FQDN)
    HostName(String),
    /// IPv4 or IPv6 address
    IpAddress(IpAddr),
    /// Port number
    Port(u16),
    /// Service name (e.g., "https")
    Service(String),
    /// Interface identifier (e.g., "en0")
    Interface(String),
    /// Socket address (IP + Port)
    SocketAddress(SocketAddr),
    /// STUN server configuration
    StunServer {
        address: String,
        port: u16,
        credentials: Option<StunCredentials>,
    },
    /// Multicast group IP address (for send operations)
    MulticastGroupIP(IpAddr),
    /// Any-source multicast (ASM) group (for receive operations)
    AnySourceMulticastGroupIP(IpAddr),
    /// Single-source multicast (SSM) group (for receive operations)
    SingleSourceMulticastGroupIP {
        group: IpAddr,
        source: IpAddr,
    },
    /// Hop limit for multicast packets
    HopLimit(u8),
}

/// STUN server credentials
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StunCredentials {
    pub username: String,
    pub password: String,
}

/// Local endpoint specification
#[derive(Debug, Clone, Default)]
pub struct LocalEndpoint {
    pub identifiers: Vec<EndpointIdentifier>,
}

impl LocalEndpoint {
    /// Create a new empty LocalEndpoint
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new LocalEndpoint builder
    pub fn builder() -> LocalEndpointBuilder {
        LocalEndpointBuilder::new()
    }
    
    /// Add an interface identifier
    /// RFC Section 6.1: LocalSpecifier.WithInterface("en0")
    pub fn with_interface(mut self, interface: impl Into<String>) -> Self {
        self.identifiers.push(EndpointIdentifier::Interface(interface.into()));
        self
    }
    
    /// Add a port number
    /// RFC Section 6.1: LocalSpecifier.WithPort(443)
    pub fn with_port(mut self, port: u16) -> Self {
        self.identifiers.push(EndpointIdentifier::Port(port));
        self
    }
    
    /// Add an IP address
    /// RFC Section 6.1: LocalSpecifier.WithIPAddress(192.0.2.21)
    pub fn with_ip_address(mut self, addr: IpAddr) -> Self {
        self.identifiers.push(EndpointIdentifier::IpAddress(addr));
        self
    }
    
    /// Add a STUN server for NAT traversal
    /// RFC Section 6.1: LocalSpecifier.WithStunServer(address, port, credentials)
    pub fn with_stun_server(
        mut self,
        address: impl Into<String>,
        port: u16,
        credentials: Option<StunCredentials>,
    ) -> Self {
        self.identifiers.push(EndpointIdentifier::StunServer {
            address: address.into(),
            port,
            credentials,
        });
        self
    }
    
    /// Add an any-source multicast group IP address (for receive operations)
    /// RFC Section 6.1.1: LocalSpecifier.JoinGroup(group_ip, [None])
    pub fn with_any_source_multicast_group_ip(mut self, group: IpAddr) -> Self {
        self.identifiers.push(EndpointIdentifier::AnySourceMulticastGroupIP(group));
        self
    }
    
    /// Add a single-source multicast group IP address (for receive operations)
    /// RFC Section 6.1.1: LocalSpecifier.JoinGroup(group_ip, source_ip)
    pub fn with_single_source_multicast_group_ip(mut self, group: IpAddr, source: IpAddr) -> Self {
        self.identifiers.push(EndpointIdentifier::SingleSourceMulticastGroupIP {
            group,
            source,
        });
        self
    }
}

/// Builder for LocalEndpoint
pub struct LocalEndpointBuilder {
    endpoint: LocalEndpoint,
}

impl LocalEndpointBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            endpoint: LocalEndpoint::new(),
        }
    }
    
    /// Add an interface identifier
    pub fn interface(mut self, interface: impl Into<String>) -> Self {
        self.endpoint = self.endpoint.with_interface(interface);
        self
    }
    
    /// Add a port number
    pub fn port(mut self, port: u16) -> Self {
        self.endpoint = self.endpoint.with_port(port);
        self
    }
    
    /// Add an IP address
    pub fn ip_address(mut self, addr: IpAddr) -> Self {
        self.endpoint = self.endpoint.with_ip_address(addr);
        self
    }
    
    /// Add a STUN server
    pub fn stun_server(
        mut self,
        address: impl Into<String>,
        port: u16,
        credentials: Option<StunCredentials>,
    ) -> Self {
        self.endpoint = self.endpoint.with_stun_server(address, port, credentials);
        self
    }
    
    /// Add an any-source multicast group IP address
    pub fn any_source_multicast_group_ip(mut self, group: IpAddr) -> Self {
        self.endpoint = self.endpoint.with_any_source_multicast_group_ip(group);
        self
    }
    
    /// Add a single-source multicast group IP address
    pub fn single_source_multicast_group_ip(mut self, group: IpAddr, source: IpAddr) -> Self {
        self.endpoint = self.endpoint.with_single_source_multicast_group_ip(group, source);
        self
    }
    
    /// Build the LocalEndpoint
    pub fn build(self) -> LocalEndpoint {
        self.endpoint
    }
}

impl Default for LocalEndpointBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Remote endpoint specification
#[derive(Debug, Clone, Default)]
pub struct RemoteEndpoint {
    pub identifiers: Vec<EndpointIdentifier>,
    pub protocol: Option<Protocol>,
}

impl RemoteEndpoint {
    /// Create a new empty RemoteEndpoint
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new RemoteEndpoint builder
    pub fn builder() -> RemoteEndpointBuilder {
        RemoteEndpointBuilder::new()
    }
    
    /// Add a hostname
    /// RFC Section 6.1: RemoteSpecifier.WithHostName("example.com")
    pub fn with_hostname(mut self, hostname: impl Into<String>) -> Self {
        self.identifiers.push(EndpointIdentifier::HostName(hostname.into()));
        self
    }
    
    /// Add a port number
    /// RFC Section 6.1: RemoteSpecifier.WithPort(443)
    pub fn with_port(mut self, port: u16) -> Self {
        self.identifiers.push(EndpointIdentifier::Port(port));
        self
    }
    
    /// Add a service name
    /// RFC Section 6.1: RemoteSpecifier.WithService("https")
    pub fn with_service(mut self, service: impl Into<String>) -> Self {
        self.identifiers.push(EndpointIdentifier::Service(service.into()));
        self
    }
    
    /// Add an IP address
    /// RFC Section 6.1: RemoteSpecifier.WithIPAddress(192.0.2.21)
    pub fn with_ip_address(mut self, addr: IpAddr) -> Self {
        self.identifiers.push(EndpointIdentifier::IpAddress(addr));
        self
    }
    
    /// Add an interface (for link-local addresses)
    /// RFC Section 6.1: Used to qualify link-local addresses
    pub fn with_interface(mut self, interface: impl Into<String>) -> Self {
        self.identifiers.push(EndpointIdentifier::Interface(interface.into()));
        self
    }
    
    /// Set the protocol for protocol-specific endpoints
    /// RFC Section 6.1.3: RemoteSpecifier.WithProtocol(QUIC)
    pub fn with_protocol(mut self, protocol: Protocol) -> Self {
        self.protocol = Some(protocol);
        self
    }
    
    /// Add a multicast group IP address (for send operations)
    /// RFC Section 6.1.1: RemoteSpecifier.WithIPAddress(multicast_group_ip)
    pub fn with_multicast_group_ip(mut self, group: IpAddr) -> Self {
        self.identifiers.push(EndpointIdentifier::MulticastGroupIP(group));
        self
    }
    
    /// Set the hop limit for multicast packets
    /// RFC Section 6.1.1: HopLimit configuration for multicast
    pub fn with_hop_limit(mut self, hop_limit: u8) -> Self {
        self.identifiers.push(EndpointIdentifier::HopLimit(hop_limit));
        self
    }
}

/// Builder for RemoteEndpoint
pub struct RemoteEndpointBuilder {
    endpoint: RemoteEndpoint,
}

impl RemoteEndpointBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            endpoint: RemoteEndpoint::new(),
        }
    }
    
    /// Add a hostname
    pub fn hostname(mut self, hostname: impl Into<String>) -> Self {
        self.endpoint = self.endpoint.with_hostname(hostname);
        self
    }
    
    /// Add a port number
    pub fn port(mut self, port: u16) -> Self {
        self.endpoint = self.endpoint.with_port(port);
        self
    }
    
    /// Add a service name
    pub fn service(mut self, service: impl Into<String>) -> Self {
        self.endpoint = self.endpoint.with_service(service);
        self
    }
    
    /// Add an IP address
    pub fn ip_address(mut self, addr: IpAddr) -> Self {
        self.endpoint = self.endpoint.with_ip_address(addr);
        self
    }
    
    /// Add an interface
    pub fn interface(mut self, interface: impl Into<String>) -> Self {
        self.endpoint = self.endpoint.with_interface(interface);
        self
    }
    
    /// Set the protocol
    pub fn protocol(mut self, protocol: Protocol) -> Self {
        self.endpoint = self.endpoint.with_protocol(protocol);
        self
    }
    
    /// Add a multicast group IP address
    pub fn multicast_group_ip(mut self, group: IpAddr) -> Self {
        self.endpoint = self.endpoint.with_multicast_group_ip(group);
        self
    }
    
    /// Set the hop limit for multicast packets
    pub fn hop_limit(mut self, hop_limit: u8) -> Self {
        self.endpoint = self.endpoint.with_hop_limit(hop_limit);
        self
    }
    
    /// Build the RemoteEndpoint
    pub fn build(self) -> RemoteEndpoint {
        self.endpoint
    }
}

impl Default for RemoteEndpointBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Supported protocols
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    TCP,
    UDP,
    QUIC,
    SCTP,
    TLS,
    DTLS,
}

/// Transport properties for configuring connections
#[derive(Debug, Clone, Default)]
pub struct TransportProperties {
    pub selection_properties: SelectionProperties,
    pub connection_properties: ConnectionProperties,
    pub message_properties: MessageProperties,
}

/// Selection properties (used during preestablishment)
#[derive(Debug, Clone)]
pub struct SelectionProperties {
    pub reliability: Preference,
    pub preserve_msg_boundaries: Preference,
    pub per_msg_reliability: Preference,
    pub preserve_order: Preference,
    pub zero_rtt_msg: Preference,
    pub multistreaming: Preference,
    pub full_checksum_send: Preference,
    pub full_checksum_recv: Preference,
    pub congestion_control: Preference,
    pub keep_alive: Preference,
    pub interface: Vec<(String, Preference)>,
    pub pvd: Vec<(String, Preference)>,
    pub use_temporary_local_address: Preference,
    pub multipath: MultipathConfig,
    pub advertises_altaddr: bool,
    pub direction: CommunicationDirection,
    pub soft_error_notify: Preference,
    pub active_read_before_send: Preference,
}

impl Default for SelectionProperties {
    fn default() -> Self {
        Self {
            reliability: Preference::Require,
            preserve_msg_boundaries: Preference::NoPreference,
            per_msg_reliability: Preference::NoPreference,
            preserve_order: Preference::Require,
            zero_rtt_msg: Preference::NoPreference,
            multistreaming: Preference::Prefer,
            full_checksum_send: Preference::Require,
            full_checksum_recv: Preference::Require,
            congestion_control: Preference::Require,
            keep_alive: Preference::NoPreference,
            interface: Vec::new(),
            pvd: Vec::new(),
            use_temporary_local_address: Preference::Prefer,
            multipath: MultipathConfig::Disabled,
            advertises_altaddr: false,
            direction: CommunicationDirection::Bidirectional,
            soft_error_notify: Preference::NoPreference,
            active_read_before_send: Preference::NoPreference,
        }
    }
}

/// Connection properties (can be set during preestablishment and after)
#[derive(Debug, Clone, Default)]
pub struct ConnectionProperties {
    pub connection_timeout: Option<Duration>,
    pub keep_alive_timeout: Option<Duration>,
    pub connection_priority: Option<i32>,
    pub maximum_message_size_on_send: Option<usize>,
    pub maximum_message_size_on_receive: Option<usize>,
}

/// Message properties (per-message basis)
#[derive(Debug, Clone, Default)]
pub struct MessageProperties {
    pub lifetime: Option<Duration>,
    pub priority: Option<i32>,
    pub ordered: Option<bool>,
    pub idempotent: bool,
    pub final_message: bool,
    pub corruption_protection_length: Option<usize>,
    pub reliable: Option<bool>,
    pub message_capacity: Option<usize>,
}

/// Multipath configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultipathConfig {
    Disabled,
    Active,
    Passive,
}

impl Default for MultipathConfig {
    fn default() -> Self {
        MultipathConfig::Disabled
    }
}

/// Communication direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommunicationDirection {
    Bidirectional,
    UnidirectionalSend,
    UnidirectionalReceive,
}

impl Default for CommunicationDirection {
    fn default() -> Self {
        CommunicationDirection::Bidirectional
    }
}

/// Security parameters for connections
#[derive(Debug, Clone)]
pub struct SecurityParameters {
    pub disabled: bool,
    pub opportunistic: bool,
    pub allowed_protocols: Vec<SecurityProtocol>,
    pub server_certificate: Vec<Certificate>,
    pub client_certificate: Vec<Certificate>,
    pub pinned_server_certificate: Vec<CertificateChain>,
    pub alpn: Vec<String>,
    pub supported_groups: Vec<String>,
    pub ciphersuites: Vec<String>,
    pub signature_algorithms: Vec<String>,
    pub max_cached_sessions: Option<usize>,
    pub cached_session_lifetime_seconds: Option<u64>,
    pub pre_shared_key: Option<PreSharedKey>,
}

impl Default for SecurityParameters {
    fn default() -> Self {
        Self {
            disabled: false,
            opportunistic: false,
            allowed_protocols: vec![SecurityProtocol::TLS13, SecurityProtocol::TLS12],
            server_certificate: Vec::new(),
            client_certificate: Vec::new(),
            pinned_server_certificate: Vec::new(),
            alpn: Vec::new(),
            supported_groups: Vec::new(),
            ciphersuites: Vec::new(),
            signature_algorithms: Vec::new(),
            max_cached_sessions: None,
            cached_session_lifetime_seconds: None,
            pre_shared_key: None,
        }
    }
}

/// Supported security protocols
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityProtocol {
    TLS12,
    TLS13,
    DTLS12,
    DTLS13,
}

/// Certificate representation (placeholder for actual implementation)
#[derive(Debug, Clone)]
pub struct Certificate {
    pub data: Vec<u8>,
}

/// Certificate chain representation
#[derive(Debug, Clone)]
pub struct CertificateChain {
    pub certificates: Vec<Certificate>,
}

/// Pre-shared key configuration
#[derive(Debug, Clone)]
pub struct PreSharedKey {
    pub key: Vec<u8>,
    pub identity: String,
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Establishing,
    Established,
    Closing,
    Closed,
}

/// Event types that can be emitted by connections
#[derive(Debug, Clone)]
pub enum ConnectionEvent {
    Ready,
    EstablishmentError(String),
    ConnectionError(String),
    PathChange,
    SoftError(String),
    Closed,
}