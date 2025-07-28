# Transport Services Rust Implementation Checklist

This document outlines the phases and steps required to implement the TAPS (Transport Services) API in a Rust crate named `TransportServices`, as specified in RFC 9622. The primary goal is to produce a static library compatible with iOS, macOS, Linux, Android, and Windows, using Tokio for asynchronous operations.

## Phase 0: Project Setup & Core Abstractions

- [x] Initialize a new Rust library crate named `TransportServices`.
- [x] Define core data structures based on **RFC Section 1.1 (Terminology and Notation)** (e.g., custom types for `Preference`, `EndpointIdentifier`, etc.).
- [x] Define the primary public objects as Rust structs (**RFC Section 3, API Summary**): `Preconnection`, `Connection`, `Listener`, `Message`, `MessageContext`.
- [x] Set up cross-compilation toolchains and targets in `rustup`:
    - [x] `aarch64-apple-ios`
    - [x] `aarch64-apple-darwin` (Apple Silicon macOS)
    - [x] `x86_64-unknown-linux-gnu`
    - [x] `aarch64-unknown-linux-gnu`
    - [x] `aarch64-linux-android`
    - [x] `x86_64-pc-windows-msvc`
    - [x] `aarch64-pc-windows-msvc`
- [x] Configure `Cargo.toml` to produce a `staticlib` crate type.
- [x] Integrate `tokio` as the core asynchronous runtime.
- [x] Design a C-compatible Foreign Function Interface (FFI) layer for the public API to ensure interoperability with Swift, Kotlin/JNI, and C++ (**RFC Appendix A, Implementation Mapping**).

## Phase 1: Pre-establishment (RFC Section 6, Preestablishment Phase)

- [x] Implement the `Preconnection` object and its creation (`NewPreconnection`) as defined in **RFC Section 6**.
- [x] Implement Endpoint specification (**RFC Section 6.1, Specifying Endpoints**):
    - [x] `LocalEndpoint` and `RemoteEndpoint` structs.
    - [x] Builder methods for setting identifiers: `WithHostName`, `WithPort`, `WithService`, `WithIPAddress`, `WithInterface`.
    - [x] Support for Multicast endpoints (**RFC Section 6.1.1, Using Multicast Endpoints**).
    - [x] Support for Protocol-Specific endpoints (**RFC Section 6.1.3, Protocol-Specific Endpoints**).
- [x] Implement Transport Properties specification (**RFC Section 4, Transport Properties & 6.2, Specifying Transport Properties**):
    - [x] `TransportProperties` struct.
    - [x] A `Set` method for properties, likely using an enum to represent property keys.
    - [x] Implement all Selection Properties with a `Preference` enum (Require, Prefer, Avoid, Prohibit, NoPreference) as defined in **RFC Section 6.2**:
        - [x] `reliability` (**6.2.1, Reliable Data Transfer (Connection)**)
        - [x] `preserveMsgBoundaries` (**6.2.2, Preservation of Message Boundaries**)
        - [x] `perMsgReliability` (**6.2.3, Configure Per-Message Reliability**)
        - [x] `preserveOrder` (**6.2.4, Preservation of Data Ordering**)
        - [x] `zeroRttMsg` (**6.2.5, Use 0-RTT Session Establishment with a Safely Replayable Message**)
        - [x] `multistreaming` (**6.2.6, Multistream Connections in a Group**)
        - [x] `fullChecksumSend` / `fullChecksumRecv` (**6.2.7, Full Checksum Coverage on Sending**, **6.2.8, Full Checksum Coverage on Receiving**)
        - [x] `congestionControl` (**6.2.9, Congestion Control**)
        - [x] `keepAlive` (**6.2.10, Keep-Alive Packets**)
        - [x] `interface` (**6.2.11, Interface Instance or Type**)
        - [x] `pvd` (**6.2.12, Provisioning Domain Instance or Type**)
        - [x] `useTemporaryLocalAddress` (**6.2.13, Use Temporary Local Address**)
        - [x] `multipath` (**6.2.14, Multipath Transport**)
        - [x] `advertisesAltaddr` (**6.2.15, Advertisement of Alternative Addresses**)
        - [x] `direction` (**6.2.16, Direction of Communication**)
        - [x] `softErrorNotify` (**6.2.17, Notification of ICMP Soft Error Message Arrival**)
        - [x] `activeReadBeforeSend` (**6.2.18, Initiating Side Is Not the First to Write**)
- [x] Implement Security Parameters specification (**RFC Section 6.3, Specifying Security Parameters and Callbacks**):
    - [x] `SecurityParameters` struct.
    - [x] Functions for disabled and opportunistic security.
    - [x] `Set` method for parameters like `allowedSecurityProtocols` (**6.3.1, Allowed Security Protocols**), certificates (**6.3.2, Certificate Bundles**), ALPN (**6.3.4, Application-Layer Protocol Negotiation**), etc.
    - [x] Implement callback mechanisms for trust verification and identity challenges using function pointers (`extern "C" fn`) in the FFI layer (**6.3.8, Connection Establishment Callbacks**).

## Phase 2: Connection Establishment (RFC Section 7, Establishing Connections)

- [x] Implement Active Open: `Preconnection.Initiate()` (**RFC Section 7.1, Active Open: Initiate**).
    - [x] Return a `Connection` object.
    - [x] Use `tokio::net::TcpStream::connect` and other Tokio APIs for the underlying network operations.
    - [x] Implement an event system (e.g., via FFI callbacks) to signal `Ready` or `EstablishmentError`.
- [ ] Implement Passive Open: `Preconnection.Listen()` (**RFC Section 7.2, Passive Open: Listen**).
    - [ ] Return a `Listener` object.
    - [ ] Use `tokio::net::TcpListener` for asynchronous listening.
    - [ ] Emit `ConnectionReceived` events containing new `Connection` objects.
    - [ ] Implement `Listener.Stop()`.
