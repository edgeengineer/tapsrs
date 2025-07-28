# TAPS Implementation Plan

This document outlines the plan for implementing a Transport Services (TAPS) library in Rust, based on RFC 9622. The library will be a static library with a C-FFI interface, supporting cross-compilation for various platforms.

## Phase 1: Project Setup & Core Structure

This phase focuses on setting up the basic project structure, build system, and C-FFI interface.

- [ ] Initialize a new Rust library project using `cargo new --lib tapsrs`.
- [ ] Set up the `Cargo.toml` file with necessary dependencies:
    - `tokio` (with `full` features) for the async runtime.
    - `tokio-rustls` for TLS support.
    - `rustls` and `rustls-pki-types` for certificate handling.
    - `libc` for C types.
    - `cbindgen` for C header generation.
    - `log` and `env_logger` for logging.
- [ ] Create a `Makefile` with the following targets:
    - `build`: Builds the Rust library.
    - `test`: Runs the Rust tests.
    - `header`: Generates the C header file (`taps.h`).
    - `clean`: Cleans the build artifacts.
    - Cross-compilation targets (to be added in Phase 6).
- [ ] Define the initial C-FFI layer in `src/lib.rs`.
    - Create placeholder functions for the main TAPS actions (`NewPreconnection`, `Initiate`, `Listen`, etc.).
    - Use `#[no_mangle]` and `extern "C"` for all exported functions.
    - Define opaque structs for TAPS objects (`Preconnection`, `Connection`, etc.) to be used as pointers in C.
- [ ] Set up `cbindgen.toml` to configure C header generation.

## Phase 2: Pre-establishment Implementation

This phase implements the objects and properties required before a connection is established.

- [ ] Implement the `Preconnection` object.
    - It will hold `LocalEndpoint`, `RemoteEndpoint`, `TransportProperties`, and `SecurityParameters`.
- [ ] Implement `Endpoint` objects (`LocalEndpoint` and `RemoteEndpoint`).
    - Support for `HostName`, `Port`, `Service`, `IPAddress`, and `Interface`.
    - `With...` methods for configuration.
- [ ] Implement `TransportProperties` object.
    - `Set` method for properties.
    - Convenience methods (`Require`, `Prefer`, etc.).
    - Implement all selection properties from RFC 9622 Section 6.2.
- [ ] Implement `SecurityParameters` object.
    - Support for security protocols, certificates, and other parameters from RFC 9622 Section 6.3.
    - Use `tokio-rustls` and `rustls` types for TLS configuration.

## Phase 3: Connection Establishment

This phase implements the connection establishment logic.

- [ ] Implement the `Initiate` action.
    - Takes a `Preconnection`.
    - Returns a `Connection` object.
    - Use `tokio::net::TcpStream` and `tokio_rustls` for TCP+TLS connections.
    - Handle async connection logic.
    - Implement `Ready` and `EstablishmentError` events using callbacks or a polling mechanism.
- [ ] Implement the `Listen` action.
    - Takes a `Preconnection`.
    - Returns a `Listener` object.
    - Use `tokio::net::TcpListener`.
    - Implement the `ConnectionReceived` event.
- [ ] Implement the `Rendezvous` action (P2P).
    - This is a more complex feature and can be a stretch goal. It would require ICE/STUN/TURN logic, which is out of scope for the initial `tokio-rustls` implementation but can be added later. For now, it can return `EstablishmentError`.
- [ ] Implement `Connection` object.
    - It will hold the state of the connection (e.g., the `tokio` socket and TLS session).

## Phase 4: Data Transfer

This phase implements sending and receiving data.

- [ ] Implement the `Send` action on the `Connection` object.
    - Handle `messageData` and `messageContext`.
    - Implement partial sends (`endOfMessage`).
    - Implement `Sent`, `Expired`, and `SendError` events.
- [ ] Implement the `Receive` action on the `Connection` object.
    - Handle `minIncompleteLength` and `maxLength`.
    - Implement `Received` and `ReceivedPartial` events.
- [ ] Implement `Message` and `MessageContext` objects.
    - `MessageContext` to hold `MessageProperties`.
    - Implement `MessageProperties` from RFC 9622 Section 9.1.3.

## Phase 5: Connection Management & Termination

This phase covers managing and closing connections.

- [ ] Implement `ConnectionProperties` querying on the `Connection` object.
    - `GetProperties` action.
    - Read-only properties.
- [ ] Implement `Connection` lifecycle events (`SoftError`, `PathChange`).
    - These might be placeholders initially, as they depend on deeper OS integration.
- [ ] Implement `Close` and `Abort` actions on the `Connection` object.
- [ ] Implement `ConnectionGroup` and `Clone` functionality. This can be a stretch goal.

## Phase 6: Cross-Compilation and Packaging

This phase focuses on building the static library for all target platforms.

- [ ] Add cross-compilation targets to the `Makefile`.
- [ ] Install cross-compilation toolchains using `rustup`.
    - `aarch64-apple-ios`
    - `aarch64-apple-darwin`
    - `x86_64-unknown-linux-gnu`
    - `aarch64-unknown-linux-gnu`
    - `x86_64-pc-windows-msvc`
    - `aarch64-pc-windows-msvc`
    - `aarch64-linux-android`
- [ ] Configure `cargo` for cross-compilation in `.cargo/config.toml` for linkers.
- [ ] Write scripts to build the static library (`.a` or `.lib`) for each target.
- [ ] Package the static library and the C header (`taps.h`) for distribution.

## Phase 7: Testing & Refinement

This phase focuses on ensuring the library is robust and correct.

- [ ] Write unit tests for individual components.
- [ ] Write integration tests for the C-FFI layer.
    - Create a simple C program that uses the `tapsrs` library to test the API.
- [ ] Set up Continuous Integration (e.g., GitHub Actions) to run tests and builds on different platforms.
- [ ] Refine the implementation based on testing feedback. 