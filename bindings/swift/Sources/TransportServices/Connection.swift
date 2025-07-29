#if !hasFeature(Embedded)
#if canImport(FoundationEssentials)
import FoundationEssentials
#elseif canImport(Foundation)
import Foundation
#endif
#endif
import TransportServicesFFI

// MARK: - Connection State

/// Connection state enumeration
public enum ConnectionState: Sendable {
    case establishing
    case ready
    case closing
    case closed
    case failed(Error)
    
    /// Create from FFI state
    init(ffi: TransportServicesConnectionState) {
        switch ffi {
        case TRANSPORT_SERVICES_CONNECTION_STATE_ESTABLISHING:
            self = .establishing
        case TRANSPORT_SERVICES_CONNECTION_STATE_READY:
            self = .ready
        case TRANSPORT_SERVICES_CONNECTION_STATE_CLOSING:
            self = .closing
        case TRANSPORT_SERVICES_CONNECTION_STATE_CLOSED:
            self = .closed
        case TRANSPORT_SERVICES_CONNECTION_STATE_FAILED:
            self = .failed(TransportServicesError.connectionFailed(message: "Connection failed"))
        default:
            self = .closed
        }
    }
}

// MARK: - Connection Events

/// Events that can occur on a connection
public enum ConnectionEvent: Sendable {
    case stateChanged(ConnectionState)
    case received(Data)
    case receivedPartial(Data, isEnd: Bool)
    case sent
    case sendError(Error)
    case pathChanged
    case softError(Error)
}

// MARK: - Message

/// Message for sending data with metadata
public struct Message: Sendable {
    public let data: Data
    public let context: MessageContext?
    
    public init(data: Data, context: MessageContext? = nil) {
        self.data = data
        self.context = context
    }
    
    /// Create a message from a string
    public static func from(_ string: String, encoding: String.Encoding = .utf8) -> Message? {
        guard let data = string.data(using: encoding) else { return nil }
        return Message(data: data)
    }
}

/// Message context for additional metadata
public struct MessageContext: Sendable {
    public let messageLifetime: TimeInterval?
    public let priority: Int?
    public let isEndOfMessage: Bool
    
    public init(messageLifetime: TimeInterval? = nil, priority: Int? = nil, isEndOfMessage: Bool = true) {
        self.messageLifetime = messageLifetime
        self.priority = priority
        self.isEndOfMessage = isEndOfMessage
    }
}

// MARK: - Connection Actor

