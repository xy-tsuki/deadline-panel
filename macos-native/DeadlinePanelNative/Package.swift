// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "DeadlinePanelNative",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .executable(
            name: "DeadlinePanelNative",
            targets: ["DeadlinePanelNative"]
        )
    ],
    targets: [
        .executableTarget(
            name: "DeadlinePanelNative",
            resources: [
                .copy("Resources")
            ],
            linkerSettings: [
                .linkedFramework("AppKit"),
                .linkedFramework("CoreGraphics"),
                .linkedFramework("ServiceManagement"),
                .linkedFramework("SwiftUI"),
                .linkedFramework("UniformTypeIdentifiers"),
                .linkedFramework("UserNotifications"),
                .unsafeFlags([
                    "-L", "../../crates/deadline-core/target/release",
                    "-ldeadline_core",
                    "-Xlinker", "-rpath",
                    "-Xlinker", "../../crates/deadline-core/target/release"
                ])
            ]
        )
    ]
)
