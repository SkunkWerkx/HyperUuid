import Foundation
import HyperUuidCore

/// RFC 9562 UUID generation (v4 random, v5 deterministic, v7 time-sortable, and v7 converted
/// to a version 8 SQL Server order) calling directly into the native `hyperuuid` core through
/// `@convention(c)` function pointers — no runtime bridge, no cgo-style shim.
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
        /// A SQL Server order conversion was handed a value of the wrong version: going to SQL
        /// order, anything but an RFC 9562 version 7; coming back, anything but a SQL-ordered
        /// version 7 (an RFC-ordered value included).
        case wrongVersion
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
            case .wrongVersion:
                return "hyperuuid: the value is not a UUID of the version this conversion takes"
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
    private typealias UuidNewV7Fn = @convention(c) (UInt64, UnsafeMutablePointer<UInt8>?) -> Int32
    private typealias UuidV7UnixMillisFn = @convention(c) (UnsafePointer<UInt8>?) -> UInt64
    private typealias UuidNewV7BatchFn = @convention(c) (UInt64, UInt32, UnsafeMutablePointer<UInt8>?) -> Int32
    private typealias UuidV7ToSqlOrderFn = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Int32
    private typealias UuidV7ToRfcOrderFn = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Int32
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
        let newV7: UuidNewV7Fn
        let v7UnixMillis: UuidV7UnixMillisFn
        let newV7Batch: UuidNewV7BatchFn
        let v7ToSqlOrder: UuidV7ToSqlOrderFn
        let v7ToRfcOrder: UuidV7ToRfcOrderFn
        let version: VersionFn
        let uuidVersion: UuidVersionFn
        let uuidVariant: UuidVariantFn
        let uuidIsRfc: UuidIsRfcFn
        let v7UnixMillisIn: UuidUnixMillisInFn
        let getTimestamp: UuidGetTimestampFn

        init(
            newV4: UuidNewV4Fn, newV5: UuidNewV5Fn,
            newV7: UuidNewV7Fn, v7UnixMillis: UuidV7UnixMillisFn, newV7Batch: UuidNewV7BatchFn,
            v7ToSqlOrder: UuidV7ToSqlOrderFn, v7ToRfcOrder: UuidV7ToRfcOrderFn,
            version: VersionFn,
            uuidVersion: UuidVersionFn, uuidVariant: UuidVariantFn, uuidIsRfc: UuidIsRfcFn,
            v7UnixMillisIn: UuidUnixMillisInFn, getTimestamp: UuidGetTimestampFn
        ) {
            self.newV4 = newV4; self.newV5 = newV5
            self.newV7 = newV7; self.v7UnixMillis = v7UnixMillis; self.newV7Batch = newV7Batch
            self.v7ToSqlOrder = v7ToSqlOrder; self.v7ToRfcOrder = v7ToRfcOrder
            self.version = version
            self.uuidVersion = uuidVersion; self.uuidVariant = uuidVariant; self.uuidIsRfc = uuidIsRfc
            self.v7UnixMillisIn = v7UnixMillisIn; self.getTimestamp = getTimestamp
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

    /// The conversion over a copy of the value's own bytes, returned as a UUID.
    private static func reorder(_ uuid: UUID, _ fn: UuidV7ToSqlOrderFn) throws -> UUID {
        var bytes = uuid.uuid
        let rc = withUnsafeMutablePointer(to: &bytes) {
            fn(UnsafeMutableRawPointer($0).assumingMemoryBound(to: UInt8.self))
        }
        try checkSqlOrder(rc)
        return UUID(uuid: bytes)
    }

    /// A SQL Server order conversion's return code as this binding's error. The core's one
    /// refusal is the wrong version (5), and it leaves the bytes as they were.
    private static func checkSqlOrder(_ rc: Int32) throws {
        switch rc {
        case 0: return
        case 5: throw Error.wrongVersion
        default: preconditionFailure("hyperuuid: the native core returned SQL order code \(rc)")
        }
    }

    // Linked in: the C declarations are the table, and there is nothing to find or open.
    private static let library = LoadedLibrary(
        newV4: uuid_new_v4, newV5: uuid_new_v5,
        newV7: uuid_new_v7, v7UnixMillis: uuid_v7_unix_millis, newV7Batch: uuid_new_v7_batch,
        v7ToSqlOrder: uuid_v7_to_sql_order, v7ToRfcOrder: uuid_v7_to_rfc_order,
        version: hyperuuid_version,
        uuidVersion: uuid_version, uuidVariant: uuid_variant, uuidIsRfc: uuid_is_rfc,
        v7UnixMillisIn: uuid_v7_unix_millis_in, getTimestamp: uuid_get_timestamp)

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
    /// isn't an RFC 9562 version 7 UUID. Unlike `v7Timestamp`, this checks the variant as well
    /// as the version, in one native call, so a caller doesn't need to already know (or
    /// separately check) what `uuid` is before asking — a 7 nibble under another variant has no
    /// RFC version and so no timestamp. A version 6 or version 1 from another generator is
    /// `nil` too: this library reads only version 7 timestamps. No bit-layout logic here.
    /// Every 48-bit v7 timestamp has a `Date`, up to 2^48 − 1 ms (year 10889).
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
    /// before anything is allocated or written.
    ///
    /// The roll-forward orders one batch, not the stream. The next batch or `newV7` call in the
    /// same real millisecond starts its counter just past the wrap and carries the supplied
    /// timestamp, so it sorts before the previous batch's tail, stamped a millisecond later;
    /// two single calls either side of the wrap in one millisecond sort in reverse the same
    /// way. It happens at most once per ``maxV7Batch`` UUIDs the process mints.
    public static let maxV7Batch = 1 << 26

    /// Creates `count` time-sortable version 7 UUIDs sharing one Unix-epoch millisecond
    /// timestamp capture and one contiguous block of the monotonic counter — one native call
    /// and one random-bytes fetch instead of `count` of each. The batch is strictly
    /// increasing; if it crosses the counter's wrap, the UUIDs from there on carry
    /// `unixMillis + 1`. That orders this batch, not the stream: a batch or `newV7` call in
    /// the same real millisecond after it can sort before its tail (see ``maxV7Batch``).
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

    /// Converts an RFC 9562 version 7 `uuid` to SQL Server order: the RFC 9562 version 8
    /// (custom) layout that a `uniqueidentifier` column sorts by creation.
    ///
    /// `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a
    /// `uniqueidentifier` column — doesn't compare a GUID's 16 bytes left to right; it uses a
    /// fixed significance order (octets `10,11,12,13,14,15, 8,9, 6,7, 4,5, 0,1,2,3`, most
    /// significant first). The result carries version 8 at octet 7's top nibble and the RFC
    /// variant at octet 8's top bits, where SQL Server's text form and a mixed-endian GUID show
    /// them, and fills the octets around them, most significant first, with the version 7's
    /// other 122 bits: the 48-bit millisecond timestamp, the 26-bit counter, and the 48 random
    /// bits as one block. Converted values sort by creation. The layout is computed in the
    /// native Rust core, and verified there — and against the real
    /// `System.Data.SqlTypes.SqlGuid` comparator in this project's C# test suite.
    ///
    /// The conversion is exact, and ``v7FromSqlOrder(_:)`` inverts it. Every version 7
    /// converts; it throws ``Error/wrongVersion`` unless `uuid` is an RFC 9562 version 7.
    public static func v7ToSqlOrder(_ uuid: UUID) throws -> UUID {
        try reorder(uuid, try loaded().v7ToSqlOrder)
    }

    /// Inverse of ``v7ToSqlOrder(_:)`` — converts a SQL-ordered version 7 back to that RFC 9562
    /// version 7. Throws ``Error/wrongVersion`` for anything else, including an RFC-ordered
    /// value.
    public static func v7FromSqlOrder(_ uuid: UUID) throws -> UUID {
        try reorder(uuid, try loaded().v7ToRfcOrder)
    }

    // MARK: - Destination-buffer fills

    /// Fills `destination` with time-sortable version 7 UUIDs sharing one `unixMillis`
    /// timestamp capture and one contiguous block of the monotonic counter.
    ///
    /// `newV7Batch(count:)` allocates a fresh array on every call; this writes into storage
    /// the caller already owns, which is what lets a hot path reuse one buffer across batches.
    /// `destination` is raw RFC 9562-ordered bytes, 16 per UUID, and its length must be a whole
    /// multiple of 16. The UUIDs are strictly increasing, with the same possible
    /// `unixMillis + 1` as ``newV7Batch(count:unixMillis:)``, which orders this fill, not the
    /// stream (see ``maxV7Batch``); more than ``maxV7Batch`` of them
    /// throws ``Error/batchTooLarge(count:)`` with nothing written.
    public static func fillV7(into destination: UnsafeMutableRawBufferPointer, unixMillis: UInt64) throws {
        try fill(into: destination, unixMillis: unixMillis, limit: maxV7Batch) { l in l.newV7Batch }
    }

    /// Fills `destination` with version 7 UUIDs sharing the current time.
    public static func fillV7(into destination: UnsafeMutableRawBufferPointer) throws {
        try fillV7(into: destination, unixMillis: unixMillis(of: Date()))
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

    // `limit` is the v7 counter space, checked here before the native call so the core's
    // own refusal (code 4) is a backstop. The count must also fit the call's UInt32, which a
    // conversion would otherwise trap on.
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

    /// ``v7ToSqlOrder(_:)`` over 16 raw RFC 9562-ordered bytes, in place. Throws
    /// ``Error/bufferNotWholeUUIDs(count:)`` unless `uuid` is exactly 16 bytes, and the same
    /// refusals as ``v7ToSqlOrder(_:)``, leaving the bytes as they were.
    public static func v7ToSqlOrder(bytes uuid: UnsafeMutableRawBufferPointer) throws {
        try sqlOrder(uuid) { l in l.v7ToSqlOrder }
    }

    /// Inverse of ``v7ToSqlOrder(bytes:)``, in place, with ``v7FromSqlOrder(_:)``'s refusals;
    /// a refused value's bytes are left as they were.
    public static func v7FromSqlOrder(bytes uuid: UnsafeMutableRawBufferPointer) throws {
        try sqlOrder(uuid) { l in l.v7ToRfcOrder }
    }

    private static func sqlOrder(
        _ uuid: UnsafeMutableRawBufferPointer,
        _ pick: (LoadedLibrary) -> UuidV7ToSqlOrderFn
    ) throws {
        guard uuid.count == 16 else { throw Error.bufferNotWholeUUIDs(count: uuid.count) }
        let l = try loaded()
        try checkSqlOrder(pick(l)(uuid.baseAddress?.assumingMemoryBound(to: UInt8.self)))
    }

    // MARK: - Inspection and layout-aware timestamps
    //
    // The layout knowledge lives in the core: these only hand it the value's own 16 bytes and
    // the layout's code. A SQL-ordered UUID is the one `v7ToSqlOrder` returns,
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
    /// ``UuidLayout/sqlServer`` answers the version a SQL-ordered value was converted from: 7
    /// when the bytes carry the layout's version 8 and variant, else 0. The layout is the
    /// caller's to know: an RFC-ordered UUID's bytes can genuinely look SQL-ordered (a random
    /// v4 does one time in 16, octet 7's top nibble reading 8), so reading one as
    /// ``UuidLayout/sqlServer`` can answer 7.
    ///
    /// With the default layout this reads `uuid` as RFC 9562 order. A value from
    /// `v7ToSqlOrder`, or read back from a `uniqueidentifier` column, needs
    /// `layout: .sqlServer`: the default reads the wrong bytes there. There is deliberately no
    /// layout-agnostic form: the caller holding the value knows its order.
    public static func version(_ uuid: UUID, layout: UuidLayout = .rfc9562) throws -> Int {
        let l = try loaded()
        return Int(withBytes(of: uuid) { l.uuidVersion($0, layout.rawValue) })
    }

    /// ``version(_:layout:)`` over 16 raw bytes already in `layout`'s order (RFC 9562 network
    /// order by default). Throws ``Error/bufferNotWholeUUIDs(count:)`` unless `uuid` is
    /// exactly 16 bytes. With the default layout the bytes are read as RFC 9562 order; SQL
    /// Server wire bytes need `layout: .sqlServer`.
    public static func version(bytes uuid: UnsafeRawBufferPointer, layout: UuidLayout = .rfc9562) throws -> Int {
        let l = try loaded()
        return Int(try withSingleUuid(uuid) { l.uuidVersion($0, layout.rawValue) })
    }

    /// The variant field of an RFC 9562-ordered `uuid` (RFC 9562 §4.1): ``UuidVariant/ncs``
    /// for Nil, ``UuidVariant/future`` for Max, and ``UuidVariant/rfc9562`` for anything this
    /// library or Foundation's `UUID()` mints.
    ///
    /// RFC 9562 order only, and there is no layout form. A `uniqueidentifier` read-back or a
    /// `v7ToSqlOrder` result keeps the variant where RFC order does, but to
    /// validate one of those use ``isRfc(_:version:layout:)`` with `layout: .sqlServer`, which
    /// checks the SQL layout's version and variant together.
    public static func variant(_ uuid: UUID) throws -> UuidVariant {
        let l = try loaded()
        return variantOf(withBytes(of: uuid) { l.uuidVariant($0) })
    }

    /// ``variant(_:)`` over 16 raw RFC 9562-ordered bytes, never SQL Server wire bytes (see
    /// ``variant(_:)``). Throws ``Error/bufferNotWholeUUIDs(count:)`` unless `uuid` is exactly
    /// 16 bytes.
    public static func variant(bytes uuid: UnsafeRawBufferPointer) throws -> UuidVariant {
        let l = try loaded()
        return variantOf(try withSingleUuid(uuid) { l.uuidVariant($0) })
    }

    /// Whether a `uuid` held in `layout`'s byte order is an RFC 9562 UUID of version
    /// `version` — the RFC variant and that version, in one native call. The guard to run
    /// before trusting a value's version-specific fields, such as a version 7's timestamp. In
    /// ``UuidLayout/sqlServer`` only version 7 can be `true` (see
    /// ``version(_:layout:)``). A `version` outside 0–15 is simply never matched.
    ///
    /// With the default layout this reads `uuid` as RFC 9562 order. A SQL-ordered value — from
    /// `v7ToSqlOrder`, or read back from a `uniqueidentifier` column — needs
    /// `layout: .sqlServer`: the default reads the wrong bytes there and answers for whatever
    /// they happen to hold. There is deliberately no layout-agnostic form: the caller holding
    /// the value knows its order, and only the layout it names says which bytes to read.
    public static func isRfc(_ uuid: UUID, version: Int, layout: UuidLayout = .rfc9562) throws -> Bool {
        let l = try loaded()
        guard let code = versionCode(version) else { return false }
        return withBytes(of: uuid) { l.uuidIsRfc($0, code, layout.rawValue) } != 0
    }

    /// ``isRfc(_:version:layout:)`` over 16 raw bytes already in `layout`'s order (RFC 9562
    /// network order by default). Throws ``Error/bufferNotWholeUUIDs(count:)`` unless `uuid`
    /// is exactly 16 bytes. With the default layout the bytes are read as RFC 9562 order; SQL
    /// Server wire bytes need `layout: .sqlServer`.
    public static func isRfc(
        bytes uuid: UnsafeRawBufferPointer, version: Int, layout: UuidLayout = .rfc9562
    ) throws -> Bool {
        let l = try loaded()
        let answer = try withSingleUuid(uuid) { p in versionCode(version).map { l.uuidIsRfc(p, $0, layout.rawValue) } }
        return answer.map { $0 != 0 } ?? false
    }

    /// ``getTimestamp(_:)`` for a `uuid` held in `layout`'s byte order — in
    /// ``UuidLayout/sqlServer``, the timestamp of a value straight from `v7ToSqlOrder` (or a
    /// `uniqueidentifier` column), read from its SQL-ordered bytes in one native call with no
    /// conversion back first. `nil` when the bytes don't form an RFC 9562 version 7 UUID in
    /// that layout; the variant is checked in either layout (see ``version(_:layout:)`` for why
    /// the layout is the caller's to track).
    public static func getTimestamp(_ uuid: UUID, layout: UuidLayout) throws -> Date? {
        let l = try loaded()
        var millis: UInt64 = 0
        let version = withBytes(of: uuid) { l.getTimestamp($0, layout.rawValue, &millis) }
        return version == 0 ? nil : date(unixMillis: millis)
    }

    /// ``v7UnixMillis(_:)`` for a `uuid` held in `layout`'s byte order, reading a SQL-ordered
    /// value's bytes directly. Meaningful only for a genuine version 7 UUID in that
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
