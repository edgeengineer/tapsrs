#if !hasFeature(Embedded)
#if canImport(FoundationEssentials)
import FoundationEssentials
#elseif canImport(Foundation)
import Foundation
#endif
#endif
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
    private nonisolated(unsafe) let handle: UnsafeMutablePointer<transport_services_handle_t>
    private var eventContinuation: AsyncStream<ListenerEvent>.Continuation?
    private var acceptContinuations: [CheckedContinuation<Connection, Error>] = []
    private var isStopped = false
    
    /// Maximum number of pending connections
    public var connectionLimit: Int = 100
    
    /// Create a listener from an FFI handle
    init(handle: UnsafeMutablePointer<transport_services_handle_t>) {
        self.handle = handle
        Task {
            await setupEventHandling()
        }
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
        
        // TODO: FFI function transport_services_listener_get_local_endpoint is not yet exposed
        throw TransportServicesError.notImplemented(feature: "transport_services_listener_get_local_endpoint")
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
        let context = Unmanaged.passRetained(ListenerContext { connectionHandle in
            let wrapper = HandleWrapper(connectionHandle)
            Task { [weak self] in
                await self?.handleConnectionReceived(wrapper.rawHandle)
            }
        })
        
        transport_services_listener_set_callbacks(
            handle,
            { connectionHandle, userData in
                guard let userData = userData, let connectionHandle = connectionHandle else { return }
                let context = Unmanaged<ListenerContext>.fromOpaque(userData).takeUnretainedValue()
                context.callback(connectionHandle)
            },
            { error, errorMessage, userData in
                // TODO: Handle errors if needed
            },
            context.toOpaque()
        )
    }
    
    private func startAccepting() {
        guard !isStopped, !acceptContinuations.isEmpty else { return }
        
        // The FFI layer will call our callback when a connection is received
        // No explicit accept call needed - it's event-driven
    }
    
    private func handleConnectionReceived(_ connectionHandle: UnsafeMutablePointer<transport_services_handle_t>) {
        let wrapper = HandleWrapper(connectionHandle)
        let connection = Connection(handle: wrapper.rawHandle)
        
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
    let callback: (UnsafeMutablePointer<transport_services_handle_t>) -> Void
    
    init(callback: @escaping (UnsafeMutablePointer<transport_services_handle_t>) -> Void) {
        self.callback = callback
    }
}

// MARK: - Convenience Extensions

public extension Listener {
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