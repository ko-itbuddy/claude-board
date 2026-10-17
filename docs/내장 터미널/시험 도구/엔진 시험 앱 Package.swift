// swift-tools-version: 6.2
import PackageDescription

let package = Package(
    name: "Spike",
    platforms: [.macOS(.v14)],
    dependencies: [
        .package(url: "https://github.com/Lakr233/libghostty-spm.git", exact: "2.2.2026101502"),
    ],
    targets: [
        .executableTarget(
            name: "Spike",
            dependencies: [
                .product(name: "GhosttyTerminal", package: "libghostty-spm"),
                .product(name: "GhosttyKit", package: "libghostty-spm"),
                .product(name: "GhosttyTheme", package: "libghostty-spm"),
            ],
            path: "Sources/Spike",
            swiftSettings: [.swiftLanguageMode(.v5)]
        ),
    ]
)
