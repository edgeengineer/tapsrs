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
    
    /// Add a socket address
    pub fn socket_address(mut self, addr: SocketAddr) -> Self {
        self.endpoint.identifiers.push(EndpointIdentifier::SocketAddress(addr));
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

impl TransportProperties {
    /// Create a new TransportProperties with default values
    /// RFC Section 6.2: NewTransportProperties()
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set a property value
    /// RFC Section 6.2: TransportProperties.Set(property, value)
    pub fn set(&mut self, property: TransportProperty, value: PropertyValue) -> &mut Self {
        match property {
            // Selection Properties
            TransportProperty::Reliability => {
                if let PropertyValue::Preference(pref) = value {
                    self.selection_properties.reliability = pref;
                }
            }
            TransportProperty::PreserveMsgBoundaries => {
                if let PropertyValue::Preference(pref) = value {
                    self.selection_properties.preserve_msg_boundaries = pref;
                }
            }
            TransportProperty::PerMsgReliability => {
                if let PropertyValue::Preference(pref) = value {
                    self.selection_properties.per_msg_reliability = pref;
                }
            }
            TransportProperty::PreserveOrder => {
                if let PropertyValue::Preference(pref) = value {
                    self.selection_properties.preserve_order = pref;
                }
            }
            TransportProperty::ZeroRttMsg => {
                if let PropertyValue::Preference(pref) = value {
                    self.selection_properties.zero_rtt_msg = pref;
                }
            }
            TransportProperty::Multistreaming => {
                if let PropertyValue::Preference(pref) = value {
                    self.selection_properties.multistreaming = pref;
                }
            }
            TransportProperty::FullChecksumSend => {
                if let PropertyValue::Preference(pref) = value {
                    self.selection_properties.full_checksum_send = pref;
                }
            }
            TransportProperty::FullChecksumRecv => {
                if let PropertyValue::Preference(pref) = value {
                    self.selection_properties.full_checksum_recv = pref;
                }
            }
            TransportProperty::CongestionControl => {
                if let PropertyValue::Preference(pref) = value {
                    self.selection_properties.congestion_control = pref;
                }
            }
            TransportProperty::KeepAlive => {
                if let PropertyValue::Preference(pref) = value {
                    self.selection_properties.keep_alive = pref;
                }
            }
            TransportProperty::Interface => {
                if let PropertyValue::StringPreference(iface, pref) = value {
                    self.selection_properties.interface.push((iface, pref));
                }
            }
            TransportProperty::Pvd => {
                if let PropertyValue::StringPreference(pvd, pref) = value {
                    self.selection_properties.pvd.push((pvd, pref));
                }
            }
            TransportProperty::UseTemporaryLocalAddress => {
                if let PropertyValue::Preference(pref) = value {
                    self.selection_properties.use_temporary_local_address = pref;
                }
            }
            TransportProperty::Multipath => {
                if let PropertyValue::Multipath(config) = value {
                    self.selection_properties.multipath = config;
                }
            }
            TransportProperty::AdvertisesAltaddr => {
                if let PropertyValue::Bool(val) = value {
                    self.selection_properties.advertises_altaddr = val;
                }
            }
            TransportProperty::Direction => {
                if let PropertyValue::Direction(dir) = value {
                    self.selection_properties.direction = dir;
                }
            }
            TransportProperty::SoftErrorNotify => {
                if let PropertyValue::Preference(pref) = value {
                    self.selection_properties.soft_error_notify = pref;
                }
            }
            TransportProperty::ActiveReadBeforeSend => {
                if let PropertyValue::Preference(pref) = value {
                    self.selection_properties.active_read_before_send = pref;
                }
            }
            // Connection Properties
            TransportProperty::ConnectionTimeout => {
                if let PropertyValue::Duration(duration) = value {
                    self.connection_properties.connection_timeout = Some(duration);
                }
            }
            TransportProperty::KeepAliveTimeout => {
                if let PropertyValue::Duration(duration) = value {
                    self.connection_properties.keep_alive_timeout = Some(duration);
                }
            }
            TransportProperty::ConnectionPriority => {
                if let PropertyValue::Integer(priority) = value {
                    self.connection_properties.connection_priority = Some(priority);
                }
            }
            TransportProperty::MaximumMessageSizeOnSend => {
                if let PropertyValue::Size(size) = value {
                    self.connection_properties.maximum_message_size_on_send = Some(size);
                }
            }
            TransportProperty::MaximumMessageSizeOnReceive => {
                if let PropertyValue::Size(size) = value {
                    self.connection_properties.maximum_message_size_on_receive = Some(size);
                }
            }
        }
        self
    }
    
    /// Create a new builder for TransportProperties
    pub fn builder() -> TransportPropertiesBuilder {
        TransportPropertiesBuilder::new()
    }
}

/// Enumeration of all transport properties
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportProperty {
    // Selection Properties
    Reliability,
    PreserveMsgBoundaries,
    PerMsgReliability,
    PreserveOrder,
    ZeroRttMsg,
    Multistreaming,
    FullChecksumSend,
    FullChecksumRecv,
    CongestionControl,
    KeepAlive,
    Interface,
    Pvd,
    UseTemporaryLocalAddress,
    Multipath,
    AdvertisesAltaddr,
    Direction,
    SoftErrorNotify,
    ActiveReadBeforeSend,
    // Connection Properties
    ConnectionTimeout,
    KeepAliveTimeout,
    ConnectionPriority,
    MaximumMessageSizeOnSend,
    MaximumMessageSizeOnReceive,
}

/// Values that can be assigned to transport properties
#[derive(Debug, Clone)]
pub enum PropertyValue {
    Preference(Preference),
    Bool(bool),
    Integer(i32),
    Size(usize),
    Duration(Duration),
    StringPreference(String, Preference),
    Multipath(MultipathConfig),
    Direction(CommunicationDirection),
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

/// Message Capacity Profile for overriding connection defaults
/// RFC Section 9.1.3.8
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageCapacityProfile {
    /// Optimized for low latency at the expense of efficiency
    LowLatencyInteractive,
    /// Optimized for low latency for application-limited transfers  
    LowLatencyNonInteractive,
    /// Optimized for high throughput and efficiency
    ConstantRate,
    /// Optimized for transferring large amounts of data
    Scavenger,
}

/// Message properties (per-message basis)
/// RFC Section 9.1.3
#[derive(Debug, Clone, Default)]
pub struct MessageProperties {
    /// Message lifetime before expiry
    /// RFC Section 9.1.3.1
    pub lifetime: Option<Duration>,
    
    /// Message priority (higher values = higher priority)
    /// RFC Section 9.1.3.2
    pub priority: Option<i32>,
    
    /// Whether ordering should be preserved for this message
    /// RFC Section 9.1.3.3
    pub ordered: Option<bool>,
    
    /// Whether this message is safely replayable (idempotent)
    /// RFC Section 9.1.3.4
    pub safely_replayable: bool,
    
    /// Whether this is the final message on the connection
    /// RFC Section 9.1.3.5
    pub final_message: bool,
    
    /// Checksum coverage length in bytes
    /// RFC Section 9.1.3.6
    pub checksum_length: Option<usize>,
    
    /// Whether reliable delivery is required for this message
    /// RFC Section 9.1.3.7
    pub reliable: Option<bool>,
    
    /// Capacity profile override for this message
    /// RFC Section 9.1.3.8
    pub capacity_profile: Option<MessageCapacityProfile>,
    
    /// Disable network-layer fragmentation
    /// RFC Section 9.1.3.9
    pub no_fragmentation: bool,
    
    /// Disable transport-layer segmentation
    /// RFC Section 9.1.3.10
    pub no_segmentation: bool,
    
    // Legacy fields (keeping for compatibility)
    #[deprecated(note = "Use safely_replayable instead")]
    pub idempotent: bool,
    #[deprecated(note = "Use checksum_length instead")]
    pub corruption_protection_length: Option<usize>,
    #[deprecated(note = "Use capacity_profile instead")]
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
    // Callbacks are stored as Option<Box<dyn Fn>> in Rust
    // For FFI, we'll use function pointers
    #[cfg(not(feature = "ffi"))]
    pub trust_verification_callback: Option<Box<dyn Fn(&CertificateChain) -> bool + Send + Sync>>,
    #[cfg(not(feature = "ffi"))]
    pub identity_challenge_callback: Option<Box<dyn Fn(&[u8]) -> Vec<u8> + Send + Sync>>,
}

impl SecurityParameters {
    /// Create new SecurityParameters with secure defaults
    /// RFC Section 6.3: NewSecurityParameters()
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Create disabled security parameters (no security)
    /// RFC Section 6.3: NewDisabledSecurityParameters()
    pub fn new_disabled() -> Self {
        Self {
            disabled: true,
            ..Default::default()
        }
    }
    
    /// Create opportunistic security parameters (try security, fall back if unavailable)
    /// RFC Section 6.3: NewOpportunisticSecurityParameters()
    pub fn new_opportunistic() -> Self {
        Self {
            opportunistic: true,
            ..Default::default()
        }
    }
    
    /// Set a security parameter value
    /// RFC Section 6.3: SecurityParameters.Set(property, value)
    pub fn set(&mut self, parameter: SecurityParameter, value: SecurityParameterValue) -> &mut Self {
        match parameter {
            SecurityParameter::Disabled => {
                if let SecurityParameterValue::Bool(val) = value {
                    self.disabled = val;
                }
            }
            SecurityParameter::Opportunistic => {
                if let SecurityParameterValue::Bool(val) = value {
                    self.opportunistic = val;
                }
            }
            SecurityParameter::AllowedProtocols => {
                if let SecurityParameterValue::Protocols(protocols) = value {
                    self.allowed_protocols = protocols;
                }
            }
            SecurityParameter::ServerCertificate => {
                if let SecurityParameterValue::Certificates(certs) = value {
                    self.server_certificate = certs;
                }
            }
            SecurityParameter::ClientCertificate => {
                if let SecurityParameterValue::Certificates(certs) = value {
                    self.client_certificate = certs;
                }
            }
            SecurityParameter::PinnedServerCertificate => {
                if let SecurityParameterValue::CertificateChains(chains) = value {
                    self.pinned_server_certificate = chains;
                }
            }
            SecurityParameter::Alpn => {
                if let SecurityParameterValue::Strings(protocols) = value {
                    self.alpn = protocols;
                }
            }
            SecurityParameter::SupportedGroups => {
                if let SecurityParameterValue::Strings(groups) = value {
                    self.supported_groups = groups;
                }
            }
            SecurityParameter::Ciphersuites => {
                if let SecurityParameterValue::Strings(suites) = value {
                    self.ciphersuites = suites;
                }
            }
            SecurityParameter::SignatureAlgorithms => {
                if let SecurityParameterValue::Strings(algos) = value {
                    self.signature_algorithms = algos;
                }
            }
            SecurityParameter::MaxCachedSessions => {
                if let SecurityParameterValue::Size(size) = value {
                    self.max_cached_sessions = Some(size);
                }
            }
            SecurityParameter::CachedSessionLifetimeSeconds => {
                if let SecurityParameterValue::U64(seconds) = value {
                    self.cached_session_lifetime_seconds = Some(seconds);
                }
            }
            SecurityParameter::PreSharedKey => {
                if let SecurityParameterValue::Psk(psk) = value {
                    self.pre_shared_key = Some(psk);
                }
            }
        }
        self
    }
    
    /// Set trust verification callback
    #[cfg(not(feature = "ffi"))]
    pub fn set_trust_verification_callback<F>(&mut self, callback: F) -> &mut Self
    where
        F: Fn(&CertificateChain) -> bool + Send + Sync + 'static,
    {
        self.trust_verification_callback = Some(Box::new(callback));
        self
    }
    
    /// Set identity challenge callback  
    #[cfg(not(feature = "ffi"))]
    pub fn set_identity_challenge_callback<F>(&mut self, callback: F) -> &mut Self
    where
        F: Fn(&[u8]) -> Vec<u8> + Send + Sync + 'static,
    {
        self.identity_challenge_callback = Some(Box::new(callback));
        self
    }
}

impl std::fmt::Debug for SecurityParameters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecurityParameters")
            .field("disabled", &self.disabled)
            .field("opportunistic", &self.opportunistic)
            .field("allowed_protocols", &self.allowed_protocols)
            .field("server_certificate", &self.server_certificate.len())
            .field("client_certificate", &self.client_certificate.len())
            .field("pinned_server_certificate", &self.pinned_server_certificate.len())
            .field("alpn", &self.alpn)
            .field("supported_groups", &self.supported_groups)
            .field("ciphersuites", &self.ciphersuites)
            .field("signature_algorithms", &self.signature_algorithms)
            .field("max_cached_sessions", &self.max_cached_sessions)
            .field("cached_session_lifetime_seconds", &self.cached_session_lifetime_seconds)
            .field("pre_shared_key", &self.pre_shared_key.is_some())
            .finish()
    }
}

