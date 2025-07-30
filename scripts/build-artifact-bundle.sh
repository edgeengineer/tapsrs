#!/usr/bin/env bash

# Build script for creating Transport Services artifact bundle
# Supports all target platforms: iOS, tvOS, macOS, watchOS, visionOS (devices and simulators for Apple Silicon),
# Android ARM64, Linux x86_64 and ARM64, Windows x86_64 and ARM64

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="$PROJECT_ROOT/build"
ARTIFACT_BUNDLE_DIR="$BUILD_DIR/transport_services.artifactbundle"
ARTIFACT_NAME="TransportServicesFFI"
VERSION="0.1.0"

# Target platforms and architectures
# Note: tvOS, watchOS, and visionOS targets are not available in stable Rust
PLATFORMS=(
    # Apple device targets
    "ios-arm64"              # iPhone/iPad
    "macos-arm64"            # Apple Silicon Mac
    # "tvos-arm64"           # Apple TV - NOT AVAILABLE IN RUST
    # "watchos-arm64"        # Apple Watch - NOT AVAILABLE IN RUST
    # "visionos-arm64"       # Vision Pro - NOT AVAILABLE IN RUST
    
    # Apple simulator targets
    "ios-sim-arm64"          # iOS Simulator on Apple Silicon
    
    # Android targets
    "android-arm64"          # Android ARM64
    
    # Linux targets
    "linux-x86_64"           # Linux x86_64
    "linux-arm64"            # Linux ARM64
    
    # Windows targets
    "windows-x86_64"         # Windows 11 x86_64
    # "windows-arm64"          # Windows 11 ARM64 - NOT SUPPORTED WITH MINGW
)

RUST_TARGETS=(
    # Apple device targets
    "aarch64-apple-ios"
    "aarch64-apple-darwin"
    
    # Apple simulator targets
    "aarch64-apple-ios-sim"
    
    # Android targets
    "aarch64-linux-android"
    
    # Linux targets
    "x86_64-unknown-linux-gnu"
    "aarch64-unknown-linux-gnu"
    
    # Windows targets
    "x86_64-pc-windows-gnu"
    # "aarch64-pc-windows-gnu"  # NOT SUPPORTED
)

# Function to get rust target for a platform
get_rust_target() {
    local platform=$1
    for i in "${!PLATFORMS[@]}"; do
        if [[ "${PLATFORMS[$i]}" == "$platform" ]]; then
            echo "${RUST_TARGETS[$i]}"
            return
        fi
    done
    echo ""
}

# Initialize build environment
init_build() {
    echo "Initializing build environment..."
    rm -rf "$BUILD_DIR"
    mkdir -p "$BUILD_DIR"
    mkdir -p "$ARTIFACT_BUNDLE_DIR"
}

# Install required Rust targets
install_rust_targets() {
    echo "Installing Rust targets..."
    for target in "${RUST_TARGETS[@]}"; do
        rustup target add "$target" || true
    done
}

# Generate C headers using cbindgen
generate_headers() {
    echo "Generating C headers..."
    
    # Ensure cbindgen is installed
    if ! command -v cbindgen &> /dev/null; then
        cargo install cbindgen
    fi
    
    # Generate header file
    cd "$PROJECT_ROOT"
    cbindgen --config cbindgen.toml --crate transport_services --output "$BUILD_DIR/transport_services.h"
    
    # Generate module map
    cat > "$BUILD_DIR/module.modulemap" << EOF
module TransportServicesFFI {
    header "transport_services.h"
    export *
}
EOF
}

# Build static library for a specific target
build_target() {
    local platform=$1
    local rust_target=$2
    local original_rust_target=$rust_target
    local variant_dir="$ARTIFACT_BUNDLE_DIR/$ARTIFACT_NAME/$platform"
    
    echo "Building for $platform ($rust_target)..."
    
    mkdir -p "$variant_dir/lib"
    mkdir -p "$variant_dir/include"
    
    # Clear potentially conflicting environment variables
    unset CC
    unset CXX
    unset AR
    unset ANDROID_NDK_ROOT
    unset ANDROID_NDK
    
    # Set up cross-compilation environment
    case "$platform" in
        ios-*|tvos-*|macos-*|watchos-*|visionos-*)
            # Apple platforms - use default toolchain
            ;;
        android-*)
            # Android requires NDK - skip if not available
            if [ -z "${ANDROID_NDK_HOME:-}" ]; then
                echo "Skipping Android build - ANDROID_NDK_HOME not set"
                rm -rf "$variant_dir"
                return
            fi
            # Set NDK environment variables that aws-lc-sys expects
            export ANDROID_NDK_ROOT="$ANDROID_NDK_HOME"
            export ANDROID_NDK="$ANDROID_NDK_HOME"  # aws-lc-sys needs this
            export CC="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android30-clang"
            export AR="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-ar"
            export CXX="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android30-clang++"
            # Point CMAKE to the actual executable as per issue #819
            export CMAKE="/opt/homebrew/bin/cmake"
            # Set CMake toolchain file for Android
            export CMAKE_TOOLCHAIN_FILE="$ANDROID_NDK_HOME/build/cmake/android.toolchain.cmake"
            export CMAKE_TOOLCHAIN_FILE_aarch64_linux_android="$ANDROID_NDK_HOME/build/cmake/android.toolchain.cmake"
            # Set Android-specific CMake variables
            export ANDROID_ABI="arm64-v8a"
            export ANDROID_PLATFORM="android-30"
            ;;
        linux-*)
            # Linux cross-compilation
            if [[ "$rust_target" == "aarch64-unknown-linux-gnu" ]]; then
                export CC="aarch64-linux-gnu-gcc"
                export AR="aarch64-linux-gnu-ar"
            fi
            ;;
        windows-*)
            # Windows cross-compilation from macOS using MinGW
            if command -v x86_64-w64-mingw32-gcc &> /dev/null; then
                export CC="x86_64-w64-mingw32-gcc"
                export AR="x86_64-w64-mingw32-ar"
                export CXX="x86_64-w64-mingw32-g++"
            else
                echo "MinGW x86_64 compiler not found - skipping Windows build"
                rm -rf "$variant_dir"
                return
            fi
            ;;
    esac
    
    # Build the static library in a subshell to isolate environment
    cd "$PROJECT_ROOT"
    if [[ "$platform" == android-* ]]; then
        # Direct Android build without cargo-ndk to avoid CMake issues
        if ! (
            cargo build --release --target "$rust_target" --features ffi
        ); then
            echo "Failed to build for $platform - skipping"
            rm -rf "$variant_dir"
            return
        fi
    else
        if ! (
            cargo build --release --target "$rust_target" --features ffi
        ); then
            echo "Failed to build for $platform - skipping"
            rm -rf "$variant_dir"
            return
        fi
    fi
    
    # Copy the built library
    local lib_name
    case "$platform" in
        windows-*)
            lib_name="transport_services.lib"
            cp "target/$rust_target/release/libtransport_services.a" "$variant_dir/lib/$lib_name" || \
            cp "target/$rust_target/release/transport_services.lib" "$variant_dir/lib/$lib_name"
            ;;
        *)
            lib_name="libtransport_services.a"
            cp "target/$rust_target/release/$lib_name" "$variant_dir/lib/"
            ;;
    esac
    
    # Copy headers
    cp "$BUILD_DIR/transport_services.h" "$variant_dir/include/"
    cp "$BUILD_DIR/module.modulemap" "$variant_dir/include/"
}

# Create artifact bundle manifest
create_manifest() {
    echo "Creating artifact bundle manifest..."
    
    local variants_json=""
    
    for i in "${!PLATFORMS[@]}"; do
        local platform="${PLATFORMS[$i]}"
        local rust_target="$(get_rust_target "$platform")"
        local variant_dir="$ARTIFACT_BUNDLE_DIR/$ARTIFACT_NAME/$platform"
        
        # Skip if the variant wasn't built
        if [ ! -d "$variant_dir" ]; then
            continue
        fi
        
        local lib_path
        
        case "$platform" in
            windows-*)
                lib_path="$ARTIFACT_NAME/$platform/lib/transport_services.lib"
                ;;
            *)
                lib_path="$ARTIFACT_NAME/$platform/lib/libtransport_services.a"
                ;;
        esac
        
        if [ -n "$variants_json" ]; then
            variants_json+=","
        fi
        
        variants_json+="
                {
                    \"path\": \"$lib_path\",
                    \"supportedTriples\": [\"$rust_target\"],
                    \"staticLibraryMetadata\": {
                        \"headerPaths\": [\"$ARTIFACT_NAME/$platform/include\"],
                        \"moduleMapPath\": \"$ARTIFACT_NAME/$platform/include/module.modulemap\"
                    }
                }"
    done
    
    cat > "$ARTIFACT_BUNDLE_DIR/info.json" << EOF
{
    "schemaVersion": "1.0",
    "artifacts": {
        "$ARTIFACT_NAME": {
            "version": "$VERSION",
            "type": "staticLibrary",
            "variants": [$variants_json
            ]
        }
    }
}
EOF
}

# Create artifact bundle index for split distribution
create_bundle_index() {
    echo "Creating artifact bundle index..."
    
    local bundles_json=""
    local bundle_groups=(
        "apple:ios-arm64,tvos-arm64,macos-arm64,watchos-arm64,visionos-arm64,ios-sim-arm64,tvos-sim-arm64,watchos-sim-arm64,visionos-sim-arm64"
        "android:android-arm64"
        "linux:linux-x86_64,linux-arm64"
        "windows:windows-x86_64,windows-arm64"
    )
    
    for group in "${bundle_groups[@]}"; do
        local name="${group%%:*}"
        local platforms="${group#*:}"
        local zip_name="transport_services-$name.zip"
        
        # Create separate bundle for this group
        local group_bundle_dir="$BUILD_DIR/transport_services-$name.artifactbundle"
        mkdir -p "$group_bundle_dir"
        
        # Copy relevant variants
        IFS=',' read -ra platform_array <<< "$platforms"
        local group_variants_json=""
        
        for platform in "${platform_array[@]}"; do
            if [ -d "$ARTIFACT_BUNDLE_DIR/$ARTIFACT_NAME/$platform" ]; then
                mkdir -p "$group_bundle_dir/$ARTIFACT_NAME"
                cp -r "$ARTIFACT_BUNDLE_DIR/$ARTIFACT_NAME/$platform" "$group_bundle_dir/$ARTIFACT_NAME/"
                
                local rust_target="$(get_rust_target "$platform")"
                local lib_path
                
                case "$platform" in
                    windows-*)
                        lib_path="$ARTIFACT_NAME/$platform/lib/transport_services.lib"
                        ;;
                    *)
                        lib_path="$ARTIFACT_NAME/$platform/lib/libtransport_services.a"
                        ;;
                esac
                
                if [ -n "$group_variants_json" ]; then
                    group_variants_json+=","
                fi
                
                group_variants_json+="
                {
                    \"path\": \"$lib_path\",
                    \"supportedTriples\": [\"$rust_target\"],
                    \"staticLibraryMetadata\": {
                        \"headerPaths\": [\"$ARTIFACT_NAME/$platform/include\"],
                        \"moduleMapPath\": \"$ARTIFACT_NAME/$platform/include/module.modulemap\"
                    }
                }"
            fi
        done
        
        # Create manifest for this group
        cat > "$group_bundle_dir/info.json" << EOF
{
    "schemaVersion": "1.0",
    "artifacts": {
        "$ARTIFACT_NAME": {
            "version": "$VERSION",
            "type": "staticLibrary",
            "variants": [$group_variants_json
            ]
        }
    }
}
EOF
        
        # Create zip file
        cd "$BUILD_DIR"
        zip -r "$zip_name" "$(basename "$group_bundle_dir")"
        
        # Calculate checksum
        local checksum=$(shasum -a 256 "$zip_name" | cut -d' ' -f1)
        
        # Collect supported triples
        local supported_triples=""
        for platform in "${platform_array[@]}"; do
            if [ -n "$supported_triples" ]; then
                supported_triples+=", "
            fi
            supported_triples+="\"$(get_rust_target "$platform")\""
        done
        
        if [ -n "$bundles_json" ]; then
            bundles_json+=","
        fi
        
        bundles_json+="
        {
            \"fileName\": \"$zip_name\",
            \"checksum\": \"$checksum\",
            \"supportedTriples\": [$supported_triples]
        }"
    done
    
    # Create the index file
    cat > "$BUILD_DIR/transport_services.artifactbundleindex" << EOF
{
    "schemaVersion": "1.0",
    "bundles": [$bundles_json
    ]
}
EOF
}

# Parse command line arguments
parse_args() {
    SELECTED_PLATFORMS=()
    while [[ $# -gt 0 ]]; do
        case $1 in
            -p|--platform)
                SELECTED_PLATFORMS+=("$2")
                shift 2
                ;;
            -h|--help)
                echo "Usage: $0 [-p|--platform PLATFORM] ..."
                echo "Available platforms:"
                for platform in "${PLATFORMS[@]}"; do
                    echo "  $platform"
                done
                exit 0
                ;;
            *)
                echo "Unknown option: $1"
                echo "Use -h or --help for usage information"
                exit 1
                ;;
        esac
    done
    
    # If no platforms specified, build all
    if [ ${#SELECTED_PLATFORMS[@]} -eq 0 ]; then
        SELECTED_PLATFORMS=("${PLATFORMS[@]}")
    fi
}

# Main build process
main() {
    parse_args "$@"
    
    echo "Building Transport Services artifact bundle..."
    if [ ${#SELECTED_PLATFORMS[@]} -ne ${#PLATFORMS[@]} ]; then
        echo "Building selected platforms: ${SELECTED_PLATFORMS[*]}"
    fi
    
    init_build
    install_rust_targets
    generate_headers
    
    # Track successful builds
    local successful_builds=0
    
    # Build selected targets
    for platform in "${SELECTED_PLATFORMS[@]}"; do
        local rust_target="$(get_rust_target "$platform")"
        if [ -z "$rust_target" ]; then
            echo "Error: Unknown platform '$platform'"
            continue
        fi
        build_target "$platform" "$rust_target"
        if [ -d "$ARTIFACT_BUNDLE_DIR/$ARTIFACT_NAME/$platform" ]; then
            ((successful_builds++))
        fi
    done
    
    if [ $successful_builds -eq 0 ]; then
        echo "ERROR: No targets were successfully built!"
        exit 1
    fi
    
    echo ""
    echo "Successfully built $successful_builds out of ${#SELECTED_PLATFORMS[@]} selected targets"
    
    create_manifest
    create_bundle_index
    
    # Create final zip of complete bundle
    cd "$BUILD_DIR"
    zip -r transport_services-all.zip transport_services.artifactbundle
    
    echo "Build complete! Artifacts available in $BUILD_DIR"
    echo "- Complete bundle: transport_services-all.zip"
    echo "- Split bundles with index: transport_services.artifactbundleindex"
}

# Run main if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi