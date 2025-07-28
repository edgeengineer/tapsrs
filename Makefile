# Makefile for building TAPS library for multiple platforms

.PHONY: all clean test build-all ios android linux windows macos

# Default target
all: build-all

# Build for all platforms
build-all: macos ios android linux windows

# macOS (both Intel and Apple Silicon)
macos:
	@echo "Building for macOS (Apple Silicon)..."
	cargo build --release --target aarch64-apple-darwin
	@echo "Building for macOS (Intel)..."
	cargo build --release --target x86_64-apple-darwin
	@echo "Creating universal macOS library..."
	@mkdir -p target/universal-macos/release
	lipo -create \
		target/aarch64-apple-darwin/release/libtaps.a \
		target/x86_64-apple-darwin/release/libtaps.a \
		-output target/universal-macos/release/libtaps.a

# iOS
ios:
	@echo "Building for iOS..."
	cargo build --release --target aarch64-apple-ios

# Android (multiple architectures)
android:
	@echo "Building for Android (ARM64)..."
	cargo build --release --target aarch64-linux-android

# Linux (multiple architectures)
linux:
	@echo "Building for Linux (x86_64)..."
	cargo build --release --target x86_64-unknown-linux-gnu
	@echo "Building for Linux (ARM64)..."
	cargo build --release --target aarch64-unknown-linux-gnu

# Windows (multiple architectures)
windows:
	@echo "Building for Windows (x86_64)..."
	cargo build --release --target x86_64-pc-windows-msvc
	@echo "Building for Windows (ARM64)..."
	cargo build --release --target aarch64-pc-windows-msvc

# Build with FFI support
ffi:
	@echo "Building with FFI support..."
	cargo build --release --features ffi

# Generate C headers
headers:
	@echo "Generating C headers..."
	cargo build --features ffi
	cbindgen --config cbindgen.toml --crate taps --output taps.h

# Run tests
test:
	cargo test --all-features

# Run clippy
clippy:
	cargo clippy --all-features -- -D warnings

# Format code
fmt:
	cargo fmt

# Clean build artifacts
clean:
	cargo clean
	rm -f taps.h

# Build documentation
doc:
	cargo doc --all-features --no-deps --open

# Check all targets compile
check-all:
	cargo check --target aarch64-apple-ios
	cargo check --target aarch64-apple-darwin
	cargo check --target x86_64-unknown-linux-gnu
	cargo check --target aarch64-unknown-linux-gnu
	cargo check --target aarch64-linux-android
	cargo check --target x86_64-pc-windows-msvc
	cargo check --target aarch64-pc-windows-msvc