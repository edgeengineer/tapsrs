import Testing
@testable import TransportServices

@Test("Transport Services version")
func testVersion() async throws {
    let version = TransportServices.version
    print("Transport Services version: \(version)")
    #expect(!version.isEmpty)
}

@Test("Runtime initialization")
func testInitialization() async throws {
    try await TransportServices.initialize()
    // Clean up
    await TransportServices.cleanup()
}

@Test("Path monitor lists network interfaces")
func testPathMonitor() async throws {
    try await TransportServices.initialize()
    defer {
        Task {
            await TransportServices.cleanup()
        }
    }
    
    let monitor = try PathMonitor()
    let interfaces = try await monitor.interfaces()
    
    print("Found \(interfaces.count) network interfaces:")
    for interface in interfaces {
        print("  - \(interface.name): \(interface.status) [\(interface.ipAddresses.joined(separator: ", "))]")
    }
    
    #expect(interfaces.count > 0, "Should have at least one network interface")
}

@Test("Example usage")
func testExample() async throws {
    let version = Example.getVersion()
    print("Example version: \(version)")
    #expect(!version.isEmpty)
    
    let result = Example.initialize()
    #expect(result == 0, "Initialization should succeed")
    
    // Clean up
    await TransportServices.cleanup()
}