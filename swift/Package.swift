// swift-tools-version:6.2
import PackageDescription

#if canImport(Darwin)
    import Foundation
#endif

// The development loop: HYPERUUID_LOCAL_CORE=1 links the bundle .github/scripts/local-core.sh
// builds from the checkout, under rust/target/ (which git ignores), in place of the committed
// one, so the suite can run against the core as it stands without replacing a committed
// archive. The repository root's Package.swift, which consumers resolve, has no such switch.
let coreBundle =
    Context.environment["HYPERUUID_LOCAL_CORE"] == nil
    ? "HyperUuidCore.artifactbundle" : "../rust/target/local-core/swift/HyperUuidCore.artifactbundle"

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
    let appleCore = "HyperUuidCoreApple.xcframework"
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
        .target(name: "HyperUuidCore", condition: .when(platforms: [.macOS, .linux, .windows, .wasi, .android])),
        .target(name: "HyperUuidCoreApple", condition: .when(platforms: [.iOS, .macCatalyst])),
    ] : ["HyperUuidCore"]

let package = Package(
    name: "HyperUuid",
    products: [
        .library(name: "HyperUuid", targets: ["HyperUuid"])
    ],
    targets: [
        // The native core as static libraries, one per triple (SE-0482, which is what sets
        // the tools version above): glibc and musl Linux and macOS on x86_64 and arm64,
        // Windows (MSVC) on x86_64 and arm64, and WASI. SwiftPM picks the variant for the
        // triple being built and links it into the consumer's executable, so nothing ships
        // beside it and nothing is opened at run time. A triple with no variant has no
        // `HyperUuidCore` module, and the build stops there rather than at run time.
        .binaryTarget(
            name: "HyperUuidCore",
            path: coreBundle
        ),
        .target(
            name: "HyperUuid",
            dependencies: coreDependencies
        ),
        .testTarget(
            name: "HyperUuidTests",
            dependencies: ["HyperUuid"]
        ),
    ] + appleCoreTargets
)
