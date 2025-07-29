#if !hasFeature(Embedded)
#if canImport(FoundationEssentials)
import FoundationEssentials
#elseif canImport(Foundation)
import Foundation
#endif
#endif
import TransportServices

/// Comprehensive example demonstrating Transport Services Swift bindings
@main
struct TransportServicesExample {
    static func main() async throws {
        print("Transport Services Swift Example")
        print("================================\n")
        
        // Initialize Transport Services
        try TransportServices.initialize()
        defer { TransportServices.cleanup() }
        
        print("Transport Services version: \(TransportServices.version)\n")
        
        // Run examples based on command line arguments
        let args = CommandLine.arguments
        if args.count > 1 {
            switch args[1] {
            case "client":
                try await runClientExample()
            case "server":
                try await runServerExample()
            case "echo":
                try await runEchoServerExample()
            case "monitor":
                try await runPathMonitorExample()
            case "builder":
                try await runBuilderExample()
            default:
                printUsage()
            }
        } else {
            // Run all examples
            try await runAllExamples()
        }
    }
    
    static func printUsage() {
        print("""
        Usage: TransportServicesExample [command]
        
        Commands:
            client   - Run TCP client example
            server   - Run TCP server example  
            echo     - Run echo server example
            monitor  - Run path monitor example
            builder  - Run preconnection builder example
        
        If no command is specified, all examples will run.
        """)
    }
    
    // MARK: - Client Example
    
    static func runClientExample() async throws {
        print("=== Client Example ===\n")
        
        // Create a preconnection using the builder pattern
        let preconnection = try PreconnectionBuilder()
            .withRemote(hostname: "example.com", port: 443)
            .withReliableStream()
            .withTLS(serverName: "example.com")
            .build()
        
        print("Connecting to example.com:443...")
        
        // Initiate connection
        let connection = try await preconnection.initiate()
        defer {
            Task {
                try? await connection.close()
            }
        }
        
        print("Connected! State: \(await connection.getState())")
        
        // Send HTTP request
        let request = """
        GET / HTTP/1.1\r
        Host: example.com\r
        Connection: close\r
        \r
        
        """
        
        try await connection.send(request)
        print("Sent HTTP request")
        
        // Receive response
        let responseData = try await connection.receive()
        if let response = String(data: responseData, encoding: .utf8) {
            let lines = response.split(separator: "\n").prefix(10)
            print("\nReceived response (first 10 lines):")
            for line in lines {
                print("  \(line)")
            }
        }
        
        print("\nClient example completed\n")
    }
    
    // MARK: - Server Example
    
    static func runServerExample() async throws {
        print("=== Server Example ===\n")
        
        // Create a listener
        let preconnection = try Preconnection(
            localEndpoints: [.any(port: 8080)],
            transportProperties: .reliableStream()
        )
        
        let listener = try await preconnection.listen()
        let (address, port) = try await listener.getLocalAddress()
        print("Listening on \(address):\(port)")
        
        // Set connection limit
        await listener.set { $0.connectionLimit = 5 }
        
        // Accept connections with timeout
        let connectionTask = Task {
            try await listener.accept()
        }
        
        // Wait for connection or timeout
        let timeoutTask = Task {
            try await Task.sleep(for: .seconds(5))
            throw TransportServicesError.timeout
        }
        
        do {
            let result = try await Task.select(connectionTask, timeoutTask)
            switch result {
            case .first(let connection):
                print("Accepted connection!")
                
                // Send greeting
                try await connection.send("Hello from Swift Transport Services!\n")
                
                // Close connection
                try await connection.close()
                
            case .second:
                print("No connections received within timeout")
            }
        } catch {
            print("Server error: \(error)")
        }
        
        // Stop listener
        await listener.stop()
        print("\nServer example completed\n")
    }
    
    // MARK: - Echo Server Example
    
    static func runEchoServerExample() async throws {
        print("=== Echo Server Example ===\n")
        
        // Create echo server
        let preconnection = try Preconnection(
            localEndpoints: [.localhost(port: 7777)],
            transportProperties: .reliableStream()
        )
        
        let listener = try await preconnection.listen()
        let (address, port) = try await listener.getLocalAddress()
        print("Echo server listening on \(address):\(port)")
        print("Server will run for 10 seconds...\n")
        
        // Handle connections concurrently
        let serverTask = Task {
            await listener.acceptLoop { connection in
                print("Client connected")
                
                // Echo received data
                while true {
                    do {
                        let data = try await connection.receive()
                        if let text = String(data: data, encoding: .utf8) {
                            print("Echoing: \(text.trimmingCharacters(in: .whitespacesAndNewlines))")
                        }
                        try await connection.send(data)
                    } catch {
                        print("Client disconnected")
                        break
                    }
                }
            }
        }
        
        // Run for 10 seconds
        try await Task.sleep(for: .seconds(10))
        
        // Stop server
        serverTask.cancel()
        await listener.stop()
        
        print("\nEcho server stopped\n")
    }
    
