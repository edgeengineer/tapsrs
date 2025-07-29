import Foundation
import TransportServicesFFI

// MARK: - Listener Events

/// Events that can occur on a listener
public enum ListenerEvent: Sendable {
    case ready(localAddress: String, port: UInt16)
    case connectionReceived(Connection)
    case stopped(Error?)
}

// MARK: - Listener

/// Listener for accepting incoming connections
public actor Listener {
    private let handle: OpaquePointer
    private var eventContinuation: AsyncStream<ListenerEvent>.Continuation?
    private var acceptContinuations: [CheckedContinuation<Connection, Error>] = []
    private var isStopped = false
    
    /// Maximum number of pending connections
    public var connectionLimit: Int = 100 {
        didSet {
            guard !isStopped else { return }
            transport_services_listener_set_new_connection_limit(handle, Int32(connectionLimit))
        }
    }
    
    /// Create a listener from an FFI handle
    init(handle: OpaquePointer) {
        self.handle = handle
        setupEventHandling()
    }
    
    deinit {
        if !isStopped {
            transport_services_listener_stop(handle)
        }
        transport_services_listener_free(handle)
    }
    
    // MARK: - Public Methods
    
    /// Get the local address the listener is bound to
    public func getLocalAddress() async throws -> (address: String, port: UInt16) {
        guard !isStopped else {
            throw TransportServicesError.listenerFailed(message: "Listener is stopped")
        }
        
        var address: UnsafeMutablePointer<CChar>?
        var port: UInt16 = 0
        
        let result = transport_services_listener_get_local_endpoint(handle, &address, &port)
        
        guard result == 0, let addressPtr = address else {
            throw TransportServicesError.listenerFailed(message: "Failed to get local address")
        }
        
        defer { transport_services_free_string(addressPtr) }
        
        let addressString = String(cString: addressPtr)
        return (addressString, port)
    }
    
    /// Accept a new connection
    public func accept() async throws -> Connection {
        guard !isStopped else {
            throw TransportServicesError.listenerFailed(message: "Listener is stopped")
        }
        
        return try await withCheckedThrowingContinuation { continuation in
            acceptContinuations.append(continuation)
            
            // Trigger accept if not already waiting
            if acceptContinuations.count == 1 {
                startAccepting()
            }
        }
    }
    
    /// Get an async sequence of incoming connections
    public func connections() -> ListenerConnectionSequence {
        ListenerConnectionSequence(listener: self)
    }
    
    /// Get an async sequence of listener events
    public func events() -> AsyncStream<ListenerEvent> {
        AsyncStream { continuation in
            self.eventContinuation = continuation
            
            // Get and yield initial ready event
            Task {
                do {
                    let (address, port) = try await getLocalAddress()
                    continuation.yield(.ready(localAddress: address, port: port))
                } catch {
                    // Ignore if we can't get the address immediately
                }
            }
        }
    }
    
    /// Stop the listener
    public func stop() async {
        guard !isStopped else { return }
        
        isStopped = true
        
        // Cancel all pending accepts
        for continuation in acceptContinuations {
            continuation.resume(throwing: TransportServicesError.cancelled)
        }
        acceptContinuations.removeAll()
        
        // Stop the listener
        transport_services_listener_stop(handle)
        
        // Notify event stream
        eventContinuation?.yield(.stopped(nil))
        eventContinuation?.finish()
    }
    
    // MARK: - Private Methods
    
    private func setupEventHandling() {
        // Set up connection received callback
        let context = Unmanaged.passRetained(ListenerContext { [weak self] connectionHandle in
            Task { [weak self] in
                await self?.handleConnectionReceived(connectionHandle)
            }
        })
        
        transport_services_listener_set_new_connection_handler(
            handle,
            { connectionHandle, userData in
                guard let userData = userData, let connectionHandle = connectionHandle else { return }
                let context = Unmanaged<ListenerContext>.fromOpaque(userData).takeUnretainedValue()
                context.callback(connectionHandle)
            },
            context.toOpaque()
        )
    }
    
    private func startAccepting() {
        guard !isStopped, !acceptContinuations.isEmpty else { return }
        
        // The FFI layer will call our callback when a connection is received
        // No explicit accept call needed - it's event-driven
    }
    
    private func handleConnectionReceived(_ connectionHandle: OpaquePointer) {
        let connection = Connection(handle: connectionHandle)
        
        // Fulfill waiting accept if any
        if let continuation = acceptContinuations.first {
            acceptContinuations.removeFirst()
            continuation.resume(returning: connection)
        }
        
        // Also yield to event stream
        eventContinuation?.yield(.connectionReceived(connection))
    }
}

// MARK: - AsyncSequence for Connections

/// AsyncSequence that yields incoming connections
public struct ListenerConnectionSequence: AsyncSequence {
    public typealias Element = Connection
    
    private let listener: Listener
    
    init(listener: Listener) {
        self.listener = listener
    }
    
    public func makeAsyncIterator() -> ListenerConnectionIterator {
        ListenerConnectionIterator(listener: listener)
    }
}

/// AsyncIterator for incoming connections
public struct ListenerConnectionIterator: AsyncIteratorProtocol {
    public typealias Element = Connection
    
    private let listener: Listener
    
    init(listener: Listener) {
        self.listener = listener
    }
    
    public mutating func next() async -> Connection? {
        do {
            return try await listener.accept()
        } catch {
            // Return nil on error to end iteration
            return nil
        }
    }
}

// MARK: - Listener Context

/// Context for listener callbacks
private final class ListenerContext {
    let callback: (OpaquePointer) -> Void
    
    init(callback: @escaping (OpaquePointer) -> Void) {
        self.callback = callback
    }
}

// MARK: - Convenience Extensions

public extension Listener {
    /// Accept connections with a handler closure
    func acceptLoop(handler: @escaping (Connection) async throws -> Void) async {
        for await connection in connections() {
            // Handle each connection concurrently
            Task {
                do {
                    try await handler(connection)
                } catch {
                    // Log error or handle as needed
                    print("Connection handler error: \(error)")
                }
            }
        }
    }
    
    /// Accept a limited number of connections
    func accept(count: Int) async throws -> [Connection] {
        var connections: [Connection] = []
        
        for _ in 0..<count {
            let connection = try await accept()
            connections.append(connection)
        }
        
        return connections
    }
}