/// Thread-safe connection manager using actor
public actor Connection {
    private let handle: OpaquePointer
    private var eventContinuation: AsyncStream<ConnectionEvent>.Continuation?
    private var receiveContinuations: [CheckedContinuation<Data, Error>] = []
    private var sendContinuations: [CheckedContinuation<Void, Error>] = []
    private var isClosed = false
    
    /// Current connection state
    public private(set) var state: ConnectionState = .establishing
    
    /// Create a connection from an FFI handle
    init(handle: OpaquePointer) {
        self.handle = handle
        setupEventHandling()
    }
    
    deinit {
        if !isClosed {
            transport_services_connection_close(handle)
        }
        transport_services_connection_free(handle)
    }
    
    // MARK: - Public Methods
    
    /// Get the current connection state
    public func getState() -> ConnectionState {
        guard !isClosed else { return .closed }
        
        let ffiState = transport_services_connection_get_state(handle)
        let newState = ConnectionState(ffi: ffiState)
        
        // Update our cached state
        state = newState
        return newState
    }
    
    /// Send data on the connection
    public func send(_ message: Message) async throws {
        guard !isClosed else {
            throw TransportServicesError.connectionClosed
        }
        
        guard case .ready = state else {
            throw TransportServicesError.sendFailed(message: "Connection not ready")
        }
        
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            sendContinuations.append(continuation)
            
            // Create FFI message
            var ffiMessage = TransportServicesMessage()
            ffiMessage.data = message.data.withUnsafeBytes { $0.baseAddress }
            ffiMessage.length = message.data.count
            
            if let context = message.context {
                if let lifetime = context.messageLifetime {
                    ffiMessage.lifetime_ms = UInt64(lifetime * 1000)
                }
                if let priority = context.priority {
                    ffiMessage.priority = Int32(priority)
                }
                ffiMessage.is_end_of_message = context.isEndOfMessage
            } else {
                ffiMessage.is_end_of_message = true
            }
            
            // Set up callback
            let callbackContext = Unmanaged.passRetained(ConnectionCallbackContext { [weak self] error in
                Task { [weak self] in
                    await self?.handleSendComplete(error: error)
                }
            })
            
            let result = transport_services_connection_send(
                handle,
                &ffiMessage,
                { error, userData in
                    guard let userData = userData else { return }
                    let context = Unmanaged<ConnectionCallbackContext>.fromOpaque(userData).takeRetainedValue()
                    context.callback(error)
                },
                callbackContext.toOpaque()
            )
            
            if result != TRANSPORT_SERVICES_ERROR_NONE {
                callbackContext.release()
                sendContinuations.removeLast()
                
                let errorMessage = TransportServices.getLastError() ?? "Send failed"
                continuation.resume(throwing: TransportServicesError.sendFailed(message: errorMessage))
            }
        }
    }
    
    /// Send data convenience method
    public func send(_ data: Data) async throws {
        try await send(Message(data: data))
    }
    
    /// Send string convenience method
    public func send(_ string: String, encoding: String.Encoding = .utf8) async throws {
        guard let message = Message.from(string, encoding: encoding) else {
            throw TransportServicesError.invalidParameter
        }
        try await send(message)
    }
    
    /// Receive data from the connection
    public func receive() async throws -> Data {
        guard !isClosed else {
            throw TransportServicesError.connectionClosed
        }
        
        return try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Data, Error>) in
            receiveContinuations.append(continuation)
            
            // Set up callback
            let callbackContext = Unmanaged.passRetained(ConnectionReceiveContext { [weak self] data, error in
                Task { [weak self] in
                    await self?.handleReceiveComplete(data: data, error: error)
                }
            })
            
            transport_services_connection_receive(
                handle,
                { messagePtr, error, userData in
                    guard let userData = userData else { return }
                    let context = Unmanaged<ConnectionReceiveContext>.fromOpaque(userData).takeRetainedValue()
                    
                    if let messagePtr = messagePtr {
                        let message = messagePtr.pointee
                        if let dataPtr = message.data, message.length > 0 {
                            let data = Data(bytes: dataPtr, count: message.length)
                            context.callback(data, nil)
                        } else {
                            context.callback(nil, TransportServicesError.receiveFailed(message: "Empty message"))
                        }
                    } else {
                        let errorMessage = TransportServices.getLastError() ?? "Receive failed"
                        context.callback(nil, TransportServicesError.receiveFailed(message: errorMessage))
                    }
                },
                callbackContext.toOpaque()
            )
        }
    }
    
    /// Close the connection gracefully
    public func close() async throws {
        guard !isClosed else { return }
        
        isClosed = true
        state = .closing
        
        // Cancel all pending operations
        for continuation in receiveContinuations {
            continuation.resume(throwing: TransportServicesError.connectionClosed)
        }
        receiveContinuations.removeAll()
        
        for continuation in sendContinuations {
            continuation.resume(throwing: TransportServicesError.connectionClosed)
        }
        sendContinuations.removeAll()
        
        // Close the connection
        transport_services_connection_close(handle)
        state = .closed
        
        // Notify event stream
        eventContinuation?.yield(.stateChanged(.closed))
        eventContinuation?.finish()
    }
    
    /// Get an async sequence of connection events
    public func events() -> AsyncStream<ConnectionEvent> {
        AsyncStream { continuation in
            self.eventContinuation = continuation
            
            // Yield current state
            continuation.yield(.stateChanged(state))
        }
    }
    
    // MARK: - Private Methods
    
    private func setupEventHandling() {
        // TODO: Set up FFI event callbacks
    }
    
    private func handleSendComplete(error: TransportServicesError?) {
        guard let continuation = sendContinuations.first else { return }
        sendContinuations.removeFirst()
        
        if let error = error {
            continuation.resume(throwing: error)
            eventContinuation?.yield(.sendError(error))
        } else {
            continuation.resume()
            eventContinuation?.yield(.sent)
        }
    }
    
    private func handleReceiveComplete(data: Data?, error: Error?) {
        guard let continuation = receiveContinuations.first else { return }
        receiveContinuations.removeFirst()
        
        if let data = data {
            continuation.resume(returning: data)
            eventContinuation?.yield(.received(data))
        } else if let error = error {
            continuation.resume(throwing: error)
        } else {
            continuation.resume(throwing: TransportServicesError.receiveFailed(message: "No data"))
        }
    }
}

// MARK: - Callback Contexts

/// Context for connection callbacks
private final class ConnectionCallbackContext {
    let callback: (TransportServicesError?) -> Void
    
    init(callback: @escaping (TransportServicesError?) -> Void) {
        self.callback = callback
    }
}

/// Context for receive callbacks
private final class ConnectionReceiveContext {
    let callback: (Data?, Error?) -> Void
    
    init(callback: @escaping (Data?, Error?) -> Void) {
        self.callback = callback
    }
}