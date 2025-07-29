# Transport Services Swift Bindings

Swift bindings for the Transport Services (RFC 9622) implementation.

## Overview

This package provides a Swift-friendly API for Transport Services, wrapping the underlying Rust FFI implementation.

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

## Requirements

- Swift 6.2 or later
- Platforms: macOS 15+, iOS 18+, tvOS 18+, watchOS 11+, visionOS 2+

## Implementation Status

- [x] Basic package structure
- [x] FFI binary target integration
- [x] Swift Testing setup
- [ ] Complete async/await wrappers
- [ ] Endpoint implementations
- [ ] Transport properties
- [ ] Security parameters
- [ ] Event handling
- [ ] Error handling