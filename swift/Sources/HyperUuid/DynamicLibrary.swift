#if os(Linux) || os(Android)
import Glibc
#elseif os(macOS) || os(iOS) || os(watchOS) || os(tvOS)
import Darwin
#elseif os(Windows)
import WinSDK
#endif

/// Thin cross-platform wrapper around `dlopen`/`dlsym` (Linux/macOS, via `Glibc`/`Darwin`) or
/// `LoadLibraryW`/`GetProcAddress` (Windows, via `WinSDK`) — this project's positioning is
/// "direct native FFI, no runtime bridge," and Swift natively supports calling a raw C
/// function pointer via an `@convention(c)` typealias cast, so no shim/trampoline is needed
/// here the way the Go binding needs purego's.
enum DynamicLibraryError: Error, CustomStringConvertible {
    case openFailed(path: String, reason: String)
    case symbolNotFound(name: String)

    var description: String {
        switch self {
        case .openFailed(let path, let reason):
            return "hyperuuid: failed to load native library at \(path): \(reason)"
        case .symbolNotFound(let name):
            return "hyperuuid: symbol \(name) not found in native library"
        }
    }
}

final class DynamicLibrary {
    #if os(Windows)
    private let handle: HMODULE
    #else
    private let handle: UnsafeMutableRawPointer
    #endif

    init(path: String) throws {
        #if os(Windows)
        guard let h = path.withCString(encodedAs: UTF16.self, { LoadLibraryW($0) }) else {
            throw DynamicLibraryError.openFailed(path: path, reason: "GetLastError=\(GetLastError())")
        }
        handle = h
        #else
        guard let h = dlopen(path, RTLD_NOW | RTLD_GLOBAL) else {
            let reason = dlerror().map { String(cString: $0) } ?? "unknown error"
            throw DynamicLibraryError.openFailed(path: path, reason: reason)
        }
        handle = h
        #endif
    }

    func symbol(_ name: String) throws -> UnsafeMutableRawPointer {
        #if os(Windows)
        guard let sym = GetProcAddress(handle, name) else {
            throw DynamicLibraryError.symbolNotFound(name: name)
        }
        return unsafeBitCast(sym, to: UnsafeMutableRawPointer.self)
        #else
        guard let sym = dlsym(handle, name) else {
            throw DynamicLibraryError.symbolNotFound(name: name)
        }
        return sym
        #endif
    }

    // Deliberately no `deinit` that closes the handle: `UuidGenerator.library` holds this for
    // the process's lifetime (same as the Go/Java bindings, which never unload either), and
    // its cached `@convention(c)` function pointers would dangle if the library were unloaded
    // while still in use.
}
