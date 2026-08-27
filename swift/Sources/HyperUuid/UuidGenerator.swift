import Foundation

/// RFC 9562 UUID generation (v4 random, v5 deterministic, v7 time-sortable) calling directly
/// into the native `libhyperuuid` shared library via `dlopen`/`dlsym` (or the Windows
/// equivalent) plus an `@convention(c)` function-pointer cast — no runtime bridge, no cgo-
/// style shim (see `DynamicLibrary.swift`).
///
/// This package bundles a native build for every platform (see `NativePlatform`) and picks
/// the right one at compile time.
public enum UuidGenerator {
    public enum Error: Swift.Error, CustomStringConvertible {
        case randomSourceFailure(code: Int32)
        case timestampOutOfRange

        public var description: String {
            switch self {
            case .randomSourceFailure(let code):
                return "hyperuuid: native call failed with code \(code) (random source failure)"
            case .timestampOutOfRange:
                return "hyperuuid: unix millisecond timestamp must be non-negative and fit within 48 bits"
            }
        }
    }

    private typealias UuidNewV4Fn = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Int32
    private typealias UuidNewV5Fn = @convention(c) (
        UnsafePointer<UInt8>?, UnsafePointer<UInt8>?, UInt32, UnsafeMutablePointer<UInt8>?
    ) -> Int32
    private typealias UuidNewV7Fn = @convention(c) (UInt64, UnsafeMutablePointer<UInt8>?) -> Int32
    private typealias UuidV7UnixMillisFn = @convention(c) (UnsafePointer<UInt8>?) -> UInt64

    private struct LoadedLibrary {
        let library: DynamicLibrary
        let newV4: UuidNewV4Fn
        let newV5: UuidNewV5Fn
        let newV7: UuidNewV7Fn
        let v7UnixMillis: UuidV7UnixMillisFn
    }

    // Swift initializes `static let`s lazily and exactly once, thread-safely — the same
    // "loaded on first use" behavior the Kotlin binding gets from `by lazy` / `object`. Unlike
    // Kotlin, that initializer can't itself `throw`, so failures are captured in a `Result`
    // and re-surfaced as a normal `throws` from `loaded()` rather than crashing the process.
    private static let loadResult: Result<LoadedLibrary, Swift.Error> = Result { try load() }

    private static func load() throws -> LoadedLibrary {
        let tempPath = try extractNativeLibrary()
        let library = try DynamicLibrary(path: tempPath)
        let newV4 = unsafeBitCast(try library.symbol("uuid_new_v4"), to: UuidNewV4Fn.self)
        let newV5 = unsafeBitCast(try library.symbol("uuid_new_v5"), to: UuidNewV5Fn.self)
        let newV7 = unsafeBitCast(try library.symbol("uuid_new_v7"), to: UuidNewV7Fn.self)
        let v7UnixMillis = unsafeBitCast(
            try library.symbol("uuid_v7_unix_millis"), to: UuidV7UnixMillisFn.self)
        return LoadedLibrary(library: library, newV4: newV4, newV5: newV5, newV7: newV7, v7UnixMillis: v7UnixMillis)
    }

    private static func loaded() throws -> LoadedLibrary {
        switch loadResult {
        case .success(let l): return l
        case .failure(let e): throw e
        }
    }

