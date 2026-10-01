/// Where this build's native core came from. Internal: the test suite pins it, so the path a
/// deployed consumer depends on can't quietly stop working behind a build-machine fallback.
enum NativeLibraryOrigin {
    /// Linked into the executable — Linux and WebAssembly. There is no file to find.
    case staticallyLinked
    /// Loaded from inside the SwiftPM resource directory — the only origin a deployed macOS
    /// or Windows binary has.
    case resourceBundle
    /// Loaded straight from this package's source tree, which exists only on the machine
    /// that built it.
    case sourceTree
}
