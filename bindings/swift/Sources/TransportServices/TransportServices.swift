#if !hasFeature(Embedded)
#if canImport(FoundationEssentials)
import FoundationEssentials
#elseif canImport(Foundation)
import Foundation
#endif
#endif
import TransportServicesFFI

/// Swift wrapper for Transport Services (RFC 9622)
/// 
/// This enum provides namespace and static methods for Transport Services operations

public enum TransportServices {
    /// Initialize the Transport Services runtime
    public static func initialize() throws {
        try Runtime.shared.initialize()
    }
    
    /// Cleanup the Transport Services runtime
    public static func cleanup() {
        Runtime.shared.cleanup()
    }
    
    /// Get the version string of the Transport Services library
    public static var version: String {
        guard let cString = transport_services_version() else {
            return "Unknown"
        }
        defer { transport_services_free_string(UnsafeMutablePointer(mutating: cString)) }
        return String(cString: cString)
    }
    
    /// Get the last error message from the FFI layer
    static func getLastError() -> String? {
        guard let errorCString = transport_services_get_last_error() else { return nil }
        return String(cString: errorCString)
    }
}