#if !hasFeature(Embedded)
#if canImport(FoundationEssentials)
import FoundationEssentials
#elseif canImport(Foundation)
import Foundation
#endif
#endif
import TransportServicesFFI

// MARK: - Preference

/// Preference level for transport properties
public enum Preference: Int32, CaseIterable, Sendable {
    case require = 0
    case prefer = 1
    case noPreference = 2
    case avoid = 3
    case prohibit = 4
    
    /// Convert to FFI representation
    var toFFI: transport_services_preference_t {
        switch self {
        case .require: return TRANSPORT_SERVICES_PREFERENCE_T_REQUIRE
        case .prefer: return TRANSPORT_SERVICES_PREFERENCE_T_PREFER
        case .noPreference: return TRANSPORT_SERVICES_PREFERENCE_T_NO_PREFERENCE
        case .avoid: return TRANSPORT_SERVICES_PREFERENCE_T_AVOID
        case .prohibit: return TRANSPORT_SERVICES_PREFERENCE_T_PROHIBIT
        }
    }
    
    /// Create from FFI representation
    init(ffi: transport_services_preference_t) {
        switch ffi {
        case TRANSPORT_SERVICES_PREFERENCE_T_REQUIRE:
            self = .require
        case TRANSPORT_SERVICES_PREFERENCE_T_PREFER:
            self = .prefer
        case TRANSPORT_SERVICES_PREFERENCE_T_NO_PREFERENCE:
            self = .noPreference
        case TRANSPORT_SERVICES_PREFERENCE_T_AVOID:
            self = .avoid
        case TRANSPORT_SERVICES_PREFERENCE_T_PROHIBIT:
            self = .prohibit
        default:
            self = .noPreference
        }
    }
}

// MARK: - Multipath Configuration

/// Multipath configuration options
public enum MultipathConfig: Int32, CaseIterable, Sendable {
    case disabled = 0
    case active = 1
    case passive = 2
    
    /// Convert to FFI representation
    var toFFI: transport_services_TransportServicesMultipathConfig {
        switch self {
        case .disabled: return TRANSPORT_SERVICES_TRANSPORT_SERVICES_MULTIPATH_CONFIG_DISABLED
        case .active: return TRANSPORT_SERVICES_TRANSPORT_SERVICES_MULTIPATH_CONFIG_ACTIVE
        case .passive: return TRANSPORT_SERVICES_TRANSPORT_SERVICES_MULTIPATH_CONFIG_PASSIVE
        }
    }
    
    /// Create from FFI representation
    init(ffi: transport_services_TransportServicesMultipathConfig) {
        switch ffi {
        case TRANSPORT_SERVICES_TRANSPORT_SERVICES_MULTIPATH_CONFIG_DISABLED:
            self = .disabled
        case TRANSPORT_SERVICES_TRANSPORT_SERVICES_MULTIPATH_CONFIG_ACTIVE:
            self = .active
        case TRANSPORT_SERVICES_TRANSPORT_SERVICES_MULTIPATH_CONFIG_PASSIVE:
            self = .passive
        default:
            self = .disabled
        }
    }
}

// MARK: - Communication Direction

/// Communication direction for connections
public enum CommunicationDirection: Int32, CaseIterable, Sendable {
    case bidirectional = 0
    case unidirectionalSend = 1
    case unidirectionalReceive = 2
    
    /// Convert to FFI representation
    var toFFI: transport_services_TransportServicesCommunicationDirection {
        switch self {
        case .bidirectional: return TRANSPORT_SERVICES_TRANSPORT_SERVICES_COMMUNICATION_DIRECTION_BIDIRECTIONAL
        case .unidirectionalSend: return TRANSPORT_SERVICES_TRANSPORT_SERVICES_COMMUNICATION_DIRECTION_UNIDIRECTIONAL_SEND
        case .unidirectionalReceive: return TRANSPORT_SERVICES_TRANSPORT_SERVICES_COMMUNICATION_DIRECTION_UNIDIRECTIONAL_RECEIVE
        }
    }
    
    /// Create from FFI representation
    init(ffi: transport_services_TransportServicesCommunicationDirection) {
        switch ffi {
        case TRANSPORT_SERVICES_TRANSPORT_SERVICES_COMMUNICATION_DIRECTION_BIDIRECTIONAL:
            self = .bidirectional
        case TRANSPORT_SERVICES_TRANSPORT_SERVICES_COMMUNICATION_DIRECTION_UNIDIRECTIONAL_SEND:
            self = .unidirectionalSend
        case TRANSPORT_SERVICES_TRANSPORT_SERVICES_COMMUNICATION_DIRECTION_UNIDIRECTIONAL_RECEIVE:
            self = .unidirectionalReceive
        default:
            self = .bidirectional
        }
    }
}

