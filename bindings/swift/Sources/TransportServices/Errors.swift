#if !hasFeature(Embedded)
#if canImport(FoundationEssentials)
import FoundationEssentials
#elseif canImport(Foundation)
import Foundation
#endif
#endif

/// Errors that can occur in Transport Services

public enum TransportServicesError: Error, LocalizedError, Sendable {
    case initializationFailed(code: Int32)
    case runtimeNotInitialized
    case invalidParameter
    case invalidHandle
    case connectionFailed(message: String)
    case connectionClosed
    case sendFailed(message: String)
    case receiveFailed(message: String)
    case listenerFailed(message: String)
    case endpointResolutionFailed
    case securityError(message: String)
    case timeout
    case cancelled
    case notImplemented(feature: String)
    
    public var errorDescription: String? {
        switch self {
        case .initializationFailed(let code):
            return "Failed to initialize Transport Services (error code: \(code))"
        case .runtimeNotInitialized:
            return "Transport Services runtime is not initialized"
        case .invalidParameter:
            return "Invalid parameter provided"
        case .invalidHandle:
            return "Invalid handle or null pointer"
        case .connectionFailed(let message):
            return "Connection failed: \(message)"
        case .connectionClosed:
            return "Connection is closed"
        case .sendFailed(let message):
            return "Failed to send data: \(message)"
        case .receiveFailed(let message):
            return "Failed to receive data: \(message)"
        case .listenerFailed(let message):
            return "Listener error: \(message)"
        case .endpointResolutionFailed:
            return "Failed to resolve endpoint"
        case .securityError(let message):
            return "Security error: \(message)"
        case .timeout:
            return "Operation timed out"
        case .cancelled:
            return "Operation was cancelled"
        case .notImplemented(let feature):
            return "\(feature) is not implemented yet"
        }
    }
}