    // MARK: - Path Monitor Example
    
    static func runPathMonitorExample() async throws {
        print("=== Path Monitor Example ===\n")
        
        let monitor = try PathMonitor()
        
        // List current interfaces
        print("Current Network Interfaces:")
        let interfaces = try await monitor.interfaces()
        for interface in interfaces.sorted(by: { $0.name < $1.name }) {
            print("\n  \(interface.name) (index: \(interface.index))")
            print("    Status: \(interface.status)")
            print("    Type: \(interface.interfaceType)")
            print("    Expensive: \(interface.isExpensive ? "Yes" : "No")")
            if !interface.ipAddresses.isEmpty {
                print("    IPs: \(interface.ipAddresses.joined(separator: ", "))")
            }
        }
        
        // Monitor changes for 10 seconds
        print("\nMonitoring network changes for 10 seconds...")
        
        let monitorTask = Task {
            for await event in monitor.changes() {
                switch event {
                case .added(let interface):
                    print("  ✅ Added: \(interface.name)")
                case .removed(let interface):
                    print("  ❌ Removed: \(interface.name)")
                case .modified(let old, let new):
                    print("  🔄 Modified: \(new.name) (was \(old.status), now \(new.status))")
                case .pathChanged(let description):
                    print("  📡 Path changed: \(description)")
                }
            }
        }
        
        try await Task.sleep(for: .seconds(10))
        monitorTask.cancel()
        
        print("\nPath monitor example completed\n")
    }
    
    // MARK: - Builder Pattern Example
    
    static func runBuilderExample() async throws {
        print("=== Builder Pattern Example ===\n")
        
        // Example 1: Simple TCP client
        print("1. Simple TCP client:")
        let tcpClient = try PreconnectionBuilder()
            .withRemote(hostname: "example.com", port: 80)
            .withReliableStream()
            .build()
        print("   Created TCP client preconnection")
        
        // Example 2: UDP client with specific local interface
        print("\n2. UDP client with local endpoint:")
        let udpClient = try PreconnectionBuilder()
            .withLocalEndpoint(.any(port: 0))
            .withRemote(hostname: "8.8.8.8", port: 53)
            .withUnreliableDatagram()
            .build()
        print("   Created UDP client preconnection")
        
        // Example 3: TLS server
        print("\n3. TLS server:")
        let tlsServer = try PreconnectionBuilder()
            .withLocalEndpoint(.any(port: 8443))
            .withReliableStream()
            .withTLS()
            .build()
        print("   Created TLS server preconnection")
        
        // Example 4: Custom transport properties
        print("\n4. Custom transport properties:")
        var customProps = TransportProperties()
        customProps.multipath = .active
        customProps.keepAlive = .require
        customProps.expiredDnsAllowed = true
        
        let customClient = try PreconnectionBuilder()
            .withRemote(hostname: "example.com", port: 443)
            .withTransportProperties(customProps)
            .withTLS(serverName: "example.com")
            .build()
        print("   Created client with custom properties")
        
        print("\nBuilder pattern examples completed\n")
    }
    
    // MARK: - All Examples
    
    static func runAllExamples() async throws {
        do {
            try await runPathMonitorExample()
        } catch {
            print("Path monitor example failed: \(error)\n")
        }
        
        do {
            try await runBuilderExample()
        } catch {
            print("Builder example failed: \(error)\n")
        }
        
        do {
            try await runClientExample()
        } catch {
            print("Client example failed: \(error)\n")
        }
        
        do {
            try await runServerExample()
        } catch {
            print("Server example failed: \(error)\n")
        }
    }
}

// MARK: - Task Selection Helper

extension Task where Success == Never, Failure == Never {
    /// Select the first task to complete from two tasks
    static func select<T1, T2>(_ task1: Task<T1, Error>, _ task2: Task<T2, Error>) async throws -> SelectResult<T1, T2> {
        await withTaskGroup(of: SelectResult<T1, T2>?.self) { group in
            group.addTask {
                do {
                    let value = try await task1.value
                    return .first(value)
                } catch {
                    return nil
                }
            }
            
            group.addTask {
                do {
                    let value = try await task2.value
                    return .second(value)
                } catch {
                    return nil
                }
            }
            
            // Return first non-nil result
            for await result in group {
                if let result = result {
                    group.cancelAll()
                    return result
                }
            }
            
            // Both tasks threw errors
            throw TransportServicesError.cancelled
        }
    }
}

enum SelectResult<T1, T2> {
    case first(T1)
    case second(T2)
}