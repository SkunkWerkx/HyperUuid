import Foundation
import HyperUuidCore

/// RFC 9562 UUID generation (v4 random, v5 deterministic, v6/v7 time-sortable) calling directly
/// into the native `hyperuuid` core through `@convention(c)` function pointers — no runtime
/// bridge, no cgo-style shim.
///
/// The core is a static library linked into the executable on every platform this package
/// builds for (`HyperUuidCore`, a SwiftPM binary target), so there is nothing to find, open
/// or deploy at run time. Every call `throws` ``Error`` when a native call ran and failed.
public enum UuidGenerator {
    /// An error returned when a native UUID generation call fails.
    public enum Error: Swift.Error, CustomStringConvertible, LocalizedError {
        /// The native random source failed; `code` is the native call's raw return code.
        case randomSourceFailure(code: Int32)
        /// The Unix millisecond timestamp doesn't fit the timestamp field being generated.
        case timestampOutOfRange
        /// A destination buffer's length wasn't a whole number of 16-byte UUIDs, or a buffer
        /// that holds one UUID wasn't exactly 16 bytes.
        case bufferNotWholeUUIDs(count: Int)
        /// A version 7 batch or fill asked for `count` UUIDs, more than
        /// ``UuidGenerator/maxV7Batch``. Refused before anything is allocated or written.
        case batchTooLarge(count: Int)
        /// A batch or fill of `count` UUIDs is too large for one native call on this
        /// platform: its byte length can't be addressed, or the count doesn't fit the
        /// call's 32-bit count. Refused before anything is allocated or written.
        case batchNotAddressable(count: Int)

        /// What was refused, in one line.
        public var description: String {
            switch self {
            case .randomSourceFailure(let code):
                return "hyperuuid: native call failed with code \(code) (random source failure)"
            case .timestampOutOfRange:
                return "hyperuuid: unix millisecond timestamp must be non-negative and fit within 48 bits"
            case .bufferNotWholeUUIDs(let count):
                return
                    "hyperuuid: destination length must be a multiple of 16 (one whole UUID per 16 bytes); got \(count)"
            case .batchTooLarge(let count):
                return
                    "hyperuuid: a single version 7 batch takes at most \(UuidGenerator.maxV7Batch) UUIDs (the 26-bit counter space); got \(count)"
            case .batchNotAddressable(let count):
                return "hyperuuid: a batch of \(count) UUIDs is too large for one native call on this platform"
            }
        }

        /// The same text as ``description``, so `localizedDescription` names the failure too
        /// instead of Foundation's generic "The operation couldn't be completed".
        public var errorDescription: String? { description }
    }

    private typealias UuidNewV4Fn = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Int32
    private typealias UuidNewV5Fn =
        @convention(c) (
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
    private typealias VersionFn = @convention(c) () -> UInt32
    private typealias UuidVersionFn = @convention(c) (UnsafePointer<UInt8>?, UInt32) -> UInt32
    private typealias UuidVariantFn = @convention(c) (UnsafePointer<UInt8>?) -> UInt32
    private typealias UuidIsRfcFn = @convention(c) (UnsafePointer<UInt8>?, UInt32, UInt32) -> UInt32
    private typealias UuidUnixMillisInFn = @convention(c) (UnsafePointer<UInt8>?, UInt32) -> UInt64
    private typealias UuidGetTimestampFn =
        @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafeMutablePointer<UInt64>?) -> UInt32

    // The native exports as one table, so every door reaches them the same way. A class:
    // a reference is one retain where a struct of function pointers would be copied.
    // Immutable once built, and C function pointers carry no state of their own.
    private final class LoadedLibrary: Sendable {
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
        let version: VersionFn
        let uuidVersion: UuidVersionFn
        let uuidVariant: UuidVariantFn
        let uuidIsRfc: UuidIsRfcFn
        let v6UnixMillisIn: UuidUnixMillisInFn
        let v7UnixMillisIn: UuidUnixMillisInFn
        let getTimestamp: UuidGetTimestampFn

