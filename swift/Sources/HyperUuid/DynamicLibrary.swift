import Foundation

// Exactly the three C libraries `NativePlatform` has a native build for. `canImport` rather
// than `os(Linux)` so a musl build reaches `NativePlatform`'s own `#error` and nothing else.
#if canImport(Glibc)
import Glibc
#elseif os(macOS)
import Darwin
#elseif os(Windows)
import WinSDK
#endif

/// Thin cross-platform wrapper around `dlopen`/`dlsym` (Linux/macOS, via `Glibc`/`Darwin`) or
/// `LoadLibraryW`/`GetProcAddress` (Windows, via `WinSDK`) — this project's positioning is
/// "direct native FFI, no runtime bridge," and Swift natively supports calling a raw C
/// function pointer via an `@convention(c)` typealias cast, so no shim/trampoline is needed
/// here the way the Go binding needs purego's. Failures are ``NativeLibraryError``, the one
/// public type in this file's orbit.
final class DynamicLibrary {
    #if os(Windows)
    private let handle: HMODULE
    #else
    private let handle: UnsafeMutableRawPointer
    #endif

    init(path: String) throws {
        #if os(Windows)
        guard let h = path.withCString(encodedAs: UTF16.self, { LoadLibraryW($0) }) else {
            throw NativeLibraryError.openFailed(path: path, reason: "GetLastError=\(GetLastError())")
        }
        handle = h
        #else
        // RTLD_LOCAL: every export is resolved through `symbol(_:)` against this handle, so
        // nothing needs the library's names in the process-wide namespace — and keeping them
        // out means a second copy loaded by something else in the process can't be bound to
        // this one's symbols, or the reverse.
        guard let h = dlopen(path, RTLD_NOW | RTLD_LOCAL) else {
            let reason = dlerror().map { String(cString: $0) } ?? "unknown error"
            throw NativeLibraryError.openFailed(path: path, reason: reason)
        }
        handle = h
        #endif
    }

    func symbol(_ name: String) throws -> UnsafeMutableRawPointer {
        #if os(Windows)
        guard let sym = GetProcAddress(handle, name) else {
            throw NativeLibraryError.symbolNotFound(name: name)
        }
        return unsafeBitCast(sym, to: UnsafeMutableRawPointer.self)
        #else
        guard let sym = dlsym(handle, name) else {
            throw NativeLibraryError.symbolNotFound(name: name)
        }
        return sym
        #endif
    }

    // Deliberately no `deinit` that closes the handle: `UuidGenerator`'s loaded-library
    // reference holds this for the process's lifetime (same as the Go/Java bindings, which
    // never unload either), and its cached `@convention(c)` function pointers would dangle
    // if the library were unloaded while still in use.
}

extension DynamicLibrary {
    /// Where `locateBundled()` found the library. The test suite pins `.resourceBundle`, so
    /// the path a deployed consumer depends on can't quietly stop working behind the
    /// build-machine fallback.
    enum Origin {
        /// Inside the SwiftPM resource directory — the only origin a deployed binary has.
        case resourceBundle
        /// Straight from this package's source tree, which exists only on the machine that
        /// built it.
        case sourceTree
    }

    /// Finds this platform's bundled native library on disk and returns a path `dlopen` can
    /// take directly — SwiftPM copies resources as plain files, so there is nothing to
    /// extract, and nothing left behind in the temp directory per process the way the old
    /// copy-then-load did.
    ///
    /// Deliberately not `Bundle.module`: the accessor SwiftPM generates `fatalError`s when
    /// the resource directory is missing, which turns "deployed the executable without its
    /// resources" into a crash no caller can catch. Looking for the directory by name makes
    /// that the same thrown ``NativeLibraryError`` as any other load failure.
    static func locateBundled() throws -> (path: String, origin: Origin) {
        let fileName = URL(fileURLWithPath: NativePlatform.libraryFileName)
        let subdirectory = "NativeLibs/\(NativePlatform.rid)"

        // Beside the executable is where `swift build` puts the directory and where a
        // deployment has to keep it; an app bundle carries it in its resources instead.
        var directories = [Bundle.main.bundleURL]
        if let resources = Bundle.main.resourceURL {
            directories.append(resources)
        }
        #if os(macOS)
        // On macOS this code can be linked into a bundle that isn't the main one — an
        // `.xctest` under `swift test` (whose main bundle is the `xctest` tool itself), or
        // a framework — so the bundle that owns this class, and the directory that bundle
        // sits in, are searched too.
        let owner = Bundle(for: DynamicLibrary.self)
        if let resources = owner.resourceURL {
            directories.append(resources)
        }
        directories.append(owner.bundleURL.deletingLastPathComponent())
        #endif

        var searched: [String] = []
        for directory in directories {
            for bundleName in NativePlatform.resourceBundleNames {
                let bundleURL = directory.appendingPathComponent(bundleName)
                if searched.contains(bundleURL.path) { continue }
                searched.append(bundleURL.path)
                if let url = Bundle(url: bundleURL)?.url(
                    forResource: fileName.deletingPathExtension().lastPathComponent,
                    withExtension: fileName.pathExtension,
                    subdirectory: subdirectory
                ) {
                    return (fileSystemPath(url), .resourceBundle)
                }
            }
        }

        // The build machine's fallback, standing in for the absolute build path the
        // generated accessor falls back to: an executable copied out of `.build` without its
        // resource directory still runs where the package's sources are.
        let inSourceTree = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .appendingPathComponent(subdirectory)
            .appendingPathComponent(NativePlatform.libraryFileName)
        if FileManager.default.fileExists(atPath: inSourceTree.path) {
            return (fileSystemPath(inSourceTree), .sourceTree)
        }

        let bundleNames = NativePlatform.resourceBundleNames.joined(separator: " or ")
        throw NativeLibraryError.openFailed(
            path: "\(NativePlatform.resourceBundleNames[0])/\(subdirectory)/\(NativePlatform.libraryFileName)",
            reason: "not found — \(bundleNames) has to ship beside the executable, with "
                + "this platform's library inside it (looked in \(searched.joined(separator: ", ")))"
        )
    }

    /// The path in the form the platform's loader takes — on Windows that is a drive-letter
    /// path with backslashes, which `URL.path` doesn't promise.
    private static func fileSystemPath(_ url: URL) -> String {
        url.withUnsafeFileSystemRepresentation { $0.map { String(cString: $0) } } ?? url.path
    }
}
