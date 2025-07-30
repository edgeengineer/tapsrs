import TransportServicesFFI

// Simple example to test basic compilation
public struct Example {
    public static func getVersion() -> String {
        guard let versionCString = transport_services_version() else {
            return "Unknown"
        }
        defer { transport_services_free_string(UnsafeMutablePointer(mutating: versionCString)) }
        return String(cString: versionCString)
    }
    
    public static func initialize() -> Int32 {
        return transport_services_init_runtime()
    }
}