        init(
            newV4: UuidNewV4Fn, newV5: UuidNewV5Fn,
            newV6: UuidNewV6Fn, v6UnixMillis: UuidV6UnixMillisFn, newV6Batch: UuidNewV6BatchFn,
            newV7: UuidNewV7Fn, v7UnixMillis: UuidV7UnixMillisFn, newV7Batch: UuidNewV7BatchFn,
            v7ToSqlOrder: UuidV7ToSqlOrderFn, v7ToRfcOrder: UuidV7ToRfcOrderFn,
            v6ToSqlOrder: UuidV6ToSqlOrderFn, v6ToRfcOrder: UuidV6ToRfcOrderFn,
            version: VersionFn,
            uuidVersion: UuidVersionFn, uuidVariant: UuidVariantFn, uuidIsRfc: UuidIsRfcFn,
            v6UnixMillisIn: UuidUnixMillisInFn, v7UnixMillisIn: UuidUnixMillisInFn,
            getTimestamp: UuidGetTimestampFn
        ) {
            self.newV4 = newV4; self.newV5 = newV5
            self.newV6 = newV6; self.v6UnixMillis = v6UnixMillis; self.newV6Batch = newV6Batch
            self.newV7 = newV7; self.v7UnixMillis = v7UnixMillis; self.newV7Batch = newV7Batch
            self.v7ToSqlOrder = v7ToSqlOrder; self.v7ToRfcOrder = v7ToRfcOrder
            self.v6ToSqlOrder = v6ToSqlOrder; self.v6ToRfcOrder = v6ToRfcOrder
            self.version = version
            self.uuidVersion = uuidVersion; self.uuidVariant = uuidVariant; self.uuidIsRfc = uuidIsRfc
            self.v6UnixMillisIn = v6UnixMillisIn; self.v7UnixMillisIn = v7UnixMillisIn
            self.getTimestamp = getTimestamp
        }
    }

    // Foundation's UUID wraps `uuid_t`, sixteen bytes already in RFC 9562 order — so a
    // `uuid_t` on the stack is both the scratch every single-UUID door needs and the value
    // the result is built from. No heap `[UInt8]` on either side of the call, which is
    // what every door here used to allocate (one for the out-value, one more per input).
    private static let zero: uuid_t = (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)

    private static func withOut<T>(_ body: (UnsafeMutablePointer<UInt8>) -> T) -> (T, uuid_t) {
        var out = zero
        let result = withUnsafeMutablePointer(to: &out) {
            body(UnsafeMutableRawPointer($0).assumingMemoryBound(to: UInt8.self))
        }
        return (result, out)
    }

    private static func withBytes<T>(of uuid: UUID, _ body: (UnsafePointer<UInt8>) -> T) -> T {
        var bytes = uuid.uuid
        return withUnsafePointer(to: &bytes) {
            body(UnsafeRawPointer($0).assumingMemoryBound(to: UInt8.self))
        }
    }

    /// The same permutation over a copy of the value's own bytes, returned as a UUID.
    private static func reorder(_ uuid: UUID, _ fn: UuidV7ToSqlOrderFn) -> UUID {
        var bytes = uuid.uuid
        withUnsafeMutablePointer(to: &bytes) {
            fn(UnsafeMutableRawPointer($0).assumingMemoryBound(to: UInt8.self))
        }
        return UUID(uuid: bytes)
    }

    // Linked in: the C declarations are the table, and there is nothing to find or open.
    private static let library = LoadedLibrary(
        newV4: uuid_new_v4, newV5: uuid_new_v5,
        newV6: uuid_new_v6, v6UnixMillis: uuid_v6_unix_millis, newV6Batch: uuid_new_v6_batch,
        newV7: uuid_new_v7, v7UnixMillis: uuid_v7_unix_millis, newV7Batch: uuid_new_v7_batch,
        v7ToSqlOrder: uuid_v7_to_sql_order, v7ToRfcOrder: uuid_v7_to_rfc_order,
        v6ToSqlOrder: uuid_v6_to_sql_order, v6ToRfcOrder: uuid_v6_to_rfc_order,
        version: hyperuuid_version,
        uuidVersion: uuid_version, uuidVariant: uuid_variant, uuidIsRfc: uuid_is_rfc,
        v6UnixMillisIn: uuid_v6_unix_millis_in, v7UnixMillisIn: uuid_v7_unix_millis_in,
        getTimestamp: uuid_get_timestamp)

    // `throws` only so every door keeps one shape; the core is always there.
    private static func loaded() throws -> LoadedLibrary {
        library
    }

    /// Whether the native core is usable. Always `true`: the core is linked into the
    /// executable on every platform this package builds for, and a platform with no
    /// prebuilt core fails to compile rather than at run time. Kept for callers that gated
    /// on it when macOS and Windows loaded a shared library.
    public static var isAvailable: Bool { true }

