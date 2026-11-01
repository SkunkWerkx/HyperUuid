// swift-tools-version:6.2
import PackageDescription

import Foundation

// This file exists purely so `.package(url: "https://github.com/SkunkWerkx/HyperUuid", ...)`
// resolves at all — SwiftPM requires Package.swift at the repository root, with no monorepo
// subdirectory support (same hard constraint Packagist has for composer.json). CI's own
// build/test still goes through swift/Package.swift (working-directory: swift); this one
// just points its targets' `path:` at the real sources instead of duplicating them, and has
// to stay in step with it — see that file for what each target is.

// The core comes from the GitHub Release: two zips per version, the static-library artifact
// bundle (Linux glibc and musl, macOS, Windows, WebAssembly and Android, one archive per
// triple) and, on a Mac, the XCFramework iOS, the iOS simulator and Mac Catalyst link (Xcode
// does not read a static-library artifact bundle). SwiftPM checks each zip against its
// checksum before using it. stage-native-binaries.yml writes the three lines below for every
// release, from the zips the release then attaches; none resolves before the first release
// that carries them (1.1.0).
let coreRelease = "1.1.0"
let coreChecksum = "470dde0c285d1be436ee6c9931415c73df3ae304f3462e81da9aed0a1e536e43"
let appleCoreChecksum = "43dd7f956f2c86f03e3352071a2c9a669843ad29d524579b213512114e81703b"

// In this repository, CI and .github/scripts/local-core.sh write the core into
// swift/HyperUuidCore.artifactbundle/ (gitignored), and then this file links that instead, the
// way swift/Package.swift does: every CI check that builds through this manifest tests the core
// the run built. A consumer's checkout never has one.
func built(_ path: String) -> Bool {
    FileManager.default.fileExists(atPath: "\(Context.packageDirectory)/\(path)")
}
let assets = "https://github.com/SkunkWerkx/HyperUuid/releases/download/v\(coreRelease)"
let localCore = "swift/HyperUuidCore.artifactbundle"
let localAppleCore = "swift/HyperUuidCoreApple.xcframework"
let isLocal = built("\(localCore)/info.json")
#if canImport(Darwin)
    let linksAppleCore = isLocal ? built("\(localAppleCore)/Info.plist") : true
#else
    let linksAppleCore = false
#endif
let coreTargets: [Target] =
    [
        isLocal
            ? .binaryTarget(name: "HyperUuidCore", path: localCore)
            : .binaryTarget(
                name: "HyperUuidCore", url: "\(assets)/HyperUuidCore.artifactbundle.zip",
                checksum: coreChecksum)
    ]
    + (!linksAppleCore
        ? []
        : [
            isLocal
                ? .binaryTarget(name: "HyperUuidCoreApple", path: localAppleCore)
                : .binaryTarget(
                    name: "HyperUuidCoreApple", url: "\(assets)/HyperUuidCoreApple.xcframework.zip",
                    checksum: appleCoreChecksum)
        ])
let coreDependencies: [Target.Dependency] =
    linksAppleCore
    ? [
        .target(name: "HyperUuidCore", condition: .when(platforms: [.macOS, .linux, .windows, .wasi, .android])),
        .target(name: "HyperUuidCoreApple", condition: .when(platforms: [.iOS, .macCatalyst])),
    ] : ["HyperUuidCore"]

let package = Package(
    name: "HyperUuid",
    products: [
        .library(name: "HyperUuid", targets: ["HyperUuid"])
    ],
    targets: [
        .target(
            name: "HyperUuid",
            dependencies: coreDependencies,
            path: "swift/Sources/HyperUuid"
        ),
        .testTarget(
            name: "HyperUuidTests",
            dependencies: ["HyperUuid"],
            path: "swift/Tests/HyperUuidTests"
        ),
    ] + coreTargets
)
