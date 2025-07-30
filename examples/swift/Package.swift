// swift-tools-version:6.0
import PackageDescription

let package = Package(
    name: "PathMonitorExample",
    platforms: [
        .macOS(.v15),
        .iOS(.v18),
        .tvOS(.v18),
        .watchOS(.v11),
        .visionOS(.v2)
    ],
    products: [
        .executable(
            name: "PathMonitorExample",
            targets: ["PathMonitorExample"]
        ),
    ],
    dependencies: [
        // Reference the local TransportServices package
        .package(path: "../..")
    ],
    targets: [
        .executableTarget(
            name: "PathMonitorExample",
            dependencies: [
                "TransportServices"
            ],
            path: ".",
            sources: ["PathMonitorExample.swift"]
        ),
    ]
)