    /// Unix-epoch milliseconds off a `Date`, for the doors that take one. `UInt64(_:)` traps
    /// on a negative, NaN or infinite `Double`, and none of those is a caller bug worth a
    /// crash: a pre-1970 date is ``Error/timestampOutOfRange`` like any other timestamp the
    /// field can't hold. A date too far in the future to trap on is passed through for the
    /// native core to refuse with the same error.
    private static func unixMillis(of date: Date) throws -> UInt64 {
        let millis = date.timeIntervalSince1970 * 1000
        // NaN fails both comparisons; 2⁶⁴ is the first value `UInt64(_:)` can't take.
        guard millis >= 0, millis < 0x1p64 else { throw Error.timestampOutOfRange }
        return UInt64(millis)
    }

    /// Creates a random UUID version 4 (RFC 9562 §5.4).
    public static func newV4() throws -> UUID {
        let l = try loaded()
        let (rc, out) = withOut { l.newV4($0) }
        guard rc == 0 else { throw Error.randomSourceFailure(code: rc) }
        return UUID(uuid: out)
    }

    /// Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and raw name
    /// bytes. The same (namespace, name) pair always produces the same UUID.
    public static func newV5(namespace: UUID, name: [UInt8]) throws -> UUID {
        try name.withUnsafeBytes { try newV5(namespace: namespace, name: $0) }
    }

    /// See ``newV5(namespace:name:)-swift.type.method``; the name as a raw view of bytes — the
    /// primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a buffer.
    public static func newV5(namespace: UUID, name: UnsafeRawBufferPointer) throws -> UUID {
        let l = try loaded()
        // An empty buffer may carry a nil base address; the ABI never dereferences at len 0.
        let (rc, out) = withBytes(of: namespace) { ns in
            withOut { l.newV5(ns, name.baseAddress?.assumingMemoryBound(to: UInt8.self), UInt32(name.count), $0) }
        }
        guard rc == 0 else { throw Error.randomSourceFailure(code: rc) }
        return UUID(uuid: out)
    }

    /// Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a UTF-8 name.
    public static func newV5(namespace: UUID, name: String) throws -> UUID {
        // A native Swift String already stores contiguous UTF-8; withUTF8 hands the door a
        // view of the string's own bytes — no Array(name.utf8) copy.
        var name = name
        return try name.withUTF8 { try newV5(namespace: namespace, name: UnsafeRawBufferPointer($0)) }
    }

    /// Creates a time-sortable UUID version 6 (RFC 9562 §5.6), a field-compatible reordering
    /// of version 1 for better sort/index locality, from a Unix-epoch millisecond timestamp.
    /// `clock_seq` and `node` are randomly generated on every call — unlike version 7, there
    /// is no monotonic counter, so calls within the same millisecond are not guaranteed to
    /// sort in creation order.
    public static func newV6(unixMillis: UInt64) throws -> UUID {
        let l = try loaded()
        let (rc, out) = withOut { l.newV6(unixMillis, $0) }
        switch rc {
        case 0: return UUID(uuid: out)
        case 2: throw Error.timestampOutOfRange
        default: throw Error.randomSourceFailure(code: rc)
        }
    }

    /// Creates a time-sortable UUID version 6 (RFC 9562 §5.6) using the current time.
    public static func newV6() throws -> UUID {
        try newV6(unixMillis: unixMillis(of: Date()))
    }

    /// Creates a time-sortable UUID version 6 (RFC 9562 §5.6) from a `Date` — pulls the
    /// Unix-epoch milliseconds off `date` and mints it through `newV6(unixMillis:)`. A date
    /// before 1970, or one that isn't a finite instant at all, throws
    /// ``Error/timestampOutOfRange`` like any other timestamp the field can't hold.
    public static func newV6(_ date: Date) throws -> UUID {
        try newV6(unixMillis: unixMillis(of: date))
    }

    /// Recovers the Unix-epoch millisecond timestamp embedded in a version 6 UUID's
    /// timestamp field. Only meaningful when `uuid`'s version nibble is 6 — the RFC 9562 bit
    /// layout doesn't distinguish "not a v6 UUID" from "v6 UUID with a very early timestamp",
    /// so the caller is responsible for checking that first if it matters.
    public static func v6UnixMillis(_ uuid: UUID) throws -> UInt64 {
        let l = try loaded()
        return withBytes(of: uuid) { l.v6UnixMillis($0) }
    }

    /// Recovers the UTC timestamp embedded in a version 6 UUID as a `Date`.
    public static func v6Timestamp(_ uuid: UUID) throws -> Date {
        Date(timeIntervalSince1970: Double(try v6UnixMillis(uuid)) / 1000)
    }

