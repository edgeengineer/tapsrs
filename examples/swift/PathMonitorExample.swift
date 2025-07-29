#if !hasFeature(Embedded)
#if canImport(FoundationEssentials)
import FoundationEssentials
#elseif canImport(Foundation)
import Foundation
#endif
#endif
import TransportServices

/// Example demonstrating the PathMonitor API with Swift 6 concurrency
@main
@available(macOS 15.0, iOS 18.0, tvOS 18.0, watchOS 11.0, visionOS 2.0, *)
struct PathMonitorExample {
    static func main() async throws {
        print("Network Path Monitor Example")
        print("============================\n")
        
        // Initialize Transport Services
        try TransportServices.initialize()
        defer { TransportServices.cleanup() }
        
        // Create a path monitor
        let monitor = try PathMonitor()
        
        // List current interfaces
        await listCurrentInterfaces(monitor: monitor)
        
        // Monitor changes concurrently with other work
        try await withThrowingTaskGroup(of: Void.self) { group in
            // Task 1: Monitor network changes
            group.addTask {
                try await monitorNetworkChanges(monitor: monitor)
            }
            
            // Task 2: Periodically check specific conditions
            group.addTask {
                try await periodicChecks(monitor: monitor)
            }
            
            // Task 3: Simulate main work (runs for 30 seconds)
            group.addTask {
                print("Monitoring network changes for 30 seconds...")
                print("Try connecting/disconnecting WiFi or changing networks\n")
                try await Task.sleep(for: .seconds(30))
                print("\nMonitoring complete.")
            }
            
            // Wait for the main task to complete
            try await group.next()
            
            // Cancel remaining tasks
            group.cancelAll()
        }
    }
    
    // MARK: - Helper Functions
    
    static func listCurrentInterfaces(monitor: PathMonitor) async {
        print("Current Network Interfaces:")
        print("--------------------------")
        
        do {
            let interfaces = try await monitor.interfaces()
            
            if interfaces.isEmpty {
                print("No network interfaces found\n")
                return
            }
            
            for interface in interfaces.sorted(by: { $0.name < $1.name }) {
                printInterface(interface)
            }
            
            // Summary
            let activeInterfaces = interfaces.filter { $0.status == .up }
            let wifiInterfaces = interfaces.filter { $0.isWiFi }
            let cellularInterfaces = interfaces.filter { $0.isCellular }
            
            print("Summary:")
            print("  Total interfaces: \(interfaces.count)")
            print("  Active interfaces: \(activeInterfaces.count)")
            print("  WiFi interfaces: \(wifiInterfaces.count)")
            print("  Cellular interfaces: \(cellularInterfaces.count)")
            print("")
            
        } catch {
            print("Failed to list interfaces: \(error)")
        }
    }
    
    static func monitorNetworkChanges(monitor: PathMonitor) async throws {
        print("Starting network change monitoring...\n")
        
        for await event in monitor.changes() {
            await handleNetworkEvent(event)
        }
    }
    
    @MainActor
    static func handleNetworkEvent(_ event: NetworkChangeEvent) {
        let timestamp = DateFormatter.localizedString(from: Date(), dateStyle: .none, timeStyle: .medium)
        
        print("[\(timestamp)] Network Event:")
        
        switch event {
        case .added(let interface):
            print("  ✅ Interface Added: \(interface.name)")
            printInterface(interface, indent: "    ")
            
        case .removed(let interface):
            print("  ❌ Interface Removed: \(interface.name)")
            printInterface(interface, indent: "    ")
            
        case .modified(let old, let new):
            print("  🔄 Interface Modified: \(new.name)")
            print("    Old state:")
            printInterface(old, indent: "      ")
            print("    New state:")
            printInterface(new, indent: "      ")
            
        case .pathChanged(let description):
            print("  📡 Path Changed: \(description)")
        }
        
        print("")
    }
    
    static func periodicChecks(monitor: PathMonitor) async throws {
        // Check network conditions every 10 seconds
        while !Task.isCancelled {
            try await Task.sleep(for: .seconds(10))
            
            let interfaces = try await monitor.interfaces()
            let hasInternet = interfaces.contains { interface in
                interface.status == .up && !interface.isLoopback
            }
            
            let expensiveOnly = interfaces.allSatisfy { interface in
                interface.status != .up || interface.isLoopback || interface.isExpensive
            }
            
            if !hasInternet {
                print("⚠️  No internet connectivity detected")
            } else if expensiveOnly {
                print("💰 Only expensive (metered) connections available")
            }
        }
    }
    
    static func printInterface(_ interface: NetworkInterface, indent: String = "  ") {
        print("\(indent)Interface: \(interface.name) (index: \(interface.index))")
        print("\(indent)  Status: \(interface.status)")
        print("\(indent)  Type: \(interface.interfaceType)")
        print("\(indent)  Expensive: \(interface.isExpensive ? "Yes" : "No")")
        
        if !interface.ipAddresses.isEmpty {
            print("\(indent)  IP Addresses:")
            for ip in interface.ipAddresses {
                let type = ip.contains(":") ? "IPv6" : "IPv4"
                print("\(indent)    - \(ip) (\(type))")
            }
        }
    }
}