impl Clone for SecurityParameters {
    fn clone(&self) -> Self {
        Self {
            disabled: self.disabled,
            opportunistic: self.opportunistic,
            allowed_protocols: self.allowed_protocols.clone(),
            server_certificate: self.server_certificate.clone(),
            client_certificate: self.client_certificate.clone(),
            pinned_server_certificate: self.pinned_server_certificate.clone(),
            alpn: self.alpn.clone(),
            supported_groups: self.supported_groups.clone(),
            ciphersuites: self.ciphersuites.clone(),
            signature_algorithms: self.signature_algorithms.clone(),
            max_cached_sessions: self.max_cached_sessions,
            cached_session_lifetime_seconds: self.cached_session_lifetime_seconds,
            pre_shared_key: self.pre_shared_key.clone(),
            // Callbacks cannot be cloned, so new instances will have None
            #[cfg(not(feature = "ffi"))]
            trust_verification_callback: None,
            #[cfg(not(feature = "ffi"))]
            identity_challenge_callback: None,
        }
    }
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
            #[cfg(not(feature = "ffi"))]
            trust_verification_callback: None,
            #[cfg(not(feature = "ffi"))]
            identity_challenge_callback: None,
        }
    }
}

/// Enumeration of security parameters
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityParameter {
    Disabled,
    Opportunistic,
    AllowedProtocols,
    ServerCertificate,
    ClientCertificate,
    PinnedServerCertificate,
    Alpn,
    SupportedGroups,
    Ciphersuites,
    SignatureAlgorithms,
    MaxCachedSessions,
    CachedSessionLifetimeSeconds,
    PreSharedKey,
}

