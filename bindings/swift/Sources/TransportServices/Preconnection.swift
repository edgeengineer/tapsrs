#if !hasFeature(Embedded)
#if canImport(FoundationEssentials)
import FoundationEssentials
#elseif canImport(Foundation)
import Foundation
#endif
#endif
import TransportServicesFFI

/// Preconnection represents a set of parameters for establishing connections
public struct Preconnection: Sendable {
    private nonisolated(unsafe) let handle: UnsafeMutablePointer<transport_services_handle_t>
    
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
    ) async throws {
        // Ensure runtime is initialized
        try await Runtime.shared.initialize()
        
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
        defer { transport_services_free_transport_properties(propertiesHandle) }
        
        guard let securityHandle = securityParameters.toFFIHandle() else {
            throw TransportServicesError.invalidParameter
        }
        defer { transport_services_free_security_parameters(securityHandle) }
        
        // Create preconnection
        guard let handle = transport_services_preconnection_new() else {
            let error = TransportServices.getLastError() ?? "Failed to create preconnection"
            throw TransportServicesError.connectionFailed(message: error)
        }
        
        self.handle = handle
        
        // Add endpoints after creation
        for endpoint in localEndpoints {
            var endpointFFI = endpoint.toFFI()
            let result = transport_services_preconnection_add_local_endpoint(handle, &endpointFFI)
            if result != TRANSPORT_SERVICES_ERROR_T_SUCCESS {
                throw TransportServicesError.invalidParameter
            }
        }
        
        for endpoint in remoteEndpoints {
            var endpointFFI = endpoint.toFFI()
            let result = transport_services_preconnection_add_remote_endpoint(handle, &endpointFFI)
            if result != TRANSPORT_SERVICES_ERROR_T_SUCCESS {
                throw TransportServicesError.invalidParameter
            }
        }
        
        // TODO: Set properties - need to convert TransportProperties to FFI struct
        // let propResult = transport_services_preconnection_set_transport_properties(handle, propertiesHandle)
        // if propResult != TRANSPORT_SERVICES_ERROR_T_SUCCESS {
        //     throw TransportServicesError.invalidParameter
        // }
    }
    
    /// Initiate a connection
    public func initiate() async throws -> Connection {
        try await withCheckedThrowingContinuation { continuation in
            let context = PreconnectionContext(continuation: continuation)
            let contextPtr = Unmanaged.passRetained(context)
            
            let result = transport_services_preconnection_initiate(
                handle,
                { connectionHandle, userData in
                    guard let userData = userData else { return }
                    let context = Unmanaged<PreconnectionContext>.fromOpaque(userData).takeRetainedValue()
                    
                    if let connectionHandle = connectionHandle {
                        let wrapper = HandleWrapper(connectionHandle)
                        let connection = Connection(handle: wrapper.rawHandle)
                        context.continuation.resume(returning: connection)
                    }
                },
                { error, errorMessage, userData in
                    guard let userData = userData else { return }
                    let context = Unmanaged<PreconnectionContext>.fromOpaque(userData).takeRetainedValue()
                    let message = errorMessage.map { String(cString: $0) } ?? "Connection initiation failed"
                    context.continuation.resume(throwing: TransportServicesError.connectionFailed(message: message))
                },
                contextPtr.toOpaque()
            )
            
            if result != TRANSPORT_SERVICES_ERROR_T_SUCCESS {
                contextPtr.release()
                let errorMessage = TransportServices.getLastError() ?? "Failed to initiate connection"
                continuation.resume(throwing: TransportServicesError.connectionFailed(message: errorMessage))
            }
        }
    }
    
    /// Listen for incoming connections
    public func listen() async throws -> Listener {
        // TODO: FFI function transport_services_preconnection_listen is not yet exposed
        throw TransportServicesError.notImplemented(feature: "transport_services_preconnection_listen")
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
    public func build() async throws -> Preconnection {
        try await Preconnection(
            localEndpoints: localEndpoints,
            remoteEndpoints: remoteEndpoints,
            transportProperties: transportProperties,
            securityParameters: securityParameters
        )
    }
}