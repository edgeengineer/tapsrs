#if !hasFeature(Embedded)
#if canImport(FoundationEssentials)
import FoundationEssentials
#elseif canImport(Foundation)
import Foundation
#endif
#endif
import TransportServicesFFI

// MARK: - Path Monitor

/// A cross-platform network path monitor using Swift 6 concurrency
/// 
/// This class provides an async/await interface for monitoring network interface changes
/// and is fully thread-safe with Sendable conformance.

public actor PathMonitor {
    private nonisolated(unsafe) let handle: UnsafeMutablePointer<transport_services_handle_t>
    
    /// Create a new network path monitor
    public init() throws {
        guard let handle = transport_services_path_monitor_create() else {
            if let errorMessage = PathMonitor.getLastError() {
                throw PathMonitorError.creationFailed(message: errorMessage)
            } else {
                throw PathMonitorError.creationFailed(message: "Unknown error")
            }
        }
        self.handle = handle
    }
    
    deinit {
        transport_services_path_monitor_destroy(handle)
    }
    
    /// List all current network interfaces
    public func interfaces() async throws -> [NetworkInterface] {
        var interfacePointers: UnsafeMutablePointer<transport_services_TransportServicesInterface>?
        var count: UInt = 0
        
        let result = transport_services_path_monitor_list_interfaces(
            handle,
            &interfacePointers,
            &count
        )
        
        guard result == 0, let interfaces = interfacePointers else {
            let error = PathMonitor.getLastError() ?? "Failed to list interfaces"
            throw PathMonitorError.listInterfacesFailed(message: error)
        }
        
        defer {
            // Always free the interfaces
            var mutableInterfaces: UnsafeMutablePointer<transport_services_TransportServicesInterface>? = interfaces
            transport_services_path_monitor_free_interfaces(&mutableInterfaces, count)
        }
        
        var swiftInterfaces: [NetworkInterface] = []
        
        for i in 0..<Int(count) {
            let interface = interfaces.advanced(by: i).pointee
            swiftInterfaces.append(NetworkInterface(from: interface))
        }
        
        return swiftInterfaces
    }
    
    /// Start monitoring network changes
    /// 
    /// Returns an AsyncSequence that yields network change events
    public func changes() -> NetworkChangeSequence {
        NetworkChangeSequence(monitor: self)
    }
    
    // MARK: - Private Helpers
    
    func startWatching(callback: @escaping (NetworkChangeEvent) -> Void) -> OptionalHandleWrapper {
        let context = Unmanaged.passRetained(NetworkChangeContext(callback: callback))
        
        let watcherHandle = transport_services_path_monitor_start_watching(
            handle,
            { eventPtr, userDataPtr in
                guard let eventPtr = eventPtr,
                      let userDataPtr = userDataPtr else { return }
                
                let context = Unmanaged<NetworkChangeContext>.fromOpaque(userDataPtr).takeUnretainedValue()
                let event = eventPtr.pointee
                
                let swiftEvent = NetworkChangeEvent(from: event)
                context.callback(swiftEvent)
            },
            context.toOpaque()
        )
        
        if watcherHandle == nil {
            context.release()
        }
        
        return OptionalHandleWrapper(watcherHandle)
    }
    
    private static func getLastError() -> String? {
        guard let errorCString = transport_services_get_last_error() else { return nil }
        return String(cString: errorCString)
    }
}

// MARK: - Network Interface

/// Represents a network interface

public struct NetworkInterface: Sendable, Identifiable {
    public let id: String
    public let name: String
    public let index: UInt32
    public let ipAddresses: [String]
    public let status: Status
    public let interfaceType: String
    public let isExpensive: Bool
    
    public enum Status: Sendable {
        case up
        case down
        case unknown
    }
    
    init(from ffi: transport_services_TransportServicesInterface) {
        let nameStr = ffi.name.map { String(cString: $0) } ?? "unknown"
        self.id = "\(nameStr)_\(ffi.index)"
        self.name = ffi.name.map { String(cString: $0) } ?? "unknown"
        self.index = ffi.index
        
        // Convert IP addresses
        var addresses: [String] = []
        if let ips = ffi.ips, ffi.ip_count > 0 {
            for i in 0..<Int(ffi.ip_count) {
                if let ipCString = ips.advanced(by: i).pointee {
                    addresses.append(String(cString: ipCString))
                }
            }
        }
        self.ipAddresses = addresses
        
        // Convert status
        switch ffi.status {
        case TRANSPORT_SERVICES_TRANSPORT_SERVICES_INTERFACE_STATUS_UP:
            self.status = .up
        case TRANSPORT_SERVICES_TRANSPORT_SERVICES_INTERFACE_STATUS_DOWN:
            self.status = .down
        default:
            self.status = .unknown
        }
        
        self.interfaceType = ffi.interface_type.map { String(cString: $0) } ?? "unknown"
        self.isExpensive = ffi.is_expensive
    }
}

// MARK: - Network Change Events

/// Represents a network change event