// MARK: - Transport Properties

/// Transport properties configuration
public struct TransportProperties: Sendable {
    // Protocol preferences
    public var reliability: Preference
    public var preserveOrder: Preference
    public var preserveMsgBoundaries: Preference
    public var perMessageReliability: Preference
    public var zeroRttMsg: Preference
    public var multistreaming: Preference
    public var fullchecksum: Preference
    public var congestionControl: Preference
    public var keepAlive: Preference
    
    // Interface preferences  
    public var useTemporaryLocalAddress: Preference
    public var multipath: MultipathConfig
    public var direction: CommunicationDirection
    public var retransmitNotify: Preference
    public var softErrorNotify: Preference
    
    // Connection preferences
    public var pvd: String?
    public var expiredDnsAllowed: Bool
    
    /// Create transport properties with default values
    public init() {
        // Defaults for reliable, ordered delivery (TCP-like)
        self.reliability = .require
        self.preserveOrder = .require
        self.preserveMsgBoundaries = .noPreference
        self.perMessageReliability = .noPreference
        self.zeroRttMsg = .noPreference
        self.multistreaming = .noPreference
        self.fullchecksum = .require
        self.congestionControl = .require
        self.keepAlive = .noPreference
        
        self.useTemporaryLocalAddress = .noPreference
        self.multipath = .disabled
        self.direction = .bidirectional
        self.retransmitNotify = .noPreference
        self.softErrorNotify = .noPreference
        
        self.pvd = nil
        self.expiredDnsAllowed = false
    }
    
    /// Create properties for reliable, ordered stream (TCP-like)
    public static func reliableStream() -> TransportProperties {
        TransportProperties()
    }
    
    /// Create properties for unreliable datagram (UDP-like)
    public static func unreliableDatagram() -> TransportProperties {
        var props = TransportProperties()
        props.reliability = .avoid
        props.preserveOrder = .avoid
        props.preserveMsgBoundaries = .require
        props.congestionControl = .avoid
        return props
    }
    
    /// Create properties for reliable datagram (SCTP-like)
    public static func reliableDatagram() -> TransportProperties {
        var props = TransportProperties()
        props.reliability = .require
        props.preserveOrder = .noPreference
        props.preserveMsgBoundaries = .require
        return props
    }
    
    /// Convert to FFI handle
    func toFFIHandle() -> UnsafeMutablePointer<transport_services_handle_t>? {
        let handle = transport_services_new_transport_properties()
        
        // TODO: Set properties using the available setters
        // The exact function names need to be determined from the header
        
        return handle
    }
}

// MARK: - Security Parameters

/// Security parameters configuration
public struct SecurityParameters: Sendable {
    /// Whether to use TLS
    public var useTLS: Bool
    
    /// Minimum TLS version
    public var minimumTLSVersion: TLSVersion?
    
    /// Server name for SNI
    public var serverName: String?
    
    /// Certificate verification mode
    public var verifyMode: CertificateVerificationMode
    
    /// TLS version enumeration
    public enum TLSVersion: String, CaseIterable, Sendable {
        case tls10 = "1.0"
        case tls11 = "1.1"
        case tls12 = "1.2"
        case tls13 = "1.3"
    }
    
    /// Certificate verification modes
    public enum CertificateVerificationMode: Sendable {
        case system         // Use system default verification
        case disabled       // No verification (dangerous!)
        case custom(verify: @Sendable (Data) -> Bool)  // Custom verification
    }
    
    /// Create default security parameters (no TLS)
    public init() {
        self.useTLS = false
        self.minimumTLSVersion = nil
        self.serverName = nil
        self.verifyMode = .system
    }
    
    /// Create TLS-enabled security parameters
    public static func tls(serverName: String? = nil, minimumVersion: TLSVersion = .tls12) -> SecurityParameters {
        var params = SecurityParameters()
        params.useTLS = true
        params.minimumTLSVersion = minimumVersion
        params.serverName = serverName
        params.verifyMode = .system
        return params
    }
    
    /// Create security parameters with disabled certificate verification (for testing only!)
    public static func insecureTLS() -> SecurityParameters {
        var params = SecurityParameters()
        params.useTLS = true
        params.minimumTLSVersion = .tls12
        params.verifyMode = .disabled
        return params
    }
    
    /// Convert to FFI handle
    func toFFIHandle() -> UnsafeMutablePointer<transport_services_handle_t>? {
        if !useTLS {
            return transport_services_new_disabled_security_parameters()
        }
        
        let handle = transport_services_new_security_parameters()
        
        // TODO: Set security parameters using available functions
        
        return handle
    }
}