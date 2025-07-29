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
@available(macOS 15.0, iOS 18.0, tvOS 18.0, watchOS 11.0, visionOS 2.0, *)
public enum Preference: Int32, CaseIterable, Sendable {
    case require = 0
    case prefer = 1
    case noPreference = 2
    case avoid = 3
    case prohibit = 4
    
    /// Convert to FFI representation
    var toFFI: TransportServicesPreference {
        TransportServicesPreference(rawValue: self.rawValue)!
    }
    
    /// Create from FFI representation
    init(ffi: TransportServicesPreference) {
        self = Preference(rawValue: ffi.rawValue)!
    }
}

// MARK: - Multipath Configuration

/// Multipath configuration options
@available(macOS 15.0, iOS 18.0, tvOS 18.0, watchOS 11.0, visionOS 2.0, *)
public enum MultipathConfig: Int32, CaseIterable, Sendable {
    case disabled = 0
    case active = 1
    case passive = 2
    
    /// Convert to FFI representation
    var toFFI: TransportServicesMultipathConfig {
        TransportServicesMultipathConfig(rawValue: self.rawValue)!
    }
    
    /// Create from FFI representation
    init(ffi: TransportServicesMultipathConfig) {
        self = MultipathConfig(rawValue: ffi.rawValue)!
    }
}

// MARK: - Communication Direction

/// Communication direction for connections
@available(macOS 15.0, iOS 18.0, tvOS 18.0, watchOS 11.0, visionOS 2.0, *)
public enum CommunicationDirection: Int32, CaseIterable, Sendable {
    case bidirectional = 0
    case unidirectionalSend = 1
    case unidirectionalReceive = 2
    
    /// Convert to FFI representation
    var toFFI: TransportServicesCommunicationDirection {
        TransportServicesCommunicationDirection(rawValue: self.rawValue)!
    }
    
    /// Create from FFI representation
    init(ffi: TransportServicesCommunicationDirection) {
        self = CommunicationDirection(rawValue: ffi.rawValue)!
    }
}

// MARK: - Transport Properties

/// Transport properties configuration
@available(macOS 15.0, iOS 18.0, tvOS 18.0, watchOS 11.0, visionOS 2.0, *)
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
    func toFFIHandle() -> OpaquePointer? {
        let handle = transport_services_transport_properties_new()
        
        // Set all properties
        transport_services_transport_properties_set_reliability(handle, reliability.toFFI)
        transport_services_transport_properties_set_preserve_order(handle, preserveOrder.toFFI)
        transport_services_transport_properties_set_preserve_msg_boundaries(handle, preserveMsgBoundaries.toFFI)
        transport_services_transport_properties_set_per_msg_reliability(handle, perMessageReliability.toFFI)
        transport_services_transport_properties_set_zero_rtt_msg(handle, zeroRttMsg.toFFI)
        transport_services_transport_properties_set_multistreaming(handle, multistreaming.toFFI)
        transport_services_transport_properties_set_fullchecksum(handle, fullchecksum.toFFI)
        transport_services_transport_properties_set_congestion_control(handle, congestionControl.toFFI)
        transport_services_transport_properties_set_keep_alive(handle, keepAlive.toFFI)
        
        transport_services_transport_properties_set_temporary_local_address(handle, useTemporaryLocalAddress.toFFI)
        transport_services_transport_properties_set_multipath(handle, multipath.toFFI)
        transport_services_transport_properties_set_direction(handle, direction.toFFI)
        transport_services_transport_properties_set_retransmit_notify(handle, retransmitNotify.toFFI)
        transport_services_transport_properties_set_soft_error_notify(handle, softErrorNotify.toFFI)
        
        if let pvd = pvd {
            pvd.withCString { cString in
                transport_services_transport_properties_set_pvd(handle, cString)
            }
        }
        
        transport_services_transport_properties_set_expired_dns_allowed(handle, expiredDnsAllowed)
        
        return handle
    }
}

// MARK: - Security Parameters

/// Security parameters configuration
@available(macOS 15.0, iOS 18.0, tvOS 18.0, watchOS 11.0, visionOS 2.0, *)
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
    func toFFIHandle() -> OpaquePointer? {
        let handle = transport_services_security_parameters_new()
        
        transport_services_security_parameters_set_use_tls(handle, useTLS)
        
        if let serverName = serverName {
            serverName.withCString { cString in
                transport_services_security_parameters_set_server_name(handle, cString)
            }
        }
        
        // TODO: Implement certificate verification modes and TLS version setting
        
        return handle
    }
}