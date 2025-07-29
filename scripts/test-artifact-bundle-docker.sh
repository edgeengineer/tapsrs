#!/bin/bash

# Test script for artifact bundle creation in Docker
# This tests Linux and Android builds

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Testing artifact bundle creation in Docker..."

# Build a simplified Docker image for testing
cat > "$PROJECT_ROOT/Dockerfile.test-build" << 'EOF'
FROM rust:1.83-slim

# Install build essentials
RUN apt-get update && apt-get install -y \
    build-essential \
    curl \
    git \
    pkg-config \
    libssl-dev \
    zip \
    gcc-aarch64-linux-gnu \
    g++-aarch64-linux-gnu \
    && rm -rf /var/lib/apt/lists/*

# Install Rust targets for Linux
RUN rustup target add \
    x86_64-unknown-linux-gnu \
    aarch64-unknown-linux-gnu

# Install cbindgen
RUN cargo install cbindgen

# Set up cargo config
RUN mkdir -p /root/.cargo && \
    echo '[target.aarch64-unknown-linux-gnu]' >> /root/.cargo/config.toml && \
    echo 'linker = "aarch64-linux-gnu-gcc"' >> /root/.cargo/config.toml

WORKDIR /workspace
EOF

# Build the test Docker image
echo "Building Docker image..."
docker build -f "$PROJECT_ROOT/Dockerfile.test-build" -t transport-services-test-builder "$PROJECT_ROOT"

# Run the build in Docker
echo "Running build in Docker..."
docker run --rm \
    -v "$PROJECT_ROOT:/workspace" \
    -w /workspace \
    transport-services-test-builder \
    bash -c "
        set -euo pipefail
        
        # Create build directory
        mkdir -p build/transport_services.artifactbundle
        
        # Generate headers
        echo 'Generating headers...'
        cbindgen --config cbindgen.toml --crate transport_services --output build/transport_services.h
        
        # Create module map
        cat > build/module.modulemap << 'MODULEMAP'
module TransportServices {
    header \"transport_services.h\"
    export *
}
MODULEMAP
        
        # Build for Linux x86_64
        echo 'Building for Linux x86_64...'
        cargo build --release --target x86_64-unknown-linux-gnu --features ffi
        
        # Create variant directory
        mkdir -p build/transport_services.artifactbundle/transport_services/linux-x86_64/{lib,include}
        cp target/x86_64-unknown-linux-gnu/release/libtransport_services.a \
           build/transport_services.artifactbundle/transport_services/linux-x86_64/lib/
        cp build/transport_services.h \
           build/transport_services.artifactbundle/transport_services/linux-x86_64/include/
        cp build/module.modulemap \
           build/transport_services.artifactbundle/transport_services/linux-x86_64/include/
        
        # Build for Linux ARM64
        echo 'Building for Linux ARM64...'
        cargo build --release --target aarch64-unknown-linux-gnu --features ffi
        
        # Create variant directory
        mkdir -p build/transport_services.artifactbundle/transport_services/linux-arm64/{lib,include}
        cp target/aarch64-unknown-linux-gnu/release/libtransport_services.a \
           build/transport_services.artifactbundle/transport_services/linux-arm64/lib/
        cp build/transport_services.h \
           build/transport_services.artifactbundle/transport_services/linux-arm64/include/
        cp build/module.modulemap \
           build/transport_services.artifactbundle/transport_services/linux-arm64/include/
        
        # Create manifest
        cat > build/transport_services.artifactbundle/info.json << 'MANIFEST'
{
    \"schemaVersion\": \"1.0\",
    \"artifacts\": {
        \"transport_services\": {
            \"version\": \"0.1.0\",
            \"type\": \"staticLibrary\",
            \"variants\": [
                {
                    \"path\": \"transport_services/linux-x86_64/lib/libtransport_services.a\",
                    \"supportedTriples\": [\"x86_64-unknown-linux-gnu\"],
                    \"staticLibraryMetadata\": {
                        \"headerPaths\": [\"transport_services/linux-x86_64/include\"],
                        \"moduleMapPath\": \"transport_services/linux-x86_64/include/module.modulemap\"
                    }
                },
                {
                    \"path\": \"transport_services/linux-arm64/lib/libtransport_services.a\",
                    \"supportedTriples\": [\"aarch64-unknown-linux-gnu\"],
                    \"staticLibraryMetadata\": {
                        \"headerPaths\": [\"transport_services/linux-arm64/include\"],
                        \"moduleMapPath\": \"transport_services/linux-arm64/include/module.modulemap\"
                    }
                }
            ]
        }
    }
}
MANIFEST
        
        # Create zip file
        cd build
        zip -r transport_services-linux.zip transport_services.artifactbundle
        
        echo 'Build complete!'
        echo 'Contents of artifact bundle:'
        find transport_services.artifactbundle -type f | sort
    "

# Clean up
rm -f "$PROJECT_ROOT/Dockerfile.test-build"

echo "Test complete! Check build/transport_services-linux.zip"