import Foundation
import TransportServicesFFI

/// Preconnection represents a set of parameters for establishing connections
public struct Preconnection: Sendable {
    private let handle: OpaquePointer
    
    /// Local endpoints
    public let localEndpoints: [LocalEndpoint]
    
    /// Remote endpoints
    public let remoteEndpoints: [RemoteEndpoint]
    
    /// Transport properties
    public let transportProperties: TransportProperties
    
    /// Security parameters
    public let securityParameters: SecurityParameters
    
    /// Create a new preconnection
    public init(
        localEndpoints: [LocalEndpoint] = [],
        remoteEndpoints: [RemoteEndpoint] = [],
        transportProperties: TransportProperties = TransportProperties(),
        securityParameters: SecurityParameters = SecurityParameters()
    ) throws {
        // Ensure runtime is initialized
        try Runtime.shared.initialize()
        
        self.localEndpoints = localEndpoints
        self.remoteEndpoints = remoteEndpoints
        self.transportProperties = transportProperties
        self.securityParameters = securityParameters
        
        // Convert endpoints to FFI
        let (localFFI, localCount) = localEndpoints.map { $0 as any Endpoint }.toFFIArray()
        defer { freeFFIEndpoints(localFFI, count: localCount) }
        
        let (remoteFFI, remoteCount) = remoteEndpoints.map { $0 as any Endpoint }.toFFIArray()
        defer { freeFFIEndpoints(remoteFFI, count: remoteCount) }
        
        // Get property handles
        guard let propertiesHandle = transportProperties.toFFIHandle() else {
            throw TransportServicesError.invalidParameter
        }
        defer { transport_services_transport_properties_free(propertiesHandle) }
        
        guard let securityHandle = securityParameters.toFFIHandle() else {
            throw TransportServicesError.invalidParameter
        }
        defer { transport_services_security_parameters_free(securityHandle) }
        
        // Create preconnection
        guard let handle = transport_services_preconnection_new(
            localFFI,
            localCount,
            remoteFFI,
            remoteCount,
            propertiesHandle,
            securityHandle
        ) else {
            let error = TransportServices.getLastError() ?? "Failed to create preconnection"
            throw TransportServicesError.connectionFailed(message: error)
        }
        
        self.handle = handle
    }
    
    /// Initiate a connection
    public func initiate() async throws -> Connection {
        try await withCheckedThrowingContinuation { continuation in
            let context = PreconnectionContext(continuation: continuation)
            let contextPtr = Unmanaged.passRetained(context)
            
            transport_services_preconnection_initiate(
                handle,
                { connectionHandle, error, userData in
                    guard let userData = userData else { return }
                    let context = Unmanaged<PreconnectionContext>.fromOpaque(userData).takeRetainedValue()
                    
                    if let connectionHandle = connectionHandle {
                        let connection = Connection(handle: connectionHandle)
                        context.continuation.resume(returning: connection)
                    } else {
                        let errorMessage = TransportServices.getLastError() ?? "Connection initiation failed"
                        context.continuation.resume(throwing: TransportServicesError.connectionFailed(message: errorMessage))
                    }
                },
                contextPtr.toOpaque()
            )
        }
    }
    
    /// Listen for incoming connections
    public func listen() async throws -> Listener {
        let listenerHandle = transport_services_preconnection_listen(handle)
        guard let handle = listenerHandle else {
            let error = TransportServices.getLastError() ?? "Failed to create listener"
            throw TransportServicesError.listenerFailed(message: error)
        }
        
        return Listener(handle: handle)
    }
    
    /// Start a rendezvous (simultaneous connect/listen)
    public func rendezvous() async throws -> (Connection, Listener) {
        // Create a listener first
        let listener = try await listen()
        
        // Then initiate a connection
        let connection = try await initiate()
        
        return (connection, listener)
    }
}

// MARK: - Preconnection Context

/// Context for preconnection callbacks
private final class PreconnectionContext {
    let continuation: CheckedContinuation<Connection, Error>
    
    init(continuation: CheckedContinuation<Connection, Error>) {
        self.continuation = continuation
    }
}

// MARK: - Preconnection Builder

/// Builder pattern for creating preconnections
public struct PreconnectionBuilder: Sendable {
    private var localEndpoints: [LocalEndpoint] = []
    private var remoteEndpoints: [RemoteEndpoint] = []
    private var transportProperties = TransportProperties()
    private var securityParameters = SecurityParameters()
    
    public init() {}
    
    /// Add a local endpoint
    public func withLocalEndpoint(_ endpoint: LocalEndpoint) -> PreconnectionBuilder {
        var builder = self
        builder.localEndpoints.append(endpoint)
        return builder
    }
    
    /// Add a remote endpoint
    public func withRemoteEndpoint(_ endpoint: RemoteEndpoint) -> PreconnectionBuilder {
        var builder = self
        builder.remoteEndpoints.append(endpoint)
        return builder
    }
    
    /// Add a remote endpoint with hostname and port
    public func withRemote(hostname: String, port: UInt16) -> PreconnectionBuilder {
        withRemoteEndpoint(RemoteEndpoint(hostname: hostname, port: port))
    }
    
    /// Set transport properties
    public func withTransportProperties(_ properties: TransportProperties) -> PreconnectionBuilder {
        var builder = self
        builder.transportProperties = properties
        return builder
    }
    
    /// Use reliable stream transport (TCP-like)
    public func withReliableStream() -> PreconnectionBuilder {
        withTransportProperties(.reliableStream())
    }
    
    /// Use unreliable datagram transport (UDP-like)
    public func withUnreliableDatagram() -> PreconnectionBuilder {
        withTransportProperties(.unreliableDatagram())
    }
    
    /// Set security parameters
    public func withSecurityParameters(_ parameters: SecurityParameters) -> PreconnectionBuilder {
        var builder = self
        builder.securityParameters = parameters
        return builder
    }
    
    /// Enable TLS
    public func withTLS(serverName: String? = nil) -> PreconnectionBuilder {
        withSecurityParameters(.tls(serverName: serverName))
    }
    
    /// Build the preconnection
    public func build() throws -> Preconnection {
        try Preconnection(
            localEndpoints: localEndpoints,
            remoteEndpoints: remoteEndpoints,
            transportProperties: transportProperties,
            securityParameters: securityParameters
        )
    }
}