    /// Creates `count` time-sortable version 6 UUIDs sharing one Unix-epoch millisecond
    /// timestamp capture — one native call and one random-bytes fetch instead of `count` of
    /// each. `clock_seq` and `node` are independently random per item.
    ///
    /// - Precondition: `count` is not negative — a caller bug, the same one
    ///   `Array(repeating:count:)` traps on. Zero is an empty array.
    public static func newV6Batch(count: Int, unixMillis: UInt64) throws -> [UUID] {
        precondition(count >= 0, "count must not be negative; got \(count)")
        guard count > 0 else { return [] }
        guard UInt64(count) <= UInt64(UInt32.max) else { throw Error.batchNotAddressable(count: count) }
        // The result array is the destination: one native call writes every UUID in place,
        // with no scratch buffer and no per-element construction — the fill's own path.
        var result = [UUID](repeating: UUID(uuid: zero), count: count)
        try fillV6(into: &result, unixMillis: unixMillis)
        return result
    }

    /// Creates `count` time-sortable version 6 UUIDs sharing the current time.
    public static func newV6Batch(count: Int) throws -> [UUID] {
        try newV6Batch(count: count, unixMillis: unixMillis(of: Date()))
    }

    /// Creates a time-sortable UUID version 7 (RFC 9562 §6.2) from a Unix-epoch millisecond
    /// timestamp.
    public static func newV7(unixMillis: UInt64) throws -> UUID {
        let l = try loaded()
        let (rc, out) = withOut { l.newV7(unixMillis, $0) }
        switch rc {
        case 0: return UUID(uuid: out)
        case 2: throw Error.timestampOutOfRange
        default: throw Error.randomSourceFailure(code: rc)
        }
    }

    /// Creates a time-sortable UUID version 7 (RFC 9562 §6.2) using the current time.
    public static func newV7() throws -> UUID {
        try newV7(unixMillis: unixMillis(of: Date()))
    }

    /// Creates a time-sortable UUID version 7 (RFC 9562 §6.2) from a `Date` — pulls the
    /// Unix-epoch milliseconds off `date` and mints it through `newV7(unixMillis:)`. A date
    /// before 1970, or one that isn't a finite instant at all, throws
    /// ``Error/timestampOutOfRange`` like any other timestamp the field can't hold.
    public static func newV7(_ date: Date) throws -> UUID {
        try newV7(unixMillis: unixMillis(of: date))
    }

    /// Recovers the Unix-epoch millisecond timestamp embedded in a version 7 UUID's
    /// `unix_ts_ms` field. Only meaningful when `uuid`'s version nibble is 7 — the RFC 9562
    /// bit layout doesn't distinguish "not a v7 UUID" from "v7 UUID with a very early
    /// timestamp", so the caller is responsible for checking that first if it matters.
    public static func v7UnixMillis(_ uuid: UUID) throws -> UInt64 {
        let l = try loaded()
        return withBytes(of: uuid) { l.v7UnixMillis($0) }
    }

    /// Recovers the UTC timestamp embedded in a version 7 UUID as a `Date`.
    public static func v7Timestamp(_ uuid: UUID) throws -> Date {
        Date(timeIntervalSince1970: Double(try v7UnixMillis(uuid)) / 1000)
    }

    /// Recovers the UTC timestamp embedded in `uuid` as a `Date`, or `nil` for anything that
    /// isn't an RFC 9562 version 6 or 7 UUID. Unlike `v6Timestamp`/`v7Timestamp`, this checks
    /// the variant as well as the version, in one native call, so a caller doesn't need to
    /// already know (or separately check) what `uuid` is before asking — a 6 or 7 nibble under
    /// another variant has no RFC version and so no timestamp. No bit-layout logic here. Every 48-bit v7 timestamp
    /// has a `Date`, up to 2^48 − 1 ms (year 10889).
    public static func getTimestamp(_ uuid: UUID) throws -> Date? {
        try getTimestamp(uuid, layout: .rfc9562)
    }

    /// The most UUIDs one version 7 batch (`newV7Batch`, `fillV7` and their overloads)
    /// mints: 67,108,864, the size of the 26-bit counter that orders UUIDs within a
    /// millisecond.
    ///
    /// Every batch up to this size is in strictly increasing order. The counter is one
    /// process-wide sequence, so a batch can straddle the point where it wraps back to 0; the
    /// UUIDs from there on carry a timestamp one millisecond later than the one supplied
    /// rather than sorting before the ones ahead of them. A larger batch would have to reuse
    /// counter values within one millisecond, so it throws ``Error/batchTooLarge(count:)``
    /// before anything is allocated or written. Version 6 has no counter and no such limit.
    public static let maxV7Batch = 1 << 26

