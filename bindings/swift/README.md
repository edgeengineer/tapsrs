# Transport Services Swift Bindings

Modern Swift 6 bindings for the Transport Services (RFC 9622) implementation with full concurrency support.

## Overview

This package provides a Swift-friendly API for Transport Services, wrapping the underlying Rust FFI implementation with modern Swift concurrency features including async/await, AsyncSequence, and Sendable conformance.

## Building

### Local Development

To build using the local artifact bundle:

```bash
USE_LOCAL_ARTIFACT=1 swift build
```

### Testing

Run tests using Swift Testing (built into Swift 6):

```bash
USE_LOCAL_ARTIFACT=1 swift test
```

## Usage

### Basic Connection Example

```swift
import TransportServices

// Initialize the Transport Services runtime
try TransportServices.initialize()

// Create a preconnection
let preconnection = try Preconnection(
    remoteEndpoints: [/* ... */],
    transportProperties: TransportProperties()
)

// Initiate a connection
let connection = try await preconnection.initiate()

// Send data
let data = "Hello, world!".data(using: .utf8)!
try await connection.send(data)

// Receive data
let receivedData = try await connection.receive()

// Close the connection
try await connection.close()

// Cleanup when done
TransportServices.cleanup()
```

### Path Monitoring Example

```swift
import TransportServices

// Create a path monitor
let monitor = try PathMonitor()

// List current interfaces
let interfaces = try await monitor.interfaces()
for interface in interfaces {
    print("\(interface.name): \(interface.status) - \(interface.interfaceType)")
}

// Monitor network changes
for await event in monitor.changes() {
    switch event {
    case .added(let interface):
        print("Interface added: \(interface.name)")
    case .removed(let interface):
        print("Interface removed: \(interface.name)")
    case .modified(let old, let new):
        print("Interface changed: \(new.name)")
    case .pathChanged(let description):
        print("Path changed: \(description)")
    }
}
```

## Requirements

- Swift 6.2 or later
- Platforms: macOS 15+, iOS 18+, tvOS 18+, watchOS 11+, visionOS 2+

## Implementation Status

- [x] Basic package structure
- [x] FFI binary target integration
- [x] Swift Testing setup
- [x] Path monitoring with async/await
- [x] NetworkInterface type with Sendable conformance
- [x] AsyncSequence for network changes
- [x] Thread-safe actor-based implementation
- [ ] Complete connection async/await wrappers
- [ ] Endpoint implementations
- [ ] Transport properties
- [ ] Security parameters
- [ ] Connection event handling