/// Values that can be assigned to security parameters
#[derive(Debug, Clone)]
pub enum SecurityParameterValue {
    Bool(bool),
    Protocols(Vec<SecurityProtocol>),
    Certificates(Vec<Certificate>),
    CertificateChains(Vec<CertificateChain>),
    Strings(Vec<String>),
    Size(usize),
    U64(u64),
    Psk(PreSharedKey),
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
    /// Message was successfully sent
    /// RFC Section 9.2.2.1
    Sent { message_id: Option<u64> },
    /// Message expired before it could be sent  
    /// RFC Section 9.2.2.2
    Expired { message_id: Option<u64> },
    /// Error occurred while sending
    /// RFC Section 9.2.2.3
    SendError { message_id: Option<u64>, error: String },
    /// Complete message was received
    /// RFC Section 9.3.2.1
    Received { message_data: Vec<u8>, message_context: crate::MessageContext },
    /// Partial message was received
    /// RFC Section 9.3.2.2
    ReceivedPartial { message_data: Vec<u8>, message_context: crate::MessageContext, end_of_message: bool },
    /// Error occurred while receiving
    /// RFC Section 9.3.2.3
    ReceiveError { error: String },
}

/// Event types that can be emitted during rendezvous
#[derive(Debug, Clone)]
pub enum RendezvousEvent {
    /// Rendezvous completed successfully - connection established
    RendezvousDone,
    /// Rendezvous establishment failed
    EstablishmentError(String),
}

