#!/bin/bash
# Test Linux build in Docker

docker run --rm -v $(pwd):/workspace -w /workspace rust:1.83-slim sh -c "
    apt-get update && apt-get install -y build-essential pkg-config libssl-dev
    cargo build --example path_monitor_detailed --release
"