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
    init(ffi: transport_services_connection_state_t) {
        switch ffi {
        case TRANSPORT_SERVICES_CONNECTION_STATE_T_ESTABLISHING:
            self = .establishing
        case TRANSPORT_SERVICES_CONNECTION_STATE_T_ESTABLISHED:
            self = .ready
        case TRANSPORT_SERVICES_CONNECTION_STATE_T_CLOSING:
            self = .closing
        case TRANSPORT_SERVICES_CONNECTION_STATE_T_CLOSED:
            self = .closed
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
    private let handleWrapper: HandleWrapper
    private var eventContinuation: AsyncStream<ConnectionEvent>.Continuation?
    private var isClosed = false
    
    /// Current connection state
    public private(set) var state: ConnectionState = .establishing
    
    /// Create a connection from an FFI handle
    init(handle: UnsafeMutablePointer<transport_services_handle_t>) {
        self.handleWrapper = HandleWrapper(handle)
        Task {
            await setupEventHandling()
        }
    }
    
    deinit {
        if !isClosed {
            transport_services_connection_close(handleWrapper.rawHandle)
        }
        transport_services_connection_free(handleWrapper.rawHandle)
    }
    
    // MARK: - Public Methods
    
    /// Get the current connection state
    public func getState() -> ConnectionState {
        guard !isClosed else { return .closed }
        
        let ffiState = transport_services_connection_get_state(handleWrapper.rawHandle)
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
            message.data.withUnsafeBytes { dataBufferPointer in
                var ffiMessage = transport_services_message_t()
                ffiMessage.data = dataBufferPointer.baseAddress?.assumingMemoryBound(to: UInt8.self)
                ffiMessage.length = UInt(message.data.count)
                
                if let context = message.context {
                    if let lifetime = context.messageLifetime {
                        ffiMessage.lifetime_ms = UInt64(lifetime * 1000)
                    }
                    if let priority = context.priority {
                        ffiMessage.priority = Int32(priority)
                    }
                    ffiMessage.final_message = context.isEndOfMessage
                } else {
                    ffiMessage.final_message = true
                }
                
                let sendContext = Unmanaged.passRetained(SendContinuationContext(continuation: continuation))
                
                let result = transport_services_connection_send(
                    handleWrapper.rawHandle,
                    &ffiMessage,
                    { error, _, userData in // message pointer is ignored here
                        guard let userData = userData else { return }
                        let context = Unmanaged<SendContinuationContext>.fromOpaque(userData).takeRetainedValue()
                        if error == TRANSPORT_SERVICES_ERROR_T_SUCCESS {
                            context.continuation.resume()
                        } else {
                            let errorMessage = TransportServices.getLastError() ?? "Send failed with code \(error)"
                            context.continuation.resume(throwing: TransportServicesError.sendFailed(message: errorMessage))
                        }
                    },
                    sendContext.toOpaque()
                )
                
                if result != TRANSPORT_SERVICES_ERROR_T_SUCCESS {
                    sendContext.release()
                    let errorMessage = TransportServices.getLastError() ?? "Send failed"
                    continuation.resume(throwing: TransportServicesError.sendFailed(message: errorMessage))
                }
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
            let receiveContext = Unmanaged.passRetained(ReceiveContinuationContext(continuation: continuation))
            
            transport_services_connection_receive(
                handleWrapper.rawHandle,
                { messagePtr, context, userData in
                    guard let userData = userData else { return }
                    let receiveContext = Unmanaged<ReceiveContinuationContext>.fromOpaque(userData).takeRetainedValue()
                    
                    if let messagePtr = messagePtr {
                        let message = messagePtr.pointee
                        if let dataPtr = message.data, message.length > 0 {
                            let data = Data(bytes: dataPtr, count: Int(message.length))
                            receiveContext.continuation.resume(returning: data)
                        } else {
                            receiveContext.continuation.resume(throwing: TransportServicesError.receiveFailed(message: "Empty message received"))
                        }
                    }
                },
                { error, errorMessage, userData in
                    guard let userData = userData else { return }
                    let receiveContext = Unmanaged<ReceiveContinuationContext>.fromOpaque(userData).takeRetainedValue()
                    let message = errorMessage.map { String(cString: $0) } ?? "Receive failed"
                    receiveContext.continuation.resume(throwing: TransportServicesError.receiveFailed(message: message))
                },
                receiveContext.toOpaque()
            )
        }
    }
    
    /// Close the connection gracefully
    public func close() async throws {
        guard !isClosed else { return }
        
        isClosed = true
        state = .closing
        
        // Close the connection
        transport_services_connection_close(handleWrapper.rawHandle)
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
}

// MARK: - Callback Contexts

/// Context for send continuations
private final class SendContinuationContext {
    let continuation: CheckedContinuation<Void, Error>
    init(continuation: CheckedContinuation<Void, Error>) { self.continuation = continuation }
}

/// Context for receive continuations
private final class ReceiveContinuationContext {
    let continuation: CheckedContinuation<Data, Error>
    init(continuation: CheckedContinuation<Data, Error>) { self.continuation = continuation }
}
