import Foundation
import TransportServicesFFI

/// Thread-safe runtime manager for Transport Services
/// 
/// This actor ensures that runtime initialization and cleanup are handled safely
/// across concurrent contexts.

actor Runtime {
    /// Shared runtime instance
    static let shared = Runtime()
    
    private var isInitialized = false
    private var initializationCount = 0
    
    private init() {}
    
    /// Initialize the runtime (can be called multiple times safely)
    func initialize() throws {
        if !isInitialized {
            let result = transport_services_init_runtime()
            guard result == 0 else {
                throw TransportServicesError.initializationFailed(code: result)
            }
            isInitialized = true
        }
        initializationCount += 1
    }
    
    /// Cleanup the runtime (only actually cleans up when all references are released)
    func cleanup() {
        guard isInitialized else { return }
        
        initializationCount -= 1
        if initializationCount <= 0 {
            transport_services_shutdown_runtime()
            isInitialized = false
            initializationCount = 0
        }
    }
    
    /// Check if the runtime is initialized
    var initialized: Bool {
        isInitialized
    }
    
    /// Ensure runtime is initialized for an operation
    func ensureInitialized() throws {
        guard isInitialized else {
            throw TransportServicesError.runtimeNotInitialized
        }
    }
}