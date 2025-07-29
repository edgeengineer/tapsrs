# tapsrs: A Rust Implementation of the Transport Services API

[![Build Status](https://github.com/edgeengineer/tapsrs/workflows/Rust/badge.svg)](https://github.com/edgeengineer/tapsrs/actions)
[![Crates.io](https://img.shields.io/crates/v/tapsrs.svg)](https://crates.io/crates/tapsrs)
[![Docs.rs](https://docs.rs/tapsrs/badge.svg)](https://docs.rs/tapsrs)

`tapsrs` is a modern, asynchronous, and cross-platform transport services library written in Rust. It provides a flexible and protocol-agnostic API for network applications, based on the IETF Transport Services (TAPS) architecture defined in [RFC 9621](spec/rfc9621.txt) and [RFC 9622](spec/rfc9622.txt).

The primary goal of this library is to replace the traditional BSD Socket API with a higher-level, message-oriented interface that enables applications to leverage modern transport protocols and features (like QUIC, Multipath TCP, etc.) without being tightly coupled to a specific protocol.

## Vision and Goals

The TAPS architecture aims to solve the ossification of the transport layer by decoupling applications from specific transport protocols. `tapsrs` embraces this vision by providing:

- **Protocol Agility**: Automatically select the best transport protocol (e.g., TCP, QUIC) based on application requirements and network conditions.
- **Message-Oriented API**: A clean, asynchronous, message-based interface for all data transfer, abstracting away the differences between stream and datagram transports.
- **Path and Endpoint Flexibility**: Seamlessly manage multiple network interfaces, IP addresses, and paths, enabling features like connection racing and migration.
- **Rich Feature Set**: Expose modern transport features like multipath, 0-RTT, and per-message reliability through a consistent API.
- **Cross-Platform Static Library**: Produce a C-compatible static library (`.a`/`.lib`) that can be easily integrated into Swift, C#, Python, C++, and other language ecosystems.

## Core Concepts

The `tapsrs` API is built around a few key abstractions defined by the TAPS architecture:

- **Preconnection**: A template for creating connections. Here, you specify your desired `TransportProperties` (e.g., reliability, security, ordering) and `Endpoints` (local and remote).
- **Connection**: An active communication channel established from a `Preconnection`. It represents a logical connection that can be backed by one or more underlying transport protocols.
- **Listener**: An object that listens for incoming connections that match a given `Preconnection` configuration.
- **TransportProperties**: A set of requirements and preferences that guide the selection of transport protocols and paths. This is how an application expresses its intent (e.g., "I need reliable, in-order delivery" or "I prefer low latency over reliability").
- **Message**: The fundamental unit of data transfer. All data is sent and received as messages, which can have their own properties (e.g., lifetime, priority).

## Features

This library is a progressive implementation of the TAPS specification. Key implemented features include:

- **Pre-establishment Phase (RFC Section 6)**: Full support for `Preconnection` setup, including endpoint specification and transport property configuration.
- **Connection Establishment (RFC Section 7)**:
    - Active Open (`Initiate`)
    - Passive Open (`Listen`)
    - Connection Groups (`Clone`)
- **Data Transfer (RFC Section 9)**:
    - Asynchronous Message Sending (`Send`) and Receiving (`Receive`).
    - Support for Message Properties (lifetime, priority, ordering, etc.).
    - Partial (streaming) sends.
- **Connection Management (RFC Section 8 & 10)**:
    - Settable and read-only connection properties.
    - Graceful (`Close`) and immediate (`Abort`) termination.

For a detailed implementation status, please see the [Implementation Checklist](checklist.md).

## Getting Started

### Prerequisites

- [Rust and Cargo](https://www.rust-lang.org/tools/install)
- `cbindgen` for generating the C header file (`cargo install cbindgen`)
- Cross-compilation targets if needed (e.g., `rustup target add aarch64-apple-ios`)

#### Additional Requirements for QUIC/TLS Support

The library includes optional QUIC and TLS transport support through the `quinn` crate. To build with these features enabled (which is the default), you'll need:

**On Windows:**
- [CMake](https://cmake.org/download/) - Required by the crypto library build process
- [NASM](https://www.nasm.us/) - The Netwide Assembler for optimized cryptographic operations
  - Install via [Chocolatey](https://chocolatey.org/): `choco install cmake nasm`
  - Or download and install manually from the links above

**On macOS:**
- CMake and NASM can be installed via Homebrew: `brew install cmake nasm`

**On Linux:**
- Install via package manager: `sudo apt-get install cmake nasm` (Ubuntu/Debian)
- Or: `sudo yum install cmake nasm` (RHEL/CentOS)

**Note:** If you don't need QUIC/TLS support, you can build without these dependencies:
```sh
cargo build --release --no-default-features
```

### Building the Library

1.  **Clone the repository:**
    ```sh
    git clone https://github.com/edgeengineer/tapsrs.git
    cd tapsrs
    ```

2.  **Build the static library:**
    ```sh
    cargo build --release
    ```
    This will produce the static library in `target/release/libtapsrs.a` (or `tapsrs.lib` on Windows).

3.  **Generate the C header file:**
    ```sh
    cbindgen --config cbindgen.toml --crate tapsrs --output include/tapsrs.h
    ```

## Usage Example (C-FFI)

The primary interface for non-Rust languages is the C-compatible FFI. Here is a simple example of a client that connects to `example.com` and sends a message.

```c
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include "tapsrs.h"

// Global state to track if the connection is ready
int is_ready = 0;

void on_connection_ready(TransportServicesHandle* connection, void* user_data) {
    printf("Connection is ready!\n");
    is_ready = 1;
}

void on_send_complete(TransportServicesError error, const char* error_message, void* user_data) {
    if (error == TRANSPORT_SERVICES_ERROR_SUCCESS) {
        printf("Message sent successfully!\n");
    } else {
        printf("Send failed: %s\n", error_message);
    }
}

void on_error(TransportServicesError error, const char* error_message, void* user_data) {
    printf("An error occurred: %s\n", error_message);
}

int main() {
    transport_services_init();

    // 1. Create a Preconnection
    TransportServicesHandle* preconnection = transport_services_preconnection_new();

    // 2. Specify the Remote Endpoint
    TransportServicesEndpoint remote_endpoint = {
        .hostname = "example.com",
        .port = 443,
        .service = "https",
    };
    transport_services_preconnection_add_remote_endpoint(preconnection, &remote_endpoint);

    // 3. Initiate the Connection
    printf("Initiating connection...\n");
    transport_services_preconnection_initiate(preconnection, on_connection_ready, on_error, NULL);

    // Wait for the connection to be ready (in a real app, this would be event-driven)
    while (!is_ready) {
        sleep(1);
    }

    // 4. Send a Message
    const char* http_request = "GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
    TransportServicesMessage message = {
        .data = (const uint8_t*)http_request,
        .length = strlen(http_request),
    };
    transport_services_connection_send(preconnection, &message, on_send_complete, NULL);

    // Clean up
    sleep(2); // Wait for send to complete
    transport_services_preconnection_free(preconnection);
    transport_services_cleanup();

    return 0;
}
```

## Cross-Platform Support

`tapsrs` is designed to be highly portable and is tested against the following targets:

- **macOS**: `aarch64-apple-darwin`, `x86_64-apple-darwin`
- **iOS**: `aarch64-apple-ios`
- **Linux**: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`
- **Android**: `aarch64-linux-android`
- **Windows**: `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`

## Contributing

Contributions are welcome! Please feel free to open an issue or submit a pull request.

### Setting Up Git Hooks

This project uses git hooks to ensure code quality. To set up the pre-commit hook that runs `cargo fmt`:

**On Unix/Linux/macOS:**
```bash
./setup-hooks.sh
```

**On Windows:**
```cmd
setup-hooks.bat
```

This will configure git to run `cargo fmt --check` before each commit, ensuring all code is properly formatted.

## License

This project is licensed under the MIT License.
