// swift-tools-version:6.2
import PackageDescription

#if canImport(Darwin)
    import Foundation
#endif

// This file exists purely so `.package(url: "https://github.com/SkunkWerkx/HyperUuid", ...)`
// resolves at all — SwiftPM requires Package.swift at the repository root, with no monorepo
// subdirectory support (same hard constraint Packagist has for composer.json). CI's own
// build/test still goes through swift/Package.swift (working-directory: swift); this one
// just points its targets' `path:` at the real sources instead of duplicating them, and has
// to stay in step with it — see that file for what each target is.

// iOS, the iOS simulator and Mac Catalyst take the core from an XCFramework instead: an app
// for those is built by Xcode, which has linked a static library out of an XCFramework since
// Xcode 12 and does not read a static-library artifact bundle. It carries the same no_std
// archives, one slice each (arm64 only), with the header and module map under
// Headers/HyperUuidCore/, so `import HyperUuidCore` finds the same module either way.
//
// Only a Mac can build for those platforms, so only a Mac's manifest declares the target; on
// Linux and Windows this file is what it was. And only when the XCFramework is there: it is
// committed by stage-native-binaries.yml, whole, so a checkout from before its first staging
// has none, and a binary target whose path is missing fails the whole package, macOS
// included.
#if canImport(Darwin)
    let appleCore = "swift/HyperUuidCoreApple.xcframework"
    let linksAppleCore = FileManager.default.fileExists(
        atPath: "\(Context.packageDirectory)/\(appleCore)/Info.plist")
#else
    let appleCore = ""
    let linksAppleCore = false
#endif
let appleCoreTargets: [Target] =
    linksAppleCore ? [.binaryTarget(name: "HyperUuidCoreApple", path: appleCore)] : []
let coreDependencies: [Target.Dependency] =
    linksAppleCore
    ? [
        .target(name: "HyperUuidCore", condition: .when(platforms: [.macOS, .linux, .windows, .wasi])),
        .target(name: "HyperUuidCoreApple", condition: .when(platforms: [.iOS, .macCatalyst])),
    ] : ["HyperUuidCore"]

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
            dependencies: coreDependencies,
            path: "swift/Sources/HyperUuid"
        ),
        .testTarget(
            name: "HyperUuidTests",
            dependencies: ["HyperUuid"],
            path: "swift/Tests/HyperUuidTests"
        ),
    ] + appleCoreTargets
)