// MARK: - SwiftUI Example (if building for platforms with SwiftUI)

#if canImport(SwiftUI)
import SwiftUI

@available(macOS 15.0, iOS 18.0, tvOS 18.0, watchOS 11.0, visionOS 2.0, *)
struct PathMonitorView: View {
    @State private var interfaces: [NetworkInterface] = []
    @State private var events: [String] = []
    @State private var isMonitoring = false
    @State private var monitor: PathMonitor?
    @State private var monitorTask: Task<Void, Error>?
    
    var body: some View {
        NavigationView {
            List {
                Section("Current Interfaces") {
                    ForEach(interfaces) { interface in
                        InterfaceRow(interface: interface)
                    }
                }
                
                Section("Recent Events") {
                    ForEach(events.reversed(), id: \.self) { event in
                        Text(event)
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                }
            }
            .navigationTitle("Network Monitor")
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button(isMonitoring ? "Stop" : "Start") {
                        toggleMonitoring()
                    }
                }
            }
        }
        .task {
            await setupMonitor()
        }
    }
    
    @MainActor
    private func setupMonitor() async {
        do {
            try TransportServices.initialize()
            monitor = try PathMonitor()
            await refreshInterfaces()
        } catch {
            events.append("Failed to initialize: \(error)")
        }
    }
    
    @MainActor
    private func refreshInterfaces() async {
        guard let monitor = monitor else { return }
        
        do {
            interfaces = try await monitor.interfaces()
        } catch {
            events.append("Failed to list interfaces: \(error)")
        }
    }
    
    @MainActor
    private func toggleMonitoring() {
        if isMonitoring {
            monitorTask?.cancel()
            monitorTask = nil
            isMonitoring = false
            events.append("Monitoring stopped")
        } else {
            isMonitoring = true
            events.append("Monitoring started")
            
            monitorTask = Task {
                guard let monitor = monitor else { return }
                
                for await event in monitor.changes() {
                    if Task.isCancelled { break }
                    
                    let description = eventDescription(for: event)
                    await MainActor.run {
                        events.append(description)
                        if events.count > 20 {
                            events.removeFirst()
                        }
                    }
                    
                    // Refresh interfaces on any change
                    await refreshInterfaces()
                }
            }
        }
    }
    
    private func eventDescription(for event: NetworkChangeEvent) -> String {
        let timestamp = DateFormatter.localizedString(from: Date(), dateStyle: .none, timeStyle: .medium)
        
        switch event {
        case .added(let interface):
            return "[\(timestamp)] Added: \(interface.name)"
        case .removed(let interface):
            return "[\(timestamp)] Removed: \(interface.name)"
        case .modified(_, let new):
            return "[\(timestamp)] Modified: \(new.name)"
        case .pathChanged(let description):
            return "[\(timestamp)] \(description)"
        }
    }
}

@available(macOS 15.0, iOS 18.0, tvOS 18.0, watchOS 11.0, visionOS 2.0, *)
struct InterfaceRow: View {
    let interface: NetworkInterface
    
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(interface.name)
                    .font(.headline)
                Spacer()
                StatusIndicator(status: interface.status)
            }
            
            HStack {
                Label(interface.interfaceType, systemImage: iconForType(interface.interfaceType))
                    .font(.caption)
                    .foregroundColor(.secondary)
                
                if interface.isExpensive {
                    Label("Metered", systemImage: "dollarsign.circle")
                        .font(.caption)
                        .foregroundColor(.orange)
                }
            }
            
            if !interface.ipAddresses.isEmpty {
                Text(interface.ipAddresses.joined(separator: ", "))
                    .font(.caption2)
                    .foregroundColor(.secondary)
            }
        }
        .padding(.vertical, 2)
    }
    
    func iconForType(_ type: String) -> String {
        switch type.lowercased() {
        case "wifi": return "wifi"
        case "ethernet": return "cable.connector"
        case "cellular": return "antenna.radiowaves.left.and.right"
        case "vpn": return "lock.shield"
        case "loopback": return "arrow.triangle.2.circlepath"
        default: return "network"
        }
    }
}

@available(macOS 15.0, iOS 18.0, tvOS 18.0, watchOS 11.0, visionOS 2.0, *)
struct StatusIndicator: View {
    let status: NetworkInterface.Status
    
    var body: some View {
        Circle()
            .fill(color(for: status))
            .frame(width: 8, height: 8)
    }
    
    func color(for status: NetworkInterface.Status) -> Color {
        switch status {
        case .up: return .green
        case .down: return .red
        case .unknown: return .gray
        }
    }
}
#endif