    /// Creates `count` time-sortable version 7 UUIDs sharing one Unix-epoch millisecond
    /// timestamp capture and one contiguous block of the monotonic counter — one native call
    /// and one random-bytes fetch instead of `count` of each. The batch is strictly
    /// increasing; if it crosses the counter's wrap, the UUIDs from there on carry
    /// `unixMillis + 1` (see ``maxV7Batch``).
    ///
    /// A `count` over ``maxV7Batch`` throws ``Error/batchTooLarge(count:)`` before the array
    /// is allocated.
    ///
    /// - Precondition: `count` is not negative — a caller bug, the same one
    ///   `Array(repeating:count:)` traps on. Zero is an empty array.
    public static func newV7Batch(count: Int, unixMillis: UInt64) throws -> [UUID] {
        precondition(count >= 0, "count must not be negative; got \(count)")
        guard count > 0 else { return [] }
        guard count <= maxV7Batch else { throw Error.batchTooLarge(count: count) }
        // The result array is the destination: one native call writes every UUID in place,
        // with no scratch buffer and no per-element construction — the fill's own path.
        var result = [UUID](repeating: UUID(uuid: zero), count: count)
        try fillV7(into: &result, unixMillis: unixMillis)
        return result
    }

    /// Creates `count` time-sortable version 7 UUIDs sharing the current time.
    public static func newV7Batch(count: Int) throws -> [UUID] {
        try newV7Batch(count: count, unixMillis: unixMillis(of: Date()))
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
        reorder(uuid, try loaded().v7ToSqlOrder)
    }

    /// Inverse of `v7ToSqlOrder` — converts a SQL-Server-ordered version 7 `uuid` back to
    /// RFC 9562 order.
    public static func v7FromSqlOrder(_ uuid: UUID) throws -> UUID {
        reorder(uuid, try loaded().v7ToRfcOrder)
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
        reorder(uuid, try loaded().v6ToSqlOrder)
    }

    /// Inverse of `v6ToSqlOrder` — converts a SQL-Server-ordered version 6 `uuid` back to
    /// RFC 9562 order.
    public static func v6FromSqlOrder(_ uuid: UUID) throws -> UUID {
        reorder(uuid, try loaded().v6ToRfcOrder)
    }

    // MARK: - Destination-buffer fills

    /// Fills `destination` with time-sortable version 7 UUIDs sharing one `unixMillis`
    /// timestamp capture and one contiguous block of the monotonic counter.
    ///
    /// `newV7Batch(count:)` allocates a fresh array on every call; this writes into storage
    /// the caller already owns, which is what lets a hot path reuse one buffer across batches.
    /// `destination` is raw RFC 9562-ordered bytes, 16 per UUID, and its length must be a whole
    /// multiple of 16. The UUIDs are strictly increasing, with the same possible
    /// `unixMillis + 1` as ``newV7Batch(count:unixMillis:)``; more than ``maxV7Batch`` of them
    /// throws ``Error/batchTooLarge(count:)`` with nothing written.
    public static func fillV7(into destination: UnsafeMutableRawBufferPointer, unixMillis: UInt64) throws {
        try fill(into: destination, unixMillis: unixMillis, limit: maxV7Batch) { l in l.newV7Batch }
    }

    /// Fills `destination` with version 7 UUIDs sharing the current time.
    public static func fillV7(into destination: UnsafeMutableRawBufferPointer) throws {
        try fillV7(into: destination, unixMillis: unixMillis(of: Date()))
    }

    /// Fills `destination` with time-sortable version 6 UUIDs sharing one `unixMillis`
    /// timestamp capture. `clock_seq` and `node` are independently random per item — unlike
    /// version 7 there is no monotonic counter, so items are not guaranteed to sort in
    /// creation order.
    public static func fillV6(into destination: UnsafeMutableRawBufferPointer, unixMillis: UInt64) throws {
        try fill(into: destination, unixMillis: unixMillis, limit: nil) { l in l.newV6Batch }
    }

    /// Fills `destination` with version 6 UUIDs sharing the current time.
    public static func fillV6(into destination: UnsafeMutableRawBufferPointer) throws {
        try fillV6(into: destination, unixMillis: unixMillis(of: Date()))
    }

    /// Fills `destination` with version 7 UUIDs, writing straight into the array's storage.
    ///
    /// Foundation's `UUID` wraps `uuid_t` — 16 bytes in RFC 9562 order — so a contiguous
    /// `[UUID]` is exactly the layout the native core writes and no per-element conversion is
    /// needed. The layout is asserted at runtime rather than assumed. Strictly increasing and
    /// limited to ``maxV7Batch`` UUIDs, as the raw-buffer `fillV7` is.
    public static func fillV7(into destination: inout [UUID], unixMillis: UInt64) throws {
        try fillUUIDs(into: &destination, unixMillis: unixMillis, limit: maxV7Batch) { l in l.newV7Batch }
    }