public enum NetworkChangeEvent: Sendable {
    case added(NetworkInterface)
    case removed(NetworkInterface)
    case modified(old: NetworkInterface, new: NetworkInterface)
    case pathChanged(description: String)
    
    init(from ffi: transport_services_TransportServicesChangeEvent) {
        switch ffi.event_type {
        case TRANSPORT_SERVICES_TRANSPORT_SERVICES_CHANGE_EVENT_TYPE_ADDED:
            if let interfacePtr = ffi.interface {
                let interface = NetworkInterface(from: interfacePtr.pointee)
                self = .added(interface)
            } else {
                self = .pathChanged(description: "Interface added")
            }
            
        case TRANSPORT_SERVICES_TRANSPORT_SERVICES_CHANGE_EVENT_TYPE_REMOVED:
            if let interfacePtr = ffi.interface {
                let interface = NetworkInterface(from: interfacePtr.pointee)
                self = .removed(interface)
            } else {
                self = .pathChanged(description: "Interface removed")
            }
            
        case TRANSPORT_SERVICES_TRANSPORT_SERVICES_CHANGE_EVENT_TYPE_MODIFIED:
            if let oldPtr = ffi.old_interface,
               let newPtr = ffi.interface {
                let oldInterface = NetworkInterface(from: oldPtr.pointee)
                let newInterface = NetworkInterface(from: newPtr.pointee)
                self = .modified(old: oldInterface, new: newInterface)
            } else {
                self = .pathChanged(description: "Interface modified")
            }
            
        case TRANSPORT_SERVICES_TRANSPORT_SERVICES_CHANGE_EVENT_TYPE_PATH_CHANGED:
            let description = ffi.description.map { String(cString: $0) } ?? "Path changed"
            self = .pathChanged(description: description)
            
        default:
            self = .pathChanged(description: "Unknown change")
        }
    }
}

// MARK: - AsyncSequence for Network Changes

/// An AsyncSequence that yields network change events

public struct NetworkChangeSequence: AsyncSequence, Sendable {
    public typealias Element = NetworkChangeEvent
    
    private let monitor: PathMonitor
    
    init(monitor: PathMonitor) {
        self.monitor = monitor
    }
    
    public func makeAsyncIterator() -> NetworkChangeIterator {
        NetworkChangeIterator(monitor: monitor)
    }
}

/// AsyncIterator for network change events

public struct NetworkChangeIterator: AsyncIteratorProtocol {
    public typealias Element = NetworkChangeEvent
    
    private let stream: AsyncStream<NetworkChangeEvent>
    private var iterator: AsyncStream<NetworkChangeEvent>.Iterator
    
    init(monitor: PathMonitor) {
        let (stream, continuation) = AsyncStream<NetworkChangeEvent>.makeStream()
        self.stream = stream
        self.iterator = stream.makeAsyncIterator()
        
        // Start watching in background
        Task {
            let handleWrapper = await monitor.startWatching { event in
                continuation.yield(event)
            }
            
            // If watching failed, finish the stream
            if handleWrapper.rawHandle == nil {
                continuation.finish()
            }
            
            // Ensure cleanup when stream ends
            continuation.onTermination = { _ in
                if let handle = handleWrapper.rawHandle {
                    transport_services_path_monitor_stop_watching(handle)
                }
            }
        }
    }
    
    public mutating func next() async -> NetworkChangeEvent? {
        await iterator.next()
    }
}

// MARK: - Supporting Types

/// Context for network change callbacks
private final class NetworkChangeContext {
    let callback: (NetworkChangeEvent) -> Void
    
    init(callback: @escaping (NetworkChangeEvent) -> Void) {
        self.callback = callback
    }
}

// MARK: - Errors

/// Errors that can occur with path monitoring

public enum PathMonitorError: Error, LocalizedError {
    case creationFailed(message: String)
    case listInterfacesFailed(message: String)
    case watchingFailed(message: String)
    
    public var errorDescription: String? {
        switch self {
        case .creationFailed(let message):
            return "Failed to create path monitor: \(message)"
        case .listInterfacesFailed(let message):
            return "Failed to list interfaces: \(message)"
        case .watchingFailed(let message):
            return "Failed to start watching: \(message)"
        }
    }
}

// MARK: - Convenience Extensions


public extension NetworkInterface {
    /// Check if this interface has IPv4 connectivity
    var hasIPv4: Bool {
        ipAddresses.contains { address in
            address.contains(".") && !address.contains(":")
        }
    }
    
    /// Check if this interface has IPv6 connectivity
    var hasIPv6: Bool {
        ipAddresses.contains { address in
            address.contains(":")
        }
    }
    
    /// Check if this is a loopback interface
    var isLoopback: Bool {
        name.lowercased().contains("lo") || interfaceType.lowercased() == "loopback"
    }
    
    /// Check if this is a WiFi interface
    var isWiFi: Bool {
        interfaceType.lowercased() == "wifi" || name.lowercased().contains("en0")
    }
    
    /// Check if this is a cellular interface
    var isCellular: Bool {
        interfaceType.lowercased() == "cellular" || name.lowercased().contains("pdp")
    }
}