- [ ] Implement Peer-to-Peer Establishment: `Preconnection.Rendezvous()` (**RFC Section 7.3, Peer-to-Peer Establishment: Rendezvous**).
    - [ ] This is a complex feature. Plan for a phased implementation, potentially starting with basic cases and later adding full NAT traversal (ICE-like) logic.
    - [ ] Implement `Preconnection.Resolve()` to gather candidates.
    - [ ] Emit `RendezvousDone` or `EstablishmentError` events.
- [ ] Implement Connection Groups: `Connection.Clone()` (**RFC Section 7.4, Connection Groups**).
    - [ ] Ensure shared properties are handled correctly between cloned connections.
    - [ ] Investigate mapping to underlying multistreaming protocols like QUIC if available.

## Phase 3: Data Transfer (RFC Section 9, Data Transfer)

- [ ] Implement Message Sending: `Connection.Send()` (**RFC Section 9.2, Sending Data**).
    - [ ] Handle `messageData` (e.g., `&[u8]`) and `messageContext`.
    - [ ] Support partial sends via the `endOfMessage` boolean flag (**9.2.3, Partial Sends**).
    - [ ] Support send batching (`StartBatch`/`EndBatch`) (**9.2.4, Batching Sends**).
    - [ ] Implement `InitiateWithSend` (**9.2.5, Send on Active Open: InitiateWithSend**).
- [ ] Implement Send Events via the event callback system (**RFC Section 9.2.2, Send Events**):
    - [ ] `Sent` (**9.2.2.1, Sent**)
    - [ ] `Expired` (**9.2.2.2, Expired**)
    - [ ] `SendError` (**9.2.2.3, SendError**)
- [ ] Implement Message Properties (**RFC Section 9.1.3, Message Properties**):
    - [ ] `msgLifetime` (**9.1.3.1, Lifetime**)
    - [ ] `msgPriority` (**9.1.3.2, Priority**)
    - [ ] `msgOrdered` (**9.1.3.3, Ordered**)
    - [ ] `safelyReplayable` (**9.1.3.4, Safely Replayable**)
    - [ ] `final` (**9.1.3.5, Final**)
    - [ ] `msgChecksumLen` (**9.1.3.6, Sending Corruption Protection Length**)
    - [ ] `msgReliable` (**9.1.3.7, Reliable Data Transfer (Message)**)
    - [ ] `msgCapacityProfile` (**9.1.3.8, Message Capacity Profile Override**)
    - [ ] `noFragmentation` / `noSegmentation` (**9.1.3.9, No Network-Layer Fragmentation**, **9.1.3.10, No Segmentation**)
- [ ] Implement Message Receiving: `Connection.Receive()` (**RFC Section 9.3.1, Enqueuing Receives**).
    - [ ] Handle `minIncompleteLength` and `maxLength` parameters to manage buffering.
- [ ] Implement Receive Events via the event callback system (**RFC Section 9.3.2, Receive Events**):
    - [ ] `Received` (for complete messages) (**9.3.2.1, Received**).
    - [ ] `ReceivedPartial` (for partial messages) (**9.3.2.2, ReceivedPartial**).
    - [ ] `ReceiveError` (**9.3.2.3, ReceiveError**)
- [ ] Implement a Message Framer system (**RFC Section 9.1.2, Message Framers**):
    - [ ] Define a `Framer` trait in Rust.
    - [ ] Allow adding framer implementations to a `Preconnection`. This is key for layering application protocols like HTTP over the transport.

## Phase 4: Connection Management & Termination (RFC Section 8, Managing Connections & 10, Connection Termination)

- [ ] Implement Connection Property management (**RFC Section 8.1, Generic Connection Properties**):
    - [ ] `Connection.SetProperty()` and `Connection.GetProperties()`.
    - [ ] Implement generic connection properties (e.g., `connTimeout` (**8.1.3, Timeout for Aborting Connection**), `connPriority` (**8.1.2, Connection Priority**)).
    - [ ] Implement read-only properties (e.g., `connState` (**8.1.11.1, Connection State**), `canSend` (**8.1.11.2, Can Send Data**), `canReceive` (**8.1.11.3, Can Receive Data**)).
- [ ] Implement Connection Lifecycle Events via the event callback system (**RFC Section 8.3, Connection Lifecycle Events**):
    - [ ] `SoftError` (**8.3.1, Soft Errors**)
    - [ ] `PathChange` (**8.3.2, Path Change**)
- [ ] Implement Connection Termination actions (**RFC Section 10, Connection Termination**):
    - [ ] `Connection.Close()`
    - [ ] `Connection.Abort()`
    - [ ] `Connection.CloseGroup()`
    - [ ] `Connection.AbortGroup()`
- [ ] Implement Termination Events via the event callback system (**RFC Section 10**):
    - [ ] `Closed`
    - [ ] `ConnectionError`

## Phase 5: Packaging and Distribution

- [ ] Create build scripts (e.g., `build.rs` or shell scripts) to automate the cross-compilation for all target platforms.
- [ ] Automate the generation of static libraries (`libtransport_services.a`, `transport_services.lib`) for each target architecture.
- [ ] Use `cbindgen` to automatically generate a C header file (`transport_services.h`) from the FFI layer.
- [ ] Create wrapper packages for easy integration into platform-native projects:
    - [ ] A Swift Package that bundles the `.a` and `.h` files for iOS and macOS.
    - [ ] An Android Archive (AAR) that includes the `.so` files for different Android ABIs.
    - [ ] A NuGet package for Windows developers.
- [ ] Provide comprehensive documentation and examples for using the library on each target platform.