    /// Fills `destination` with version 7 UUIDs sharing the current time, writing straight
    /// into the array's storage.
    public static func fillV7(into destination: inout [UUID]) throws {
        try fillV7(into: &destination, unixMillis: unixMillis(of: Date()))
    }

    /// Fills `destination` with version 6 UUIDs, writing straight into the array's storage.
    public static func fillV6(into destination: inout [UUID], unixMillis: UInt64) throws {
        try fillUUIDs(into: &destination, unixMillis: unixMillis, limit: nil) { l in l.newV6Batch }
    }

    /// Fills `destination` with version 6 UUIDs sharing the current time, writing straight
    /// into the array's storage.
    public static func fillV6(into destination: inout [UUID]) throws {
        try fillV6(into: &destination, unixMillis: unixMillis(of: Date()))
    }

    // `limit` is the v7 counter space, checked here before the native call so the core's
    // own refusal (code 4) is a backstop; v6 has none. Either way the count must fit the
    // call's UInt32, which a conversion would otherwise trap on.
    private static func fill(
        into destination: UnsafeMutableRawBufferPointer,
        unixMillis: UInt64,
        limit: Int?,
        _ pick: (LoadedLibrary) -> UuidNewV7BatchFn
    ) throws {
        guard destination.count % 16 == 0 else {
            throw Error.bufferNotWholeUUIDs(count: destination.count)
        }
        let count = destination.count / 16
        guard count > 0 else { return }
        if let limit, count > limit { throw Error.batchTooLarge(count: count) }
        guard let nativeCount = UInt32(exactly: count) else { throw Error.batchNotAddressable(count: count) }
        let l = try loaded()
        let rc = pick(l)(
            unixMillis,
            nativeCount,
            destination.baseAddress?.assumingMemoryBound(to: UInt8.self)
        )
        switch rc {
        case 0: return
        case 2: throw Error.timestampOutOfRange
        case 3: throw Error.batchNotAddressable(count: count)
        case 4: throw Error.batchTooLarge(count: count)
        default: throw Error.randomSourceFailure(code: rc)
        }
    }

    private static func fillUUIDs(
        into destination: inout [UUID],
        unixMillis: UInt64,
        limit: Int?,
        _ pick: (LoadedLibrary) -> UuidNewV7BatchFn
    ) throws {
        guard !destination.isEmpty else { return }
        precondition(
            MemoryLayout<UUID>.size == 16 && MemoryLayout<UUID>.stride == 16,
            "UUID is not a contiguous 16-byte value; the direct fill below would be unsound"
        )
        var thrown: Swift.Error?
        destination.withUnsafeMutableBytes { raw in
            do { try fill(into: raw, unixMillis: unixMillis, limit: limit, pick) } catch { thrown = error }
        }
        if let thrown { throw thrown }
    }

    // MARK: - Raw-byte SQL-order transforms

    /// Rewrites the 16 RFC 9562-ordered version 7 bytes in `uuid` into SQL Server
    /// `uniqueidentifier` sort order, in place. See `v7ToSqlOrder(_:)` for the rationale.
    public static func v7ToSqlOrder(bytes uuid: UnsafeMutableRawBufferPointer) throws {
        try sqlOrder(uuid) { l in l.v7ToSqlOrder }
    }

    /// Inverse of `v7ToSqlOrder(bytes:)`, in place.
    public static func v7FromSqlOrder(bytes uuid: UnsafeMutableRawBufferPointer) throws {
        try sqlOrder(uuid) { l in l.v7ToRfcOrder }
    }

    /// Rewrites the 16 RFC 9562-ordered version 6 bytes in `uuid` into SQL Server
    /// `uniqueidentifier` sort order, in place. See `v6ToSqlOrder(_:)` for the rationale.
    public static func v6ToSqlOrder(bytes uuid: UnsafeMutableRawBufferPointer) throws {
        try sqlOrder(uuid) { l in l.v6ToSqlOrder }
    }

    /// Inverse of `v6ToSqlOrder(bytes:)`, in place.
    public static func v6FromSqlOrder(bytes uuid: UnsafeMutableRawBufferPointer) throws {
        try sqlOrder(uuid) { l in l.v6ToRfcOrder }
    }

