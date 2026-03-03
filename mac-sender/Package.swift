// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "VirtualDisplayStreamer",
    platforms: [
        .macOS(.v13)
    ],
    targets: [
        .target(
            name: "CGVirtualDisplayPrivate",
            path: "Sources/CGVirtualDisplayPrivate",
            publicHeadersPath: "include",
            linkerSettings: [
                .linkedFramework("CoreGraphics"),
            ]
        ),
        .executableTarget(
            name: "VirtualDisplayStreamer",
            dependencies: ["CGVirtualDisplayPrivate"],
            path: "Sources/VirtualDisplayStreamer",
            linkerSettings: [
                .linkedFramework("CoreGraphics"),
                .linkedFramework("IOSurface"),
                .linkedFramework("ScreenCaptureKit"),
                .linkedFramework("VideoToolbox"),
                .linkedFramework("CoreMedia"),
                .linkedFramework("CoreVideo"),
                .linkedFramework("AppKit"),
                .linkedFramework("SwiftUI"),
                .linkedFramework("Network"),
            ]
        )
    ]
)
