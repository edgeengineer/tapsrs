# Build script for creating Transport Services artifact bundle on Windows
# Supports cross-compilation for multiple platforms

param(
    [string]$Target = "all"
)

$ErrorActionPreference = "Stop"

# Configuration
$SCRIPT_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
$PROJECT_ROOT = Split-Path -Parent $SCRIPT_DIR
$BUILD_DIR = Join-Path $PROJECT_ROOT "build"
$ARTIFACT_BUNDLE_DIR = Join-Path $BUILD_DIR "transport_services.artifactbundle"
$ARTIFACT_NAME = "transport_services"
$VERSION = "0.1.0"

# Target platforms and architectures
$TARGETS = @{
    "ios-arm64" = "aarch64-apple-ios"
    "macos-arm64" = "aarch64-apple-darwin"
    "macos-x86_64" = "x86_64-apple-darwin"
    "android-arm64" = "aarch64-linux-android"
    "linux-x86_64" = "x86_64-unknown-linux-gnu"
    "linux-arm64" = "aarch64-unknown-linux-gnu"
    "windows-x86_64" = "x86_64-pc-windows-msvc"
    "windows-arm64" = "aarch64-pc-windows-msvc"
}

# Initialize build environment
function Init-Build {
    Write-Host "Initializing build environment..."
    if (Test-Path $BUILD_DIR) {
        Remove-Item -Recurse -Force $BUILD_DIR
    }
    New-Item -ItemType Directory -Path $BUILD_DIR | Out-Null
    New-Item -ItemType Directory -Path $ARTIFACT_BUNDLE_DIR | Out-Null
}

# Install cbindgen if needed
function Install-Cbindgen {
    Write-Host "Checking cbindgen installation..."
    $cbindgen = Get-Command cbindgen -ErrorAction SilentlyContinue
    if (-not $cbindgen) {
        Write-Host "Installing cbindgen..."
        cargo install cbindgen
    }
}

# Generate C headers using cbindgen
function Generate-Headers {
    Write-Host "Generating C headers..."
    
    Push-Location $PROJECT_ROOT
    try {
        cbindgen --config cbindgen.toml --crate transport_services --output "$BUILD_DIR/transport_services.h"
        
        # Generate module map
        $moduleMap = @"
module TransportServices {
    header "transport_services.h"
    export *
}
"@
        $moduleMap | Out-File -FilePath "$BUILD_DIR/module.modulemap" -Encoding UTF8
    }
    finally {
        Pop-Location
    }
}

# Build static library for a specific target
function Build-Target {
    param(
        [string]$Platform,
        [string]$RustTarget
    )
    
    $variantDir = Join-Path $ARTIFACT_BUNDLE_DIR "$ARTIFACT_NAME/$Platform"
    
    Write-Host "Building for $Platform ($RustTarget)..."
    
    New-Item -ItemType Directory -Path "$variantDir/lib" -Force | Out-Null
    New-Item -ItemType Directory -Path "$variantDir/include" -Force | Out-Null
    
    Push-Location $PROJECT_ROOT
    try {
        # Build the static library
        cargo build --release --target $RustTarget --features ffi --no-default-features
        
        # Copy the built library
        $libName = switch ($Platform) {
            {$_ -match "windows-"} { "transport_services.lib" }
            default { "libtransport_services.a" }
        }
        
        $sourcePath = Join-Path "target/$RustTarget/release" $libName
        $destPath = Join-Path "$variantDir/lib" $libName
        
        if (Test-Path $sourcePath) {
            Copy-Item $sourcePath $destPath
        } else {
            # Try alternative name for Windows
            $altSourcePath = Join-Path "target/$RustTarget/release" "libtransport_services.a"
            if (Test-Path $altSourcePath) {
                Copy-Item $altSourcePath $destPath
            } else {
                Write-Warning "Library not found for $Platform"
                return
            }
        }
        
        # Copy headers
        Copy-Item "$BUILD_DIR/transport_services.h" "$variantDir/include/"
        Copy-Item "$BUILD_DIR/module.modulemap" "$variantDir/include/"
    }
    finally {
        Pop-Location
    }
}

# Create artifact bundle manifest
function Create-Manifest {
    Write-Host "Creating artifact bundle manifest..."
    
    $variants = @()
    
    foreach ($platform in $TARGETS.Keys) {
        $rustTarget = $TARGETS[$platform]
        $libPath = if ($platform -match "windows-") {
            "$ARTIFACT_NAME/$platform/lib/transport_services.lib"
        } else {
            "$ARTIFACT_NAME/$platform/lib/libtransport_services.a"
        }
        
        $variantDir = Join-Path $ARTIFACT_BUNDLE_DIR "$ARTIFACT_NAME/$platform"
        if (Test-Path "$variantDir/lib") {
            $variants += @{
                path = $libPath
                supportedTriples = @($rustTarget)
                staticLibraryMetadata = @{
                    headerPaths = @("$ARTIFACT_NAME/$platform/include")
                    moduleMapPath = "$ARTIFACT_NAME/$platform/include/module.modulemap"
                }
            }
        }
    }
    
    $manifest = @{
        schemaVersion = "1.0"
        artifacts = @{
            $ARTIFACT_NAME = @{
                version = $VERSION
                type = "staticLibrary"
                variants = $variants
            }
        }
    }
    
    $manifest | ConvertTo-Json -Depth 10 | Out-File -FilePath "$ARTIFACT_BUNDLE_DIR/info.json" -Encoding UTF8
}