    private static func sqlOrder(
        _ uuid: UnsafeMutableRawBufferPointer,
        _ pick: (LoadedLibrary) -> UuidV7ToSqlOrderFn
    ) throws {
        guard uuid.count == 16 else { throw Error.bufferNotWholeUUIDs(count: uuid.count) }
        let l = try loaded()
        pick(l)(uuid.baseAddress?.assumingMemoryBound(to: UInt8.self))
    }

    // MARK: - Inspection and layout-aware timestamps
    //
    // The layout knowledge lives in the core: these only hand it the value's own 16 bytes and
    // the layout's code. A SQL-ordered UUID is the one `v7ToSqlOrder`/`v6ToSqlOrder` return,
    // whose `uuid` bytes are the SQL Server wire bytes, and an RFC-ordered one's `uuid` bytes
    // are RFC 9562 order, so `uuid.uuid` is the right input in either layout with no
    // conversion. Every `UuidLayout` is a defined layout, so none of these can be handed an
    // unknown one.

    /// The version of a `uuid` held in `layout`'s byte order.
    ///
    /// In ``UuidLayout/rfc9562`` (the default) this is the RFC 9562 version nibble, 0 through
    /// 15 — 0 for Nil, 15 for Max — and says nothing about the variant: use
    /// ``isRfc(_:version:layout:)`` when the question is "an RFC 9562 UUID of version N".
    ///
    /// ``UuidLayout/sqlServer`` is defined only for the two versions that have a SQL Server
    /// order, and answers 6, 7, or 0 when the bytes don't form a SQL-ordered version 6 or 7
    /// RFC 9562 UUID. The version nibble lands at a different byte for each, and the other
    /// version's random bits can mimic it there, so the core checks the variant bits too,
    /// where each version puts them, and the answer never confuses the two. The layout is the
    /// caller's to know: an RFC-ordered UUID's bytes can genuinely form a SQL-ordered v7 (one
    /// random v4 in 16 does), so reading one as ``UuidLayout/sqlServer`` can answer 7.
    public static func version(_ uuid: UUID, layout: UuidLayout = .rfc9562) throws -> Int {
        let l = try loaded()
        return Int(withBytes(of: uuid) { l.uuidVersion($0, layout.rawValue) })
    }

    /// ``version(_:layout:)`` over 16 raw bytes already in `layout`'s order (RFC 9562 network
    /// order by default). Throws ``Error/bufferNotWholeUUIDs(count:)`` unless `uuid` is
    /// exactly 16 bytes.
    public static func version(bytes uuid: UnsafeRawBufferPointer, layout: UuidLayout = .rfc9562) throws -> Int {
        let l = try loaded()
        return Int(try withSingleUuid(uuid) { l.uuidVersion($0, layout.rawValue) })
    }

    /// The variant field of an RFC 9562-ordered `uuid` (RFC 9562 §4.1): ``UuidVariant/ncs``
    /// for Nil, ``UuidVariant/future`` for Max, and ``UuidVariant/rfc9562`` for anything this
    /// library or Foundation's `UUID()` mints.
    public static func variant(_ uuid: UUID) throws -> UuidVariant {
        let l = try loaded()
        return variantOf(withBytes(of: uuid) { l.uuidVariant($0) })
    }

    /// ``variant(_:)`` over 16 raw RFC 9562-ordered bytes. Throws
    /// ``Error/bufferNotWholeUUIDs(count:)`` unless `uuid` is exactly 16 bytes.
    public static func variant(bytes uuid: UnsafeRawBufferPointer) throws -> UuidVariant {
        let l = try loaded()
        return variantOf(try withSingleUuid(uuid) { l.uuidVariant($0) })
    }

    /// Whether a `uuid` held in `layout`'s byte order is an RFC 9562 UUID of version
    /// `version` — the RFC variant and that version, in one native call. The guard to run
    /// before trusting a value's version-specific fields, such as a version 7's timestamp. In
    /// ``UuidLayout/sqlServer`` only versions 6 and 7 can be `true` (see
    /// ``version(_:layout:)``). A `version` outside 0–15 is simply never matched.
    public static func isRfc(_ uuid: UUID, version: Int, layout: UuidLayout = .rfc9562) throws -> Bool {
        let l = try loaded()
        guard let code = versionCode(version) else { return false }
        return withBytes(of: uuid) { l.uuidIsRfc($0, code, layout.rawValue) } != 0
    }

