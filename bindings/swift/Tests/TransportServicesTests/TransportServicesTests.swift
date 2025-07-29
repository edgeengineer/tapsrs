import Testing
import Foundation
@testable import TransportServices

@Suite("Transport Services Tests")
struct TransportServicesTests {
    
    init() throws {
        // Initialize Transport Services for all tests
        try TransportServices.initialize()
    }
    
    deinit {
        // Cleanup after all tests
        TransportServices.cleanup()
    }
    
    @Test("Version string is not empty")
    func testVersion() {
        let version = TransportServices.version
        #expect(!version.isEmpty)
        #expect(version != "Unknown")
    }
    
    @Test("Can create a preconnection")
    func testPreconnectionCreation() throws {
        let preconnection = try Preconnection()
        // If we get here without throwing, the test passes
        #expect(true)
    }
    
    @Test("Transport Services error descriptions")
    func testErrorDescriptions() {
        let initError = TransportServicesError.initializationFailed(code: -1)
        #expect(initError.localizedDescription.contains("error code: -1"))
        
        let paramError = TransportServicesError.invalidParameter
        #expect(paramError.localizedDescription.contains("Invalid parameter"))
        
        let connError = TransportServicesError.connectionFailed(message: "Network unreachable")
        #expect(connError.localizedDescription.contains("Network unreachable"))
    }
}

@Suite("Integration Tests", .disabled("Requires full implementation"))
struct IntegrationTests {
    
    @Test("Can establish a connection")
    func testConnectionEstablishment() async throws {
        // This test is disabled until we implement the async wrappers
        let preconnection = try Preconnection()
        let connection = try await preconnection.initiate()
        try await connection.close()
    }
    
    @Test("Can send and receive data")
    func testDataTransfer() async throws {
        // This test is disabled until we implement the async wrappers
        let preconnection = try Preconnection()
        let connection = try await preconnection.initiate()
        
        let testData = "Hello, Transport Services!".data(using: .utf8)!
        try await connection.send(testData)
        
        let receivedData = try await connection.receive()
        #expect(receivedData == testData)
        
        try await connection.close()
    }
}