    /// Extracts this platform's bundled native library (an SPM resource, which may not
    /// already be a plain filesystem path depending on how the package was packaged) to a
    /// temp file, mirroring the Kotlin/Go bindings' approach. The temp file is deliberately
    /// never removed — same best-effort tradeoff as Kotlin's `deleteOnExit`/Go's approach.
    private static func extractNativeLibrary() throws -> String {
        let libNameURL = URL(fileURLWithPath: NativePlatform.libraryFileName)
        guard
            let resourceURL = Bundle.module.url(
                forResource: libNameURL.deletingPathExtension().lastPathComponent,
                withExtension: libNameURL.pathExtension,
                subdirectory: "NativeLibs/\(NativePlatform.rid)"
            )
        else {
            throw DynamicLibraryError.openFailed(
                path: "NativeLibs/\(NativePlatform.rid)/\(NativePlatform.libraryFileName)",
                reason: "resource not found (unsupported platform, or this package was built without a native library for it)"
            )
        }

        let data = try Data(contentsOf: resourceURL)
        let tempURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("libhyperuuid-\(UUID().uuidString)")
            .appendingPathExtension(libNameURL.pathExtension)
        try data.write(to: tempURL)
        return tempURL.path
    }

    /// Creates a random UUID version 4 (RFC 9562 §5.4).
    public static func newV4() throws -> UUID {
        let l = try loaded()
        var out = [UInt8](repeating: 0, count: 16)
        let rc: Int32 = out.withUnsafeMutableBufferPointer { outBuf in
            l.newV4(outBuf.baseAddress)
        }
        guard rc == 0 else { throw Error.randomSourceFailure(code: rc) }
        return UUID(rfcBytes: out)
    }

    /// Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and raw name
    /// bytes. The same (namespace, name) pair always produces the same UUID.
    public static func newV5(namespace: UUID, name: [UInt8]) throws -> UUID {
        let l = try loaded()
        let ns = namespace.rfcBytes
        var out = [UInt8](repeating: 0, count: 16)
        let rc: Int32 = ns.withUnsafeBufferPointer { nsBuf in
            name.withUnsafeBufferPointer { nameBuf in
                out.withUnsafeMutableBufferPointer { outBuf in
                    l.newV5(nsBuf.baseAddress, name.isEmpty ? nil : nameBuf.baseAddress, UInt32(name.count), outBuf.baseAddress)
                }
            }
        }
        guard rc == 0 else { throw Error.randomSourceFailure(code: rc) }
        return UUID(rfcBytes: out)
    }

    /// Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a UTF-8 name.
    public static func newV5(namespace: UUID, name: String) throws -> UUID {
        try newV5(namespace: namespace, name: Array(name.utf8))
    }

    /// Creates a time-sortable UUID version 7 (RFC 9562 §6.2) from a Unix-epoch millisecond
    /// timestamp.
    public static func newV7(unixMillis: UInt64) throws -> UUID {
        let l = try loaded()
        var out = [UInt8](repeating: 0, count: 16)
        let rc: Int32 = out.withUnsafeMutableBufferPointer { outBuf in
            l.newV7(unixMillis, outBuf.baseAddress)
        }
        switch rc {
        case 0: return UUID(rfcBytes: out)
        case 2: throw Error.timestampOutOfRange
        default: throw Error.randomSourceFailure(code: rc)
        }
    }

    /// Creates a time-sortable UUID version 7 (RFC 9562 §6.2) using the current time.
    public static func newV7() throws -> UUID {
        try newV7(unixMillis: UInt64(Date().timeIntervalSince1970 * 1000))
    }

    /// Recovers the Unix-epoch millisecond timestamp embedded in a version 7 UUID's
    /// `unix_ts_ms` field. Only meaningful when `uuid`'s version nibble is 7 — the RFC 9562
    /// bit layout doesn't distinguish "not a v7 UUID" from "v7 UUID with a very early
    /// timestamp", so the caller is responsible for checking that first if it matters.
    public static func v7UnixMillis(_ uuid: UUID) throws -> UInt64 {
        let l = try loaded()
        let bytes = uuid.rfcBytes
        return bytes.withUnsafeBufferPointer { l.v7UnixMillis($0.baseAddress) }
    }

    /// Recovers the UTC timestamp embedded in a version 7 UUID as a `Date`.
    public static func v7Timestamp(_ uuid: UUID) throws -> Date {
        Date(timeIntervalSince1970: Double(try v7UnixMillis(uuid)) / 1000)
    }
}