    /// ``isRfc(_:version:layout:)`` over 16 raw bytes already in `layout`'s order (RFC 9562
    /// network order by default). Throws ``Error/bufferNotWholeUUIDs(count:)`` unless `uuid`
    /// is exactly 16 bytes.
    public static func isRfc(
        bytes uuid: UnsafeRawBufferPointer, version: Int, layout: UuidLayout = .rfc9562
    ) throws -> Bool {
        let l = try loaded()
        let answer = try withSingleUuid(uuid) { p in versionCode(version).map { l.uuidIsRfc(p, $0, layout.rawValue) } }
        return answer.map { $0 != 0 } ?? false
    }

    /// ``getTimestamp(_:)`` for a `uuid` held in `layout`'s byte order — in
    /// ``UuidLayout/sqlServer``, the timestamp of a value straight from `v7ToSqlOrder` or
    /// `v6ToSqlOrder` (or a `uniqueidentifier` column), read from its permuted bytes in one
    /// native call with no conversion back first. `nil` when the bytes don't form an RFC
    /// 9562 version 6 or 7 UUID in that layout; the variant is checked in either layout (see
    /// ``version(_:layout:)`` for how SQL-ordered v6 and v7 are told apart, and why the layout
    /// is the caller's to track).
    public static func getTimestamp(_ uuid: UUID, layout: UuidLayout) throws -> Date? {
        let l = try loaded()
        var millis: UInt64 = 0
        let version = withBytes(of: uuid) { l.getTimestamp($0, layout.rawValue, &millis) }
        return version == 0 ? nil : date(unixMillis: millis)
    }

    /// ``v7UnixMillis(_:)`` for a `uuid` held in `layout`'s byte order, reading a SQL-ordered
    /// value's permuted bytes directly. Meaningful only for a genuine version 7 UUID in that
    /// layout; ``isRfc(_:version:layout:)`` is the check.
    public static func v7UnixMillis(_ uuid: UUID, layout: UuidLayout) throws -> UInt64 {
        let l = try loaded()
        return withBytes(of: uuid) { l.v7UnixMillisIn($0, layout.rawValue) }
    }

    /// ``v7Timestamp(_:)`` for a `uuid` held in `layout`'s byte order; see
    /// ``v7UnixMillis(_:layout:)``.
    public static func v7Timestamp(_ uuid: UUID, layout: UuidLayout) throws -> Date {
        date(unixMillis: try v7UnixMillis(uuid, layout: layout))
    }

    /// ``v6UnixMillis(_:)`` for a `uuid` held in `layout`'s byte order, reading a SQL-ordered
    /// value's permuted bytes directly. Meaningful only for a genuine version 6 UUID in that
    /// layout; ``isRfc(_:version:layout:)`` is the check.
    public static func v6UnixMillis(_ uuid: UUID, layout: UuidLayout) throws -> UInt64 {
        let l = try loaded()
        return withBytes(of: uuid) { l.v6UnixMillisIn($0, layout.rawValue) }
    }

    /// ``v6Timestamp(_:)`` for a `uuid` held in `layout`'s byte order; see
    /// ``v6UnixMillis(_:layout:)``.
    public static func v6Timestamp(_ uuid: UUID, layout: UuidLayout) throws -> Date {
        date(unixMillis: try v6UnixMillis(uuid, layout: layout))
    }

    private static func date(unixMillis: UInt64) -> Date {
        Date(timeIntervalSince1970: Double(unixMillis) / 1000)
    }

    // A version the nibble can hold, as the core's u32; nil (never a match) otherwise.
    private static func versionCode(_ version: Int) -> UInt32? {
        (0...15).contains(version) ? UInt32(version) : nil
    }

    private static func variantOf(_ code: UInt32) -> UuidVariant {
        guard let variant = UuidVariant(rawValue: code) else {
            preconditionFailure("hyperuuid: the native core returned variant code \(code)")
        }
        return variant
    }

    private static func withSingleUuid<T>(
        _ uuid: UnsafeRawBufferPointer, _ body: (UnsafePointer<UInt8>) -> T
    ) throws -> T {
        guard uuid.count == 16, let base = uuid.baseAddress else {
            throw Error.bufferNotWholeUUIDs(count: uuid.count)
        }
        return body(base.assumingMemoryBound(to: UInt8.self))
    }

    // MARK: - The native library itself

    /// The version of the native `libhyperuuid` linked into this executable, as
    /// `major.minor.patch` — the library's own answer (`hyperuuid_version`), not this
    /// package's tag — so a caller can prove the two agree before minting the first UUID
    /// and name the mismatch when they don't. Never throws in practice; `throws` is kept
    /// from when macOS and Windows loaded a shared library at run time.
    public static func nativeVersion() throws -> String {
        let packed = try loaded().version()
        return "\(packed >> 16).\(packed >> 8 & 0xFF).\(packed & 0xFF)"
    }
}
