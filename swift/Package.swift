// swift-tools-version:6.2
import PackageDescription

import Foundation

// The native core is a static-library artifact bundle, one archive per triple (SE-0482, which
// is what sets the tools version above): SwiftPM picks the variant for the triple being built
// and links it into the consumer's executable, so nothing ships beside it and nothing is
// opened at run time. A triple with no variant has no `HyperUuidCore` module, and the build
// stops there rather than at run time.
//
// The core is build output, never committed: CI writes each leg's archives (and info.json)
// into HyperUuidCore.artifactbundle/ before any Swift step, and .github/scripts/local-core.sh
// writes the host's. The bundle is declared only once its info.json is there, so the package
// still resolves without one (Dependabot resolves swift/Benchmarks through this file); a build
// then stops at `import HyperUuidCore`. Consumers never see this file: they resolve the root
// Package.swift, which takes the core from the release's assets.
//
// iOS, the iOS simulator and Mac Catalyst take the core from an XCFramework instead: an app
// for those is built by Xcode, which has linked a static library out of an XCFramework since
// Xcode 12 and does not read a static-library artifact bundle. It carries the same no_std
// archives, one slice each (arm64 only), with the header and module map under
// Headers/HyperUuidCore/, so `import HyperUuidCore` finds the same module either way. Only a
// Mac builds one (the full static-libs build), so it is declared only when it is there.
func built(_ path: String) -> Bool {
    FileManager.default.fileExists(atPath: "\(Context.packageDirectory)/\(path)")
}
let core = "HyperUuidCore.artifactbundle"
let appleCore = "HyperUuidCoreApple.xcframework"
let linksCore = built("\(core)/info.json")
let linksAppleCore = built("\(appleCore)/Info.plist")
let coreTargets: [Target] =
    (linksCore ? [.binaryTarget(name: "HyperUuidCore", path: core)] : [])
    + (linksAppleCore ? [.binaryTarget(name: "HyperUuidCoreApple", path: appleCore)] : [])
let coreDependencies: [Target.Dependency] =
    (linksCore
        ? [.target(name: "HyperUuidCore", condition: .when(platforms: [.macOS, .linux, .windows, .wasi, .android]))]
        : [])
    + (linksAppleCore
        ? [.target(name: "HyperUuidCoreApple", condition: .when(platforms: [.iOS, .macCatalyst]))] : [])

let package = Package(
    name: "HyperUuid",
    products: [
        .library(name: "HyperUuid", targets: ["HyperUuid"])
    ],
    targets: [
        .target(
            name: "HyperUuid",
            dependencies: coreDependencies
        ),
        .testTarget(
            name: "HyperUuidTests",
            dependencies: ["HyperUuid"]
        ),
    ] + coreTargets
)
