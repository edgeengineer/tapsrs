import TransportServicesFFI

/// A Sendable wrapper for FFI handles
/// 
/// This wrapper allows us to safely pass FFI handles across actor boundaries
/// by marking them as @unchecked Sendable. The underlying handle is immutable
/// once created, so this is safe.
struct HandleWrapper: @unchecked Sendable {
    let rawHandle: UnsafeMutablePointer<transport_services_handle_t>
    
    init(_ handle: UnsafeMutablePointer<transport_services_handle_t>) {
        self.rawHandle = handle
    }
}

/// Optional handle wrapper
struct OptionalHandleWrapper: @unchecked Sendable {
    let rawHandle: UnsafeMutablePointer<transport_services_handle_t>?
    
    init(_ handle: UnsafeMutablePointer<transport_services_handle_t>?) {
        self.rawHandle = handle
    }
}