# Create bundle groups
function Create-BundleGroups {
    Write-Host "Creating bundle groups..."
    
    $bundleGroups = @{
        "apple" = @("ios-arm64", "macos-arm64", "macos-x86_64")
        "android" = @("android-arm64")
        "linux" = @("linux-x86_64", "linux-arm64")
        "windows" = @("windows-x86_64", "windows-arm64")
    }
    
    $bundles = @()
    
    foreach ($groupName in $bundleGroups.Keys) {
        $platforms = $bundleGroups[$groupName]
        $zipName = "transport_services-$groupName.zip"
        $groupBundleDir = Join-Path $BUILD_DIR "transport_services-$groupName.artifactbundle"
        
        # Create group bundle directory
        New-Item -ItemType Directory -Path $groupBundleDir -Force | Out-Null
        
        $groupVariants = @()
        $supportedTriples = @()
        
        foreach ($platform in $platforms) {
            $variantSrcDir = Join-Path $ARTIFACT_BUNDLE_DIR "$ARTIFACT_NAME/$platform"
            if (Test-Path $variantSrcDir) {
                $variantDestDir = Join-Path $groupBundleDir "$ARTIFACT_NAME/$platform"
                New-Item -ItemType Directory -Path (Split-Path $variantDestDir -Parent) -Force | Out-Null
                Copy-Item -Path $variantSrcDir -Destination $variantDestDir -Recurse -Force
                
                $rustTarget = $TARGETS[$platform]
                $supportedTriples += $rustTarget
                
                $libPath = if ($platform -match "windows-") {
                    "$ARTIFACT_NAME/$platform/lib/transport_services.lib"
                } else {
                    "$ARTIFACT_NAME/$platform/lib/libtransport_services.a"
                }
                
                $groupVariants += @{
                    path = $libPath
                    supportedTriples = @($rustTarget)
                    staticLibraryMetadata = @{
                        headerPaths = @("$ARTIFACT_NAME/$platform/include")
                        moduleMapPath = "$ARTIFACT_NAME/$platform/include/module.modulemap"
                    }
                }
            }
        }
        
        if ($groupVariants.Count -gt 0) {
            # Create manifest for this group
            $groupManifest = @{
                schemaVersion = "1.0"
                artifacts = @{
                    $ARTIFACT_NAME = @{
                        version = $VERSION
                        type = "staticLibrary"
                        variants = $groupVariants
                    }
                }
            }
            
            $groupManifest | ConvertTo-Json -Depth 10 | Out-File -FilePath "$groupBundleDir/info.json" -Encoding UTF8
            
            # Create zip file
            Push-Location $BUILD_DIR
            try {
                Compress-Archive -Path (Split-Path $groupBundleDir -Leaf) -DestinationPath $zipName -Force
                
                # Calculate checksum
                $hash = Get-FileHash -Path $zipName -Algorithm SHA256
                $checksum = $hash.Hash.ToLower()
                
                $bundles += @{
                    fileName = $zipName
                    checksum = $checksum
                    supportedTriples = $supportedTriples
                }
            }
            finally {
                Pop-Location
            }
        }
    }
    
    # Create bundle index
    $bundleIndex = @{
        schemaVersion = "1.0"
        bundles = $bundles
    }
    
    $bundleIndex | ConvertTo-Json -Depth 10 | Out-File -FilePath "$BUILD_DIR/transport_services.artifactbundleindex" -Encoding UTF8
}

# Main build process
function Main {
    Write-Host "Building Transport Services artifact bundle..."
    
    Init-Build
    Install-Cbindgen
    Generate-Headers
    
    # Build targets based on parameter
    if ($Target -eq "all") {
        foreach ($platform in $TARGETS.Keys) {
            Build-Target -Platform $platform -RustTarget $TARGETS[$platform]
        }
    } else {
        if ($TARGETS.ContainsKey($Target)) {
            Build-Target -Platform $Target -RustTarget $TARGETS[$Target]
        } else {
            Write-Error "Unknown target: $Target"
            return
        }
    }
    
    Create-Manifest
    Create-BundleGroups
    
    # Create final zip of complete bundle
    Push-Location $BUILD_DIR
    try {
        Compress-Archive -Path "transport_services.artifactbundle" -DestinationPath "transport_services-all.zip" -Force
    }
    finally {
        Pop-Location
    }
    
    Write-Host "Build complete! Artifacts available in $BUILD_DIR"
    Write-Host "- Complete bundle: transport_services-all.zip"
    Write-Host "- Split bundles with index: transport_services.artifactbundleindex"
}

# Run main
Main