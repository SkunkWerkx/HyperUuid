import Foundation

/// RFC 9562 UUID generation (v4 random, v5 deterministic, v6/v7 time-sortable) calling directly
/// into the native `libhyperuuid` shared library via `dlopen`/`dlsym` (or the Windows
/// equivalent) plus an `@convention(c)` function-pointer cast — no runtime bridge, no cgo-
/// style shim (see `DynamicLibrary.swift`).
///
/// This package bundles a native build for every platform (see `NativePlatform`) and picks
/// the right one at compile time.
public enum UuidGenerator {
    /// An error returned when a native UUID generation call fails.
    public enum Error: Swift.Error, CustomStringConvertible {
        /// The native random source failed; `code` is the native call's raw return code.
        case randomSourceFailure(code: Int32)
        /// The Unix millisecond timestamp doesn't fit the timestamp field being generated.
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
    private typealias UuidNewV6Fn = @convention(c) (UInt64, UnsafeMutablePointer<UInt8>?) -> Int32
    private typealias UuidV6UnixMillisFn = @convention(c) (UnsafePointer<UInt8>?) -> UInt64
    private typealias UuidNewV6BatchFn = @convention(c) (UInt64, UInt32, UnsafeMutablePointer<UInt8>?) -> Int32
    private typealias UuidNewV7Fn = @convention(c) (UInt64, UnsafeMutablePointer<UInt8>?) -> Int32
    private typealias UuidV7UnixMillisFn = @convention(c) (UnsafePointer<UInt8>?) -> UInt64
    private typealias UuidNewV7BatchFn = @convention(c) (UInt64, UInt32, UnsafeMutablePointer<UInt8>?) -> Int32
    private typealias UuidV7ToSqlOrderFn = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Void
    private typealias UuidV7ToRfcOrderFn = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Void
    private typealias UuidV6ToSqlOrderFn = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Void
    private typealias UuidV6ToRfcOrderFn = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Void

    private struct LoadedLibrary {
        let library: DynamicLibrary
        let newV4: UuidNewV4Fn
        let newV5: UuidNewV5Fn
        let newV6: UuidNewV6Fn
        let v6UnixMillis: UuidV6UnixMillisFn
        let newV6Batch: UuidNewV6BatchFn
        let newV7: UuidNewV7Fn
        let v7UnixMillis: UuidV7UnixMillisFn
        let newV7Batch: UuidNewV7BatchFn
        let v7ToSqlOrder: UuidV7ToSqlOrderFn
        let v7ToRfcOrder: UuidV7ToRfcOrderFn
        let v6ToSqlOrder: UuidV6ToSqlOrderFn
        let v6ToRfcOrder: UuidV6ToRfcOrderFn
    }

    // Swift initializes `static let`s lazily and exactly once, thread-safely — the same
    // "loaded on first use" behavior the Java binding gets from `by lazy` / `object`. Unlike
    // Kotlin, that initializer can't itself `throw`, so failures are captured in a `Result`
    // and re-surfaced as a normal `throws` from `loaded()` rather than crashing the process.
    private static let loadResult: Result<LoadedLibrary, Swift.Error> = Result { try load() }

