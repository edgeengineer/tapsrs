#if !hasFeature(Embedded)
#if canImport(FoundationEssentials)
import FoundationEssentials
#elseif canImport(Foundation)
import Foundation
#endif
#endif
import TransportServicesFFI

// MARK: - Endpoint Protocol

/// Common protocol for all endpoint types
public protocol Endpoint: Sendable {
    /// Convert to FFI representation
    func toFFI() -> TransportServicesEndpoint
}

// MARK: - Local Endpoint

/// Local endpoint for connections
public struct LocalEndpoint: Endpoint, Hashable {
    /// IP address (optional)
    public let ipAddress: String?
    
    /// Port number (0 for any available port)
    public let port: UInt16
    
    /// Network interface name (optional)
    public let interface: String?
    
    /// Create a local endpoint
    public init(ipAddress: String? = nil, port: UInt16 = 0, interface: String? = nil) {
        self.ipAddress = ipAddress
        self.port = port
        self.interface = interface
    }
    
    /// Create a local endpoint listening on any address
    public static func any(port: UInt16 = 0) -> LocalEndpoint {
        LocalEndpoint(ipAddress: nil, port: port)
    }
    
    /// Create a local endpoint for localhost
    public static func localhost(port: UInt16 = 0) -> LocalEndpoint {
        LocalEndpoint(ipAddress: "127.0.0.1", port: port)
    }
    
    public func toFFI() -> TransportServicesEndpoint {
        TransportServicesEndpoint(
            hostname: ipAddress?.withCString { strdup($0) },
            port: port,
            service: nil,
            interface: interface?.withCString { strdup($0) }
        )
    }
}

// MARK: - Remote Endpoint

/// Remote endpoint for connections
public struct RemoteEndpoint: Endpoint, Hashable {
    /// Hostname or IP address
    public let hostname: String
    
    /// Port number or service name
    public let portOrService: PortOrService
    
    /// Network interface to use (optional)
    public let interface: String?
    
    /// Port or service identifier
    public enum PortOrService: Hashable, Sendable {
        case port(UInt16)
        case service(String)
    }
    
    /// Create a remote endpoint with hostname and port
    public init(hostname: String, port: UInt16, interface: String? = nil) {
        self.hostname = hostname
        self.portOrService = .port(port)
        self.interface = interface
    }
    
    /// Create a remote endpoint with hostname and service name
    public init(hostname: String, service: String, interface: String? = nil) {
        self.hostname = hostname
        self.portOrService = .service(service)
        self.interface = interface
    }
    
    public func toFFI() -> TransportServicesEndpoint {
        let service: UnsafeMutablePointer<CChar>?
        let port: UInt16
        
        switch portOrService {
        case .port(let p):
            port = p
            service = nil
        case .service(let s):
            port = 0
            service = s.withCString { strdup($0) }
        }
        
        return TransportServicesEndpoint(
            hostname: hostname.withCString { strdup($0) },
            port: port,
            service: service,
            interface: interface?.withCString { strdup($0) }
        )
    }
}

// MARK: - Endpoint Utilities

extension Array where Element == any Endpoint {
    /// Convert array of endpoints to FFI representation
    func toFFIArray() -> (UnsafeMutablePointer<TransportServicesEndpoint>?, Int) {
        guard !isEmpty else { return (nil, 0) }
        
        let buffer = UnsafeMutablePointer<TransportServicesEndpoint>.allocate(capacity: count)
        for (index, endpoint) in enumerated() {
            buffer.advanced(by: index).pointee = endpoint.toFFI()
        }
        
        return (buffer, count)
    }
}

/// Free FFI endpoint array
func freeFFIEndpoints(_ endpoints: UnsafeMutablePointer<TransportServicesEndpoint>?, count: Int) {
    guard let endpoints = endpoints, count > 0 else { return }
    
    for i in 0..<count {
        let endpoint = endpoints.advanced(by: i).pointee
        if let hostname = endpoint.hostname {
            free(UnsafeMutablePointer(mutating: hostname))
        }
        if let service = endpoint.service {
            free(UnsafeMutablePointer(mutating: service))
        }
        if let interface = endpoint.interface {
            free(UnsafeMutablePointer(mutating: interface))
        }
    }
    
    endpoints.deallocate()
}