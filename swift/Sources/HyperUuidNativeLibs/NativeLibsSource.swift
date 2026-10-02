// The macOS and Windows shared libraries, as a target of their own: `NativeLibs/{rid}/{lib}`
// beside this file is the target's one resource. Package.swift makes `HyperUuid` depend on it
// on macOS and Windows only, which is what keeps the libraries out of a Linux or WebAssembly
// build — SwiftPM resources take no platform condition, but a target dependency does, and a
// target that isn't built has no resources to stage. Linux and WebAssembly link the core in
// from `HyperUuidCore` instead.
//
// SwiftPM needs a source file here to treat the directory as a Swift target at all (with none,
// it takes it for a C target and asks for an `include` directory), and this is the one thing
// the loader needs from the target at compile time. The libraries themselves are found at run
// time, in the resource directory SwiftPM stages (`DynamicLibrary.locateBundled()`).

/// Where this target sits in the package's source tree.
package enum NativeLibsSource {
    /// This file's path at build time. `NativeLibs/` is beside it, which is the loader's
    /// build-machine fallback when the resource directory isn't found.
    package static let filePath = #filePath
}
