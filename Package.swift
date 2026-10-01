// swift-tools-version:6.2
import PackageDescription

// This file exists purely so `.package(url: "https://github.com/SkunkWerkx/HyperUuid", ...)`
// resolves at all — SwiftPM requires Package.swift at the repository root, with no monorepo
// subdirectory support (same hard constraint Packagist has for composer.json). CI's own
// build/test still goes through swift/Package.swift (working-directory: swift); this one
// just points its targets' `path:` at the real sources instead of duplicating them, and has
// to stay in step with it — see that file for what each target is.
let package = Package(
    name: "HyperUuid",
    products: [
        .library(name: "HyperUuid", targets: ["HyperUuid"])
    ],
    targets: [
        .binaryTarget(
            name: "HyperUuidCore",
            path: "swift/HyperUuidCore.artifactbundle"
        ),
        .target(
            name: "HyperUuid",
            dependencies: [
                .target(name: "HyperUuidCore", condition: .when(platforms: [.linux, .wasi]))
            ],
            path: "swift/Sources/HyperUuid",
            resources: [.copy("NativeLibs")]
        ),
        .testTarget(
            name: "HyperUuidTests",
            dependencies: ["HyperUuid"],
            path: "swift/Tests/HyperUuidTests"
        ),
    ]
)