/// Builder for TransportProperties
pub struct TransportPropertiesBuilder {
    properties: TransportProperties,
}

impl TransportPropertiesBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            properties: TransportProperties::new(),
        }
    }
    
    /// Set reliability preference
    pub fn reliability(mut self, pref: Preference) -> Self {
        self.properties.set(TransportProperty::Reliability, PropertyValue::Preference(pref));
        self
    }
    
    /// Set preserve message boundaries preference
    pub fn preserve_msg_boundaries(mut self, pref: Preference) -> Self {
        self.properties.set(TransportProperty::PreserveMsgBoundaries, PropertyValue::Preference(pref));
        self
    }
    
    /// Set per-message reliability preference
    pub fn per_msg_reliability(mut self, pref: Preference) -> Self {
        self.properties.set(TransportProperty::PerMsgReliability, PropertyValue::Preference(pref));
        self
    }
    
    /// Set preserve order preference
    pub fn preserve_order(mut self, pref: Preference) -> Self {
        self.properties.set(TransportProperty::PreserveOrder, PropertyValue::Preference(pref));
        self
    }
    
    /// Set zero RTT message preference
    pub fn zero_rtt_msg(mut self, pref: Preference) -> Self {
        self.properties.set(TransportProperty::ZeroRttMsg, PropertyValue::Preference(pref));
        self
    }
    
    /// Set multistreaming preference
    pub fn multistreaming(mut self, pref: Preference) -> Self {
        self.properties.set(TransportProperty::Multistreaming, PropertyValue::Preference(pref));
        self
    }
    
    /// Set full checksum send preference
    pub fn full_checksum_send(mut self, pref: Preference) -> Self {
        self.properties.set(TransportProperty::FullChecksumSend, PropertyValue::Preference(pref));
        self
    }
    
    /// Set full checksum receive preference
    pub fn full_checksum_recv(mut self, pref: Preference) -> Self {
        self.properties.set(TransportProperty::FullChecksumRecv, PropertyValue::Preference(pref));
        self
    }
    
    /// Set congestion control preference
    pub fn congestion_control(mut self, pref: Preference) -> Self {
        self.properties.set(TransportProperty::CongestionControl, PropertyValue::Preference(pref));
        self
    }
    
    /// Set keep alive preference
    pub fn keep_alive(mut self, pref: Preference) -> Self {
        self.properties.set(TransportProperty::KeepAlive, PropertyValue::Preference(pref));
        self
    }
    
    /// Add interface preference
    pub fn interface(mut self, iface: impl Into<String>, pref: Preference) -> Self {
        self.properties.set(TransportProperty::Interface, PropertyValue::StringPreference(iface.into(), pref));
        self
    }
    
    /// Add PVD preference
    pub fn pvd(mut self, pvd: impl Into<String>, pref: Preference) -> Self {
        self.properties.set(TransportProperty::Pvd, PropertyValue::StringPreference(pvd.into(), pref));
        self
    }
    
    /// Set use temporary local address preference
    pub fn use_temporary_local_address(mut self, pref: Preference) -> Self {
        self.properties.set(TransportProperty::UseTemporaryLocalAddress, PropertyValue::Preference(pref));
        self
    }
    
    /// Set multipath configuration
    pub fn multipath(mut self, config: MultipathConfig) -> Self {
        self.properties.set(TransportProperty::Multipath, PropertyValue::Multipath(config));
        self
    }
    
    /// Set advertises alternate address
    pub fn advertises_altaddr(mut self, val: bool) -> Self {
        self.properties.set(TransportProperty::AdvertisesAltaddr, PropertyValue::Bool(val));
        self
    }
    
    /// Set communication direction
    pub fn direction(mut self, dir: CommunicationDirection) -> Self {
        self.properties.set(TransportProperty::Direction, PropertyValue::Direction(dir));
        self
    }
    
    /// Set soft error notify preference
    pub fn soft_error_notify(mut self, pref: Preference) -> Self {
        self.properties.set(TransportProperty::SoftErrorNotify, PropertyValue::Preference(pref));
        self
    }
    
    /// Set active read before send preference
    pub fn active_read_before_send(mut self, pref: Preference) -> Self {
        self.properties.set(TransportProperty::ActiveReadBeforeSend, PropertyValue::Preference(pref));
        self
    }
    
    /// Set connection timeout
    pub fn connection_timeout(mut self, duration: Duration) -> Self {
        self.properties.set(TransportProperty::ConnectionTimeout, PropertyValue::Duration(duration));
        self
    }
    
    /// Set keep alive timeout
    pub fn keep_alive_timeout(mut self, duration: Duration) -> Self {
        self.properties.set(TransportProperty::KeepAliveTimeout, PropertyValue::Duration(duration));
        self
    }
    
    /// Set connection priority
    pub fn connection_priority(mut self, priority: i32) -> Self {
        self.properties.set(TransportProperty::ConnectionPriority, PropertyValue::Integer(priority));
        self
    }
    
    /// Build the TransportProperties
    pub fn build(self) -> TransportProperties {
        self.properties
    }
}

impl Default for TransportPropertiesBuilder {
    fn default() -> Self {
        Self::new()
    }
}