    private static func load() throws -> LoadedLibrary {
        let tempPath = try extractNativeLibrary()
        let library = try DynamicLibrary(path: tempPath)
        let newV4 = unsafeBitCast(try library.symbol("uuid_new_v4"), to: UuidNewV4Fn.self)
        let newV5 = unsafeBitCast(try library.symbol("uuid_new_v5"), to: UuidNewV5Fn.self)
        let newV6 = unsafeBitCast(try library.symbol("uuid_new_v6"), to: UuidNewV6Fn.self)
        let v6UnixMillis = unsafeBitCast(
            try library.symbol("uuid_v6_unix_millis"), to: UuidV6UnixMillisFn.self)
        let newV6Batch = unsafeBitCast(
            try library.symbol("uuid_new_v6_batch"), to: UuidNewV6BatchFn.self)
        let newV7 = unsafeBitCast(try library.symbol("uuid_new_v7"), to: UuidNewV7Fn.self)
        let v7UnixMillis = unsafeBitCast(
            try library.symbol("uuid_v7_unix_millis"), to: UuidV7UnixMillisFn.self)
        let newV7Batch = unsafeBitCast(
            try library.symbol("uuid_new_v7_batch"), to: UuidNewV7BatchFn.self)
        let v7ToSqlOrder = unsafeBitCast(
            try library.symbol("uuid_v7_to_sql_order"), to: UuidV7ToSqlOrderFn.self)
        let v7ToRfcOrder = unsafeBitCast(
            try library.symbol("uuid_v7_to_rfc_order"), to: UuidV7ToRfcOrderFn.self)
        let v6ToSqlOrder = unsafeBitCast(
            try library.symbol("uuid_v6_to_sql_order"), to: UuidV6ToSqlOrderFn.self)
        let v6ToRfcOrder = unsafeBitCast(
            try library.symbol("uuid_v6_to_rfc_order"), to: UuidV6ToRfcOrderFn.self)
        return LoadedLibrary(
            library: library, newV4: newV4, newV5: newV5,
            newV6: newV6, v6UnixMillis: v6UnixMillis, newV6Batch: newV6Batch,
            newV7: newV7, v7UnixMillis: v7UnixMillis, newV7Batch: newV7Batch,
            v7ToSqlOrder: v7ToSqlOrder, v7ToRfcOrder: v7ToRfcOrder,
            v6ToSqlOrder: v6ToSqlOrder, v6ToRfcOrder: v6ToRfcOrder)
    }

    private static func loaded() throws -> LoadedLibrary {
        switch loadResult {
        case .success(let l): return l
        case .failure(let e): throw e
        }
    }

    /// Extracts this platform's bundled native library (an SPM resource, which may not
    /// already be a plain filesystem path depending on how the package was packaged) to a
    /// temp file, mirroring the Go/Java bindings' approach. The temp file is deliberately
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

    /// Creates a time-sortable UUID version 6 (RFC 9562 §5.6), a field-compatible reordering
    /// of version 1 for better sort/index locality, from a Unix-epoch millisecond timestamp.
    /// `clock_seq` and `node` are randomly generated on every call — unlike version 7, there
    /// is no monotonic counter, so calls within the same millisecond are not guaranteed to
    /// sort in creation order.
    public static func newV6(unixMillis: UInt64) throws -> UUID {
        let l = try loaded()
        var out = [UInt8](repeating: 0, count: 16)
        let rc: Int32 = out.withUnsafeMutableBufferPointer { outBuf in
            l.newV6(unixMillis, outBuf.baseAddress)
        }
        switch rc {
        case 0: return UUID(rfcBytes: out)
        case 2: throw Error.timestampOutOfRange
        default: throw Error.randomSourceFailure(code: rc)
        }
    }

    /// Creates a time-sortable UUID version 6 (RFC 9562 §5.6) using the current time.
    public static func newV6() throws -> UUID {
        try newV6(unixMillis: UInt64(Date().timeIntervalSince1970 * 1000))
    }

    /// Creates a time-sortable UUID version 6 (RFC 9562 §5.6) from a `Date` — pulls the
    /// Unix-epoch milliseconds off `date` and mints it through `newV6(unixMillis:)`.
    public static func newV6(_ date: Date) throws -> UUID {
        try newV6(unixMillis: UInt64(date.timeIntervalSince1970 * 1000))
    }

    /// Recovers the Unix-epoch millisecond timestamp embedded in a version 6 UUID's
    /// timestamp field. Only meaningful when `uuid`'s version nibble is 6 — the RFC 9562 bit
    /// layout doesn't distinguish "not a v6 UUID" from "v6 UUID with a very early timestamp",
    /// so the caller is responsible for checking that first if it matters.
    public static func v6UnixMillis(_ uuid: UUID) throws -> UInt64 {
        let l = try loaded()
        let bytes = uuid.rfcBytes
        return bytes.withUnsafeBufferPointer { l.v6UnixMillis($0.baseAddress) }
    }

    /// Recovers the UTC timestamp embedded in a version 6 UUID as a `Date`.
    public static func v6Timestamp(_ uuid: UUID) throws -> Date {
        Date(timeIntervalSince1970: Double(try v6UnixMillis(uuid)) / 1000)
    }

    /// Creates `count` time-sortable version 6 UUIDs sharing one Unix-epoch millisecond
    /// timestamp capture — one native call and one random-bytes fetch instead of `count` of
    /// each. `clock_seq` and `node` are independently random per item.
    public static func newV6Batch(count: Int, unixMillis: UInt64) throws -> [UUID] {
        guard count > 0 else { return [] }
        let l = try loaded()
        var out = [UInt8](repeating: 0, count: count * 16)
        let rc: Int32 = out.withUnsafeMutableBufferPointer { outBuf in
            l.newV6Batch(unixMillis, UInt32(count), outBuf.baseAddress)
        }
        switch rc {
        case 0: return (0..<count).map { UUID(rfcBytes: Array(out[($0 * 16)..<($0 * 16 + 16)])) }
        case 2: throw Error.timestampOutOfRange
        default: throw Error.randomSourceFailure(code: rc)
        }
    }

    /// Creates `count` time-sortable version 6 UUIDs sharing the current time.
    public static func newV6Batch(count: Int) throws -> [UUID] {
        try newV6Batch(count: count, unixMillis: UInt64(Date().timeIntervalSince1970 * 1000))
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

    /// Creates a time-sortable UUID version 7 (RFC 9562 §6.2) from a `Date` — pulls the
    /// Unix-epoch milliseconds off `date` and mints it through `newV7(unixMillis:)`.
    public static func newV7(_ date: Date) throws -> UUID {
        try newV7(unixMillis: UInt64(date.timeIntervalSince1970 * 1000))
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

    /// Recovers the UTC timestamp embedded in `uuid` as a `Date`, or `nil` if it isn't a
    /// version 6 or 7 UUID. Unlike `v6Timestamp`/`v7Timestamp`, this reads the version nibble
    /// itself first, so a caller doesn't need to already know (or separately check) which
    /// version `uuid` is before asking — delegates straight to whichever of those two methods
    /// applies, no bit-layout logic duplicated here. Still `throws` for a real native-load
    /// failure, same as every other call in this type.
    public static func getTimestamp(_ uuid: UUID) throws -> Date? {
        switch uuid.rfcBytes[6] >> 4 {
        case 6: return try v6Timestamp(uuid)
        case 7: return try v7Timestamp(uuid)
        default: return nil
        }
    }

    /// Creates `count` time-sortable version 7 UUIDs sharing one Unix-epoch millisecond
    /// timestamp capture and one contiguous block of the monotonic counter — one native call
    /// and one random-bytes fetch instead of `count` of each.
    public static func newV7Batch(count: Int, unixMillis: UInt64) throws -> [UUID] {
        guard count > 0 else { return [] }
        let l = try loaded()
        var out = [UInt8](repeating: 0, count: count * 16)
        let rc: Int32 = out.withUnsafeMutableBufferPointer { outBuf in
            l.newV7Batch(unixMillis, UInt32(count), outBuf.baseAddress)
        }
        switch rc {
        case 0: return (0..<count).map { UUID(rfcBytes: Array(out[($0 * 16)..<($0 * 16 + 16)])) }
        case 2: throw Error.timestampOutOfRange
        default: throw Error.randomSourceFailure(code: rc)
        }
    }

    /// Creates `count` time-sortable version 7 UUIDs sharing the current time.
    public static func newV7Batch(count: Int) throws -> [UUID] {
        try newV7Batch(count: count, unixMillis: UInt64(Date().timeIntervalSince1970 * 1000))
    }

    /// Converts an RFC 9562-ordered version 7 `uuid` to the byte order SQL Server's
    /// `uniqueidentifier` needs on the wire to sort by creation order.
    ///
    /// `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a
    /// `uniqueidentifier` column — doesn't compare a GUID's 16 bytes left to right; it uses a
    /// fixed, non-sequential byte significance order (octets `10,11,12,13,14,15, 8,9, 6,7,
    /// 4,5, 0,1,2,3`, most significant first). This moves the timestamp and counter — the two
    /// fields that determine creation order — into those most-significant octets, and moves
    /// the trailing entropy, which carries no ordering information, into the least-significant
    /// ones as one intact block. The permutation is computed once in the native Rust core, and
    /// verified there — and independently, against the real `System.Data.SqlTypes.SqlGuid`
    /// comparator — in this project's C# test suite; this binding calls the same native
    /// function rather than reimplementing the math.
    ///
    /// Meaningful only for a genuine version 7 UUID; see `v6ToSqlOrder` for v6.
    public static func v7ToSqlOrder(_ uuid: UUID) throws -> UUID {
        let l = try loaded()
        var bytes = uuid.rfcBytes
        bytes.withUnsafeMutableBufferPointer { l.v7ToSqlOrder($0.baseAddress) }
        return UUID(rfcBytes: bytes)
    }

    /// Inverse of `v7ToSqlOrder` — converts a SQL-Server-ordered version 7 `uuid` back to
    /// RFC 9562 order.
    public static func v7FromSqlOrder(_ uuid: UUID) throws -> UUID {
        let l = try loaded()
        var bytes = uuid.rfcBytes
        bytes.withUnsafeMutableBufferPointer { l.v7ToRfcOrder($0.baseAddress) }
        return UUID(rfcBytes: bytes)
    }

    /// Converts an RFC 9562-ordered version 6 `uuid` to the byte order SQL Server's
    /// `uniqueidentifier` needs on the wire to sort by creation order.
    ///
    /// Same `SqlGuid` significance order as `v7ToSqlOrder`, applied to v6's very different
    /// field layout. v6 has no monotonic counter the way v7 does; the only field that
    /// determines its creation order is the 60-bit timestamp itself, so this moves that whole
    /// timestamp — most significant chunk first — into the comparison's most significant
    /// octets. Everything after it — `variant`, `clock_seq`, and `node` (octets 8-15, already
    /// one contiguous run with no ordering value of its own — `clock_seq`/`node` are randomly
    /// generated per call, not a counter, and `variant` is a fixed constant either way) —
    /// moves as that single 8-byte span into the remaining octets, in the same relative order,
    /// not individually reshuffled. Version and variant end up at different
    /// byte offsets than `v7ToSqlOrder`'s result (octet 8's top nibble and octet 6's top two
    /// bits here, not 7/8) — fine, since the two versions are separate methods and a caller
    /// always knows which one it's calling.
    ///
    /// Unlike v7, two version 6 UUIDs minted at the same millisecond have identical timestamp
    /// bits — `clock_seq`/`node` are independently random, not a counter — so this doesn't
    /// (and can't) make same-millisecond v6 UUIDs sort in creation order any more than plain
    /// RFC order already does. Distinct timestamps sort correctly; same-timestamp ties don't,
    /// by the RFC's own v6 design, not a limitation introduced here.
    ///
    /// Meaningful only for a genuine version 6 UUID.
    public static func v6ToSqlOrder(_ uuid: UUID) throws -> UUID {
        let l = try loaded()
        var bytes = uuid.rfcBytes
        bytes.withUnsafeMutableBufferPointer { l.v6ToSqlOrder($0.baseAddress) }
        return UUID(rfcBytes: bytes)
    }

    /// Inverse of `v6ToSqlOrder` — converts a SQL-Server-ordered version 6 `uuid` back to
    /// RFC 9562 order.
    public static func v6FromSqlOrder(_ uuid: UUID) throws -> UUID {
        let l = try loaded()
        var bytes = uuid.rfcBytes
        bytes.withUnsafeMutableBufferPointer { l.v6ToRfcOrder($0.baseAddress) }
        return UUID(rfcBytes: bytes)
    }
}
