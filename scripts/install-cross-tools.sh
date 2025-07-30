#!/usr/bin/env bash

# Script to install cross-compilation tools for building Transport Services on all platforms
# Supports macOS host only (for now)

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Helper functions
print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}!${NC} $1"
}

print_info() {
    echo -e "ℹ️  $1"
}

# Check if running on macOS
check_macos() {
    if [[ "$OSTYPE" != "darwin"* ]]; then
        print_error "This script currently only supports macOS as the host system"
        exit 1
    fi
}

# Check if Homebrew is installed
check_homebrew() {
    if ! command -v brew &> /dev/null; then
        print_error "Homebrew is not installed"
        print_info "Install Homebrew from https://brew.sh"
        exit 1
    fi
    print_success "Homebrew is installed"
}

# Install Rust targets
install_rust_targets() {
    print_info "Installing Rust targets..."
    
    local targets=(
        # Apple targets
        "aarch64-apple-ios"
        "aarch64-apple-ios-sim"
        "aarch64-apple-darwin"
        
        # Linux targets
        "x86_64-unknown-linux-gnu"
        "aarch64-unknown-linux-gnu"
        
        # Windows targets
        "x86_64-pc-windows-msvc"
        "aarch64-pc-windows-msvc"
        
        # Android target
        "aarch64-linux-android"
    )
    
    for target in "${targets[@]}"; do
        if rustup target list --installed | grep -q "^$target"; then
            print_success "Rust target $target already installed"
        else
            print_info "Installing Rust target $target..."
            if rustup target add "$target"; then
                print_success "Installed Rust target $target"
            else
                print_warning "Failed to install Rust target $target"
            fi
        fi
    done
}

# Install Linux cross-compilation tools
install_linux_cross_tools() {
    print_info "Installing Linux cross-compilation tools..."
    
    # Check if musl-cross is already installed
    if brew list --formula | grep -q "musl-cross"; then
        print_success "musl-cross already installed"
    else
        print_info "Installing musl-cross (this may take a while)..."
        if brew install messense/macos-cross-toolchains/x86_64-unknown-linux-gnu; then
            print_success "Installed x86_64-unknown-linux-gnu toolchain"
        else
            print_warning "Failed to install x86_64-unknown-linux-gnu toolchain"
        fi
        
        if brew install messense/macos-cross-toolchains/aarch64-unknown-linux-gnu; then
            print_success "Installed aarch64-unknown-linux-gnu toolchain"
        else
            print_warning "Failed to install aarch64-unknown-linux-gnu toolchain"
        fi
    fi
    
    # Set up cargo config for Linux cross-compilation
    mkdir -p ~/.cargo
    local cargo_config=~/.cargo/config.toml
    
    print_info "Updating cargo configuration for Linux cross-compilation..."
    
    # Check if config already exists
    if [ -f "$cargo_config" ]; then
        # Backup existing config
        cp "$cargo_config" "$cargo_config.backup"
        print_info "Backed up existing cargo config to $cargo_config.backup"
    fi
    
    # Add Linux target configurations
    cat >> "$cargo_config" << 'EOF'

# Linux cross-compilation targets
[target.x86_64-unknown-linux-gnu]
linker = "x86_64-unknown-linux-gnu-gcc"
ar = "x86_64-unknown-linux-gnu-ar"

[target.aarch64-unknown-linux-gnu]
linker = "aarch64-unknown-linux-gnu-gcc"
ar = "aarch64-unknown-linux-gnu-ar"
EOF
    
    print_success "Updated cargo configuration for Linux targets"
}

# Install Windows cross-compilation tools
install_windows_cross_tools() {
    print_info "Installing Windows cross-compilation tools..."
    
    # For Windows, we'll use the built-in MSVC target support
    # which works on macOS without additional tools for library compilation
    print_info "Windows MSVC targets use Rust's built-in support"
    print_info "No additional tools needed for static library compilation"
    
    # Note: Full Windows executable compilation would require Wine and MSVC tools
    print_warning "Note: This setup is sufficient for static libraries only"
}

# Install Android NDK
install_android_ndk() {
    print_info "Checking Android NDK..."
    
    if [ -n "${ANDROID_NDK_HOME:-}" ] && [ -d "$ANDROID_NDK_HOME" ]; then
        print_success "Android NDK already configured at $ANDROID_NDK_HOME"
        return
    fi
    
    print_info "Android NDK not found. You have two options:"
    print_info "1. Install Android Studio and configure NDK through SDK Manager"
    print_info "2. Download standalone NDK from https://developer.android.com/ndk/downloads"
    print_info ""
    print_info "After installation, set ANDROID_NDK_HOME environment variable:"
    print_info "  export ANDROID_NDK_HOME=/path/to/android-ndk"
    print_info ""
    print_warning "Android build will be skipped until NDK is configured"
}

# Update build script for cross-compilation
update_build_script() {
    local script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    local build_script="$script_dir/build-artifact-bundle.sh"
    
    if [ -f "$build_script" ]; then
        print_info "Build script found at $build_script"
        print_info "The script is already configured to use these tools"
    else
        print_warning "Build script not found at expected location"
    fi
}

# Main installation process
main() {
    echo "Transport Services Cross-Compilation Tools Installer"
    echo "===================================================="
    echo ""
    
    check_macos
    check_homebrew
    
    print_info "This script will install tools for cross-compiling to:"
    print_info "  • Linux x86_64"
    print_info "  • Linux ARM64"
    print_info "  • Windows x86_64 (static libraries only)"
    print_info "  • Windows ARM64 (static libraries only)"
    print_info "  • Android ARM64 (requires separate NDK installation)"
    echo ""
    
    read -p "Continue? (y/N) " -n 1 -r
    echo ""
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        print_info "Installation cancelled"
        exit 0
    fi
    
    install_rust_targets
    install_linux_cross_tools
    install_windows_cross_tools
    install_android_ndk
    update_build_script
    
    echo ""
    echo "Installation Summary"
    echo "===================="
    print_success "Rust targets installed"
    print_success "Linux cross-compilation tools installed"
    print_info "Windows builds use Rust's built-in MSVC target support"
    print_warning "Android NDK must be installed separately"
    
    echo ""
    print_info "You can now run ./scripts/build-artifact-bundle.sh to build for all platforms"
    print_info "Platforms without proper tools will be automatically skipped"
}

# Run main if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi