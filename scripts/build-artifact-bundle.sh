#!/bin/bash

# Build script for creating Transport Services artifact bundle
# Supports all target platforms: iOS, tvOS, macOS, watchOS, visionOS (devices and simulators for Apple Silicon),
# Android ARM64, Linux x86_64 and ARM64, Windows x86_64 and ARM64

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="$PROJECT_ROOT/build"
ARTIFACT_BUNDLE_DIR="$BUILD_DIR/transport_services.artifactbundle"
ARTIFACT_NAME="transport_services"
VERSION="0.1.0"

# Target platforms and architectures
declare -A TARGETS=(
    # Apple device targets
    ["ios-arm64"]="aarch64-apple-ios"
    ["tvos-arm64"]="aarch64-apple-tvos"
    ["macos-arm64"]="aarch64-apple-darwin"
    ["watchos-arm64"]="aarch64-apple-watchos"
    ["visionos-arm64"]="aarch64-apple-visionos"
    
    # Apple simulator targets (Apple Silicon only)
    ["ios-sim-arm64"]="aarch64-apple-ios-sim"
    ["tvos-sim-arm64"]="aarch64-apple-tvos-sim"
    ["watchos-sim-arm64"]="aarch64-apple-watchos-sim"
    ["visionos-sim-arm64"]="aarch64-apple-visionos-sim"
    
    # Other platforms
    ["android-arm64"]="aarch64-linux-android"
    ["linux-x86_64"]="x86_64-unknown-linux-gnu"
    ["linux-arm64"]="aarch64-unknown-linux-gnu"
    ["windows-x86_64"]="x86_64-pc-windows-msvc"
    ["windows-arm64"]="aarch64-pc-windows-msvc"
)

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
    for target in "${TARGETS[@]}"; do
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
module TransportServices {
    header "transport_services.h"
    export *
}
EOF
}

# Build static library for a specific target
build_target() {
    local platform=$1
    local rust_target=$2
    local variant_dir="$ARTIFACT_BUNDLE_DIR/$ARTIFACT_NAME/$platform"
    
    echo "Building for $platform ($rust_target)..."
    
    mkdir -p "$variant_dir/lib"
    mkdir -p "$variant_dir/include"
    
    # Set up cross-compilation environment
    case "$platform" in
        ios-*|tvos-*|macos-*|watchos-*|visionos-*)
            # Apple platforms - use default toolchain
            ;;
        android-*)
            # Android requires NDK
            export CC="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android30-clang"
            export AR="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
            ;;
        linux-*)
            # Linux cross-compilation
            if [[ "$rust_target" == "aarch64-unknown-linux-gnu" ]]; then
                export CC="aarch64-linux-gnu-gcc"
                export AR="aarch64-linux-gnu-ar"
            fi
            ;;
        windows-*)
            # Windows cross-compilation from Linux
            export CC="x86_64-w64-mingw32-gcc"
            export AR="x86_64-w64-mingw32-ar"
            ;;
    esac
    
    # Build the static library
    cd "$PROJECT_ROOT"
    cargo build --release --target "$rust_target" --features ffi
    
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
    
    for platform in "${!TARGETS[@]}"; do
        local rust_target="${TARGETS[$platform]}"
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
                
                local rust_target="${TARGETS[$platform]}"
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
            supported_triples+="\"${TARGETS[$platform]}\""
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

# Main build process
main() {
    echo "Building Transport Services artifact bundle..."
    
    init_build
    install_rust_targets
    generate_headers
    
    # Build all targets
    for platform in "${!TARGETS[@]}"; do
        build_target "$platform" "${TARGETS[$platform]}"
    done
    
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