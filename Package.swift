// swift-tools-version:6.1
import PackageDescription
#if canImport(FoundationEssentials)
import FoundationEssentials // Needed for binary target support
#else
import Foundation // Fallback for older Swift versions
#endif

// --- Logic to switch between local and remote artifact ---

// Check for an environment variable to decide which target to use.
// To use the local version, run: `USE_LOCAL_ARTIFACT=1 swift build`
let useLocalArtifact = ProcessInfo.processInfo.environment["USE_LOCAL_ARTIFACT"] != nil

// Define the binary target based on the condition
let transportServicesFFITarget: Target

if useLocalArtifact {
    // --- LOCAL DEVELOPMENT ---
    // Points to the .artifactbundle directory relative to the Package.swift file.
    print("Using local transport_services artifact.")
    transportServicesFFITarget = .binaryTarget(
        name: "TransportServicesFFI",
        path: "./build/transport_services.artifactbundle"
    )
} else {
    // --- PRODUCTION / CI ---
    // Points to the remote zip file on a release server like GitHub.
    print("Using remote transport_services artifact.")
    transportServicesFFITarget = .binaryTarget(
        name: "TransportServicesFFI",
        url: "https://github.com/edgeengineer/tapsrs/releases/download/v0.1.0/transport_services.artifactbundle.zip",
        checksum: "YOUR_CHECKSUM_HERE"
    )
}


// --- Package Definition ---

let package = Package(
    name: "TransportServices",
    platforms: [
        .macOS(.v15),
        .iOS(.v18),
        .tvOS(.v18),
        .watchOS(.v11),
        .visionOS(.v2)
    ],
    products: [
        .library(
            name: "TransportServices",
            targets: ["TransportServices"]),
    ],
    dependencies: [],
    targets: [
        // Swift wrapper library around the FFI bindings
        .target(
            name: "TransportServices",
            dependencies: ["TransportServicesFFI"],
            path: "bindings/swift/Sources/TransportServices"
        ),
        
        // Test target using Swift Testing (built into Swift 6)
        .testTarget(
            name: "TransportServicesTests",
            dependencies: ["TransportServices"],
            path: "bindings/swift/Tests/TransportServicesTests"
        ),
        
        // Add the conditionally defined FFI binary target
        transportServicesFFITarget
    ]
)