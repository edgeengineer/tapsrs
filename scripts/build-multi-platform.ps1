# PowerShell script to build Transport Services for multiple platforms
# Builds Windows targets locally and Linux/Android targets in Docker

param(
    [string]$BuildDir = "build",
    [switch]$SkipDocker,
    [switch]$AppleTargets
)

$ErrorActionPreference = "Stop"

# Configuration
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$ArtifactBundleDir = Join-Path $BuildDir "transport_services.artifactbundle"
$ArtifactName = "transport_services"
$Version = "0.1.0"

# Target configurations
$WindowsTargets = @{
    "windows-x86_64" = "x86_64-pc-windows-msvc"
    "windows-arm64" = "aarch64-pc-windows-msvc"
}

$LinuxTargets = @{
    "linux-x86_64" = "x86_64-unknown-linux-gnu"
    "linux-arm64" = "aarch64-unknown-linux-gnu"
    "android-arm64" = "aarch64-linux-android"
}

$AppleTargetsMap = @{
    "ios-arm64" = "aarch64-apple-ios"
    "tvos-arm64" = "aarch64-apple-tvos" 
    "macos-arm64" = "aarch64-apple-darwin"
    "macos-x86_64" = "x86_64-apple-darwin"
    "watchos-arm64" = "aarch64-apple-watchos"
}

function Initialize-Build {
    Write-Host "Initializing build environment..." -ForegroundColor Green
    
    if (Test-Path $BuildDir) {
        Remove-Item -Path $BuildDir -Recurse -Force
    }
    
    New-Item -ItemType Directory -Path $BuildDir -Force | Out-Null
    New-Item -ItemType Directory -Path $ArtifactBundleDir -Force | Out-Null
}

function Install-Dependencies {
    Write-Host "Checking dependencies..." -ForegroundColor Green
    
    # Check if Rust is installed
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Error "Rust is not installed. Please install from https://rustup.rs/"
        exit 1
    }
    
    # Install cbindgen if not present
    if (-not (Get-Command cbindgen -ErrorAction SilentlyContinue)) {
        Write-Host "Installing cbindgen..."
        cargo install cbindgen
    }
    
    # Add Windows targets
    Write-Host "Adding Rust targets..."
    foreach ($target in $WindowsTargets.Values) {
        rustup target add $target
    }
}

function Generate-Headers {
    Write-Host "Generating C headers..." -ForegroundColor Green
    
    Push-Location $ProjectRoot
    try {
        # Generate header file
        cbindgen --config cbindgen.toml --crate transport_services --output "$BuildDir\transport_services.h"
        
        # Generate module map
        @"
module TransportServices {
    header "transport_services.h"
    export *
}
"@ | Out-File -FilePath "$BuildDir\module.modulemap" -Encoding UTF8
    }
    finally {
        Pop-Location
    }
}

function Build-WindowsTarget {
    param(
        [string]$Platform,
        [string]$RustTarget
    )
    
    Write-Host "Building for $Platform ($RustTarget)..." -ForegroundColor Yellow
    
    $variantDir = Join-Path $ArtifactBundleDir "$ArtifactName\$Platform"
    New-Item -ItemType Directory -Path "$variantDir\lib" -Force | Out-Null
    New-Item -ItemType Directory -Path "$variantDir\include" -Force | Out-Null
    
    Push-Location $ProjectRoot
    try {
        # Build the static library
        cargo build --release --target $RustTarget --features ffi --no-default-features
        
        # Copy the built library
        $sourcePath = "target\$RustTarget\release\transport_services.lib"
        if (-not (Test-Path $sourcePath)) {
            $sourcePath = "target\$RustTarget\release\libtransport_services.a"
        }
        
        Copy-Item -Path $sourcePath -Destination "$variantDir\lib\transport_services.lib"
        
        # Copy headers
        Copy-Item -Path "$BuildDir\transport_services.h" -Destination "$variantDir\include\"
        Copy-Item -Path "$BuildDir\module.modulemap" -Destination "$variantDir\include\"
    }
    finally {
        Pop-Location
    }
}

function Build-DockerTargets {
    Write-Host "Building Linux/Android targets in Docker..." -ForegroundColor Green
    
    Push-Location $ProjectRoot
    try {
        # Build Docker image
        docker build -f Dockerfile.build -t transport-services-builder .
        
        # Run build in Docker
        docker run --rm `
            -v "${ProjectRoot}:/workspace" `
            -e BUILD_TARGETS="linux-x86_64,linux-arm64,android-arm64" `
            transport-services-builder
    }
    finally {
        Pop-Location
    }
}

function Create-Manifest {
    param(
        [hashtable]$Targets
    )
    
    Write-Host "Creating artifact bundle manifest..." -ForegroundColor Green
    
    $variants = @()
    
    foreach ($platform in $Targets.Keys) {
        $rustTarget = $Targets[$platform]
        $libPath = if ($platform -like "windows-*") {
            "$ArtifactName/$platform/lib/transport_services.lib"
        } else {
            "$ArtifactName/$platform/lib/libtransport_services.a"
        }
        
        $variant = @{
            path = $libPath
            supportedTriples = @($rustTarget)
            staticLibraryMetadata = @{
                headerPaths = @("$ArtifactName/$platform/include")
                moduleMapPath = "$ArtifactName/$platform/include/module.modulemap"
            }
        }
        
        $variants += $variant
    }
    
    $manifest = @{
        schemaVersion = "1.0"
        artifacts = @{
            $ArtifactName = @{
                version = $Version
                type = "staticLibrary"
                variants = $variants
            }
        }
    }
    
    $manifest | ConvertTo-Json -Depth 10 | Out-File -FilePath "$ArtifactBundleDir\info.json" -Encoding UTF8
}

function Create-Bundle {
    Write-Host "Creating artifact bundle..." -ForegroundColor Green
    
    Push-Location $BuildDir
    try {
        # Create zip file
        Compress-Archive -Path "transport_services.artifactbundle" -DestinationPath "transport_services-all.zip"
        
        Write-Host "Build complete! Artifact bundle created at: $BuildDir\transport_services-all.zip" -ForegroundColor Green
    }
    finally {
        Pop-Location
    }
}

# Main execution
Initialize-Build
Install-Dependencies
Generate-Headers

# Build Windows targets locally
foreach ($platform in $WindowsTargets.Keys) {
    Build-WindowsTarget -Platform $platform -RustTarget $WindowsTargets[$platform]
}

# Build Linux/Android targets in Docker (if not skipped)
if (-not $SkipDocker) {
    Build-DockerTargets
}

# Include Apple targets if requested (requires macOS)
$allTargets = $WindowsTargets.Clone()
if (-not $SkipDocker) {
    foreach ($key in $LinuxTargets.Keys) {
        $allTargets[$key] = $LinuxTargets[$key]
    }
}
if ($AppleTargets) {
    foreach ($key in $AppleTargetsMap.Keys) {
        $allTargets[$key] = $AppleTargetsMap[$key]
    }
}

Create-Manifest -Targets $allTargets
Create-Bundle

Write-Host "`nArtifact bundle created successfully!" -ForegroundColor Green
Write-Host "Location: $BuildDir\transport_services-all.zip" -ForegroundColor Cyan