import Foundation
import XCTest

@testable import HyperUuid

final class UuidGeneratorTests: XCTestCase {
    /// The core crate's own `version = "..."` from `rust/Cargo.toml`, found by walking up
    /// from this file — so the version assertion follows a release bump instead of going
    /// stale on it. `nil` only under WASI, where the test module runs sandboxed with no view
    /// of the source tree.
    private static let crateVersion: String? = {
        #if os(WASI)
        return nil
        #else
        var dir = URL(fileURLWithPath: #filePath)
        while true {
            let manifest = dir.appendingPathComponent("rust/Cargo.toml")
            if let text = try? String(contentsOf: manifest, encoding: .utf8) {
                // Split on any newline, not on "\n": Swift treats "\r\n" as one Character, so a
                // CRLF checkout (Windows) would otherwise read as a single line.
                for line in text.split(whereSeparator: \.isNewline) where line.hasPrefix("version = \"") {
                    return String(line.dropFirst("version = \"".count).prefix { $0 != "\"" })
                }
                fatalError("no version = \"...\" line in \(manifest.path)")
            }
            let parent = dir.deletingLastPathComponent()
            if parent.path == dir.path { break }
            dir = parent
        }
        fatalError("rust/Cargo.toml not found above \(#filePath)")
        #endif
    }()

    // MARK: - The native library itself

    func testNativeVersionIsTheLoadedLibrarysOwn() throws {
        let version = try UuidGenerator.nativeVersion()
        if let crateVersion = Self.crateVersion {
            XCTAssertEqual(version, crateVersion)
        } else {
            let fields = version.split(separator: ".", omittingEmptySubsequences: false)
            XCTAssertEqual(fields.count, 3, "expected major.minor.patch, got \(version)")
            XCTAssertTrue(fields.allSatisfy { UInt8($0) != nil }, "expected major.minor.patch, got \(version)")
        }
    }

    func testIsAvailableAgreesWithTheLoad() throws {
        XCTAssertTrue(UuidGenerator.isAvailable)
        XCTAssertNoThrow(try UuidGenerator.nativeVersion())
    }

    func testTheNativeCoreComesFromWhereADeployedBinaryHasIt() throws {
        #if os(Linux) || os(WASI)
        // Linked into the executable: nothing to find, so nothing to leave behind.
        XCTAssertEqual(try UuidGenerator.nativeLibraryOrigin(), .staticallyLinked)
        #else
        // The resource directory is the only place a deployed binary has. The source-tree
        // fallback would keep every other test here green on the build machine even if
        // the bundle lookup stopped working, so the origin is pinned on its own.
        XCTAssertEqual(try UuidGenerator.nativeLibraryOrigin(), .resourceBundle)
        #endif
    }

    // The two ways a load can fail exist only where there is a load: macOS and Windows.
    #if os(macOS) || os(Windows)
    func testAMissingLibraryIsANativeLibraryErrorACallerCanMatch() {
        let missing = "/nonexistent/\(NativePlatform.libraryFileName)"
        XCTAssertThrowsError(try DynamicLibrary(path: missing)) { error in
            guard case NativeLibraryError.openFailed(let path, _) = error else {
                XCTFail("expected NativeLibraryError.openFailed, got \(error)")
                return
            }
            XCTAssertEqual(path, missing)
            // LocalizedError, so the one-liner survives `localizedDescription` too.
            XCTAssertEqual(error.localizedDescription, "\(error)")
            XCTAssertTrue(error.localizedDescription.hasPrefix("hyperuuid: failed to load native library at \(missing)"))
        }
    }

    func testAMissingExportIsANativeLibraryErrorNamingTheSymbol() throws {
        let library = try DynamicLibrary(path: try DynamicLibrary.locateBundled().path)
        XCTAssertThrowsError(try library.symbol("uuid_no_such_export")) { error in
            guard case NativeLibraryError.symbolNotFound(let name) = error else {
                XCTFail("expected NativeLibraryError.symbolNotFound, got \(error)")
                return
            }
            XCTAssertEqual(name, "uuid_no_such_export")
        }
    }
    #endif

    func testGeneratorErrorsDescribeThemselvesThroughLocalizedDescription() {
        let error: Swift.Error = UuidGenerator.Error.bufferNotWholeUUIDs(count: 17)
        XCTAssertEqual(error.localizedDescription, "\(error)")
        XCTAssertTrue(error.localizedDescription.hasSuffix("got 17"))
    }

    func testV4HasVersionAndVariantBits() throws {
        let id = try UuidGenerator.newV4()
        let bytes = id.rfcBytes
        XCTAssertEqual(bytes[6] >> 4, 4)
        XCTAssertEqual(bytes[8] >> 6, 0b10)
    }

    func testV4IsNonDeterministic() throws {
        var seen = Set<UUID>()
        for _ in 0..<100 {
            seen.insert(try UuidGenerator.newV4())
        }
        XCTAssertEqual(seen.count, 100)
    }

    // RFC 9562 Appendix A.4 official test vector.
    func testV5MatchesRfcTestVector() throws {
        let id = try UuidGenerator.newV5(namespace: Namespaces.dns, name: "www.example.com")
        XCTAssertEqual(id, UUID(uuidString: "2ed6657d-e927-568b-95e1-2665a8aea6a2")!)
    }

    // Python's `uuid` standard library documentation test vector.
    func testV5MatchesPythonDocsVector() throws {
        let id = try UuidGenerator.newV5(namespace: Namespaces.dns, name: "python.org")
        XCTAssertEqual(id, UUID(uuidString: "886313e1-3b8a-5372-9b90-0c9aee199e5d")!)
    }

    func testV5IsDeterministic() throws {
        let a = try UuidGenerator.newV5(namespace: Namespaces.dns, name: "same-name")
        let b = try UuidGenerator.newV5(namespace: Namespaces.dns, name: "same-name")
        XCTAssertEqual(a, b)
    }

    func testV5DifferentNamespacesDiffer() throws {
        let dns = try UuidGenerator.newV5(namespace: Namespaces.dns, name: "test")
        let url = try UuidGenerator.newV5(namespace: Namespaces.url, name: "test")
        XCTAssertNotEqual(dns, url)
    }

    func testV5BytesAndRawBufferFormsMatchTheStringForm() throws {
        let want = UUID(uuidString: "2ed6657d-e927-568b-95e1-2665a8aea6a2")!
        let name = Array("www.example.com".utf8)
        XCTAssertEqual(try UuidGenerator.newV5(namespace: Namespaces.dns, name: name), want)
        try name.withUnsafeBytes { raw in
            XCTAssertEqual(try UuidGenerator.newV5(namespace: Namespaces.dns, name: raw), want)
        }
    }

    func testV5HashesNameBytesThatAreNotUtf8() throws {
        // SHA-1 over namespace || 0x00 0xFF 0x80, computed independently with Python's hashlib:
        // the byte forms take any bytes, not only what a String can spell.
        let name: [UInt8] = [0x00, 0xFF, 0x80]
        XCTAssertEqual(
            try UuidGenerator.newV5(namespace: Namespaces.url, name: name),
            UUID(uuidString: "37bcb657-8d00-59e4-8295-d5960041c2f3")!)
    }

    func testV5EmptyNameAgreesAcrossEveryForm() throws {
        // Python: uuid.uuid5(uuid.NAMESPACE_DNS, "").
        let want = UUID(uuidString: "4ebd0208-8328-5d69-8c44-ec50939c0967")!
        XCTAssertEqual(try UuidGenerator.newV5(namespace: Namespaces.dns, name: ""), want)
        XCTAssertEqual(try UuidGenerator.newV5(namespace: Namespaces.dns, name: [UInt8]()), want)
        // A zero-length buffer with no base address at all — the native core never
        // dereferences the name pointer at length 0.
        let nilBase = UnsafeRawBufferPointer(start: nil, count: 0)
        XCTAssertEqual(try UuidGenerator.newV5(namespace: Namespaces.dns, name: nilBase), want)
    }

    // RFC 9562 Appendix A.6: 2022-02-22T19:22:22Z = 1645557742000 ms since epoch.
    let rfcTestVectorMs: UInt64 = 1_645_557_742_000

    func testV6EmbedsTheTimestamp() throws {
        let id = try UuidGenerator.newV6(unixMillis: rfcTestVectorMs)
        let timestamp = try UuidGenerator.v6Timestamp(id)
        XCTAssertEqual(timestamp.timeIntervalSince1970, Double(rfcTestVectorMs) / 1000, accuracy: 0.0001)
    }

    func testV6HasVersionAndVariantBits() throws {
        let id = try UuidGenerator.newV6(unixMillis: rfcTestVectorMs)
        let bytes = id.rfcBytes
        XCTAssertEqual(bytes[6] >> 4, 6)
        XCTAssertEqual(bytes[8] >> 6, 0b10)
    }

    func testV6SetsTheNodeIdMulticastBit() throws {
        let id = try UuidGenerator.newV6(unixMillis: rfcTestVectorMs)
        XCTAssertEqual(id.rfcBytes[10] & 0x01, 0x01)
    }

    func testV6IsNonDeterministicWithinTheSameMillisecond() throws {
        var seen = Set<UUID>()
        for _ in 0..<100 {
            seen.insert(try UuidGenerator.newV6(unixMillis: rfcTestVectorMs))
        }
        XCTAssertEqual(seen.count, 100)
    }

    func testV6BatchReturnsCountUuidsSharingTheTimestamp() throws {
        let ids = try UuidGenerator.newV6Batch(count: 10, unixMillis: rfcTestVectorMs)
        XCTAssertEqual(ids.count, 10)
        for id in ids {
            let timestamp = try UuidGenerator.v6Timestamp(id)
            XCTAssertEqual(timestamp.timeIntervalSince1970, Double(rfcTestVectorMs) / 1000, accuracy: 0.0001)
        }
    }

    func testV6BatchProducesPairwiseDistinctUuids() throws {
        let ids = try UuidGenerator.newV6Batch(count: 100, unixMillis: rfcTestVectorMs)
        XCTAssertEqual(Set(ids).count, 100)
    }

    func testV6BatchCountZeroReturnsEmptyArray() throws {
        XCTAssertEqual(try UuidGenerator.newV6Batch(count: 0, unixMillis: rfcTestVectorMs), [])
    }

    func testV6BatchOverflowTimestampThrows() {
        XCTAssertThrowsError(try UuidGenerator.newV6Batch(count: 1, unixMillis: UInt64.max)) { error in
            guard case UuidGenerator.Error.timestampOutOfRange = error else {
                XCTFail("expected timestampOutOfRange, got \(error)")
                return
            }
        }
    }

    func testNilAndMaxUUIDs() {
        XCTAssertEqual(WellKnownUuids.nilUUID.uuidString, "00000000-0000-0000-0000-000000000000")
        XCTAssertEqual(WellKnownUuids.maxUUID.uuidString, "FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF")
    }

    func testV7EmbedsTheTimestamp() throws {
        let id = try UuidGenerator.newV7(unixMillis: rfcTestVectorMs)
        let bytes = id.rfcBytes
        var embeddedMs: UInt64 = 0
        for i in 0..<6 {
            embeddedMs = (embeddedMs << 8) | UInt64(bytes[i])
        }
        XCTAssertEqual(embeddedMs, rfcTestVectorMs)
    }

    func testV7HasVersionAndVariantBits() throws {
        let id = try UuidGenerator.newV7(unixMillis: rfcTestVectorMs)
        let bytes = id.rfcBytes
        XCTAssertEqual(bytes[6] >> 4, 7)
        XCTAssertEqual(bytes[8] >> 6, 0b10)
    }

    func testV7OverflowTimestampThrows() {
        XCTAssertThrowsError(try UuidGenerator.newV7(unixMillis: 0x0001_0000_0000_0000)) { error in
            guard case UuidGenerator.Error.timestampOutOfRange = error else {
                XCTFail("expected timestampOutOfRange, got \(error)")
                return
            }
        }
    }

    func testV7SameMillisecondBatchIsMonotonicallyOrdered() throws {
        let ids = try (0..<100).map { _ in try UuidGenerator.newV7(unixMillis: rfcTestVectorMs) }
        XCTAssertEqual(ids, ids.sorted { $0.uuidString < $1.uuidString })
    }

    func testV7CurrentTimestampIsEmbedded() throws {
        let before = UInt64(Date().timeIntervalSince1970 * 1000)
        let id = try UuidGenerator.newV7()
        let after = UInt64(Date().timeIntervalSince1970 * 1000)

        let bytes = id.rfcBytes
        var embeddedMs: UInt64 = 0
        for i in 0..<6 {
            embeddedMs = (embeddedMs << 8) | UInt64(bytes[i])
        }
        XCTAssertTrue(embeddedMs >= before && embeddedMs <= after)
    }

    func testV7TimestampRecoversTheExactMillisecond() throws {
        let id = try UuidGenerator.newV7(unixMillis: rfcTestVectorMs)
        let timestamp = try UuidGenerator.v7Timestamp(id)
        XCTAssertEqual(timestamp.timeIntervalSince1970, Double(rfcTestVectorMs) / 1000, accuracy: 0.0001)
    }

    func testV7TimestampRoundTripsZeroAndTheRfc48BitMax() throws {
        let zero = try UuidGenerator.newV7(unixMillis: 0)
        XCTAssertEqual(try UuidGenerator.v7UnixMillis(zero), 0)

        let maxMs: UInt64 = 0x0000_FFFF_FFFF_FFFF
        let id = try UuidGenerator.newV7(unixMillis: maxMs)
        XCTAssertEqual(try UuidGenerator.v7UnixMillis(id), maxMs)
    }

    func testNewV6FromDateMatchesNewV6FromTheEquivalentMillis() throws {
        let date = Date(timeIntervalSince1970: Double(rfcTestVectorMs) / 1000)
        let id = try UuidGenerator.newV6(date)
        XCTAssertEqual(try UuidGenerator.v6UnixMillis(id), rfcTestVectorMs)
    }

    func testNewV7FromDateMatchesNewV7FromTheEquivalentMillis() throws {
        let date = Date(timeIntervalSince1970: Double(rfcTestVectorMs) / 1000)
        let id = try UuidGenerator.newV7(date)
        XCTAssertEqual(try UuidGenerator.v7UnixMillis(id), rfcTestVectorMs)
    }

    /// Dates `UInt64(_:)` would trap on, plus two the conversion survives but the timestamp
    /// field can't hold: every one is `timestampOutOfRange`, never a crash.
    private static let unrepresentableDates: [(String, Date)] = [
        ("one second before the epoch", Date(timeIntervalSince1970: -1)),
        ("half a millisecond before the epoch", Date(timeIntervalSince1970: -0.0005)),
        ("distantPast", Date.distantPast),
        ("NaN", Date(timeIntervalSince1970: .nan)),
        ("+infinity", Date(timeIntervalSince1970: .infinity)),
        ("-infinity", Date(timeIntervalSince1970: -.infinity)),
        ("past UInt64 milliseconds", Date(timeIntervalSince1970: 1e300)),
        ("inside UInt64, past the timestamp field", Date(timeIntervalSince1970: 1e16)),
    ]

    private func assertTimestampOutOfRange(
        _ label: String, _ body: @autoclosure () throws -> UUID,
        file: StaticString = #filePath, line: UInt = #line
    ) {
        XCTAssertThrowsError(try body(), label, file: file, line: line) { error in
            guard case UuidGenerator.Error.timestampOutOfRange = error else {
                XCTFail("\(label): expected timestampOutOfRange, got \(error)", file: file, line: line)
                return
            }
        }
    }

    func testNewV6FromAnUnrepresentableDateThrowsInsteadOfTrapping() {
        for (label, date) in Self.unrepresentableDates {
            assertTimestampOutOfRange(label, try UuidGenerator.newV6(date))
        }
    }

    func testNewV7FromAnUnrepresentableDateThrowsInsteadOfTrapping() {
        for (label, date) in Self.unrepresentableDates {
            assertTimestampOutOfRange(label, try UuidGenerator.newV7(date))
        }
    }

    func testNewV7FromTheEpochItselfIsMillisecondZero() throws {
        let id = try UuidGenerator.newV7(Date(timeIntervalSince1970: 0))
        XCTAssertEqual(try UuidGenerator.v7UnixMillis(id), 0)
    }

    func testGetTimestampReturnsNilForNonTimeBasedVersions() throws {
        let v4 = try UuidGenerator.newV4()
        XCTAssertNil(try UuidGenerator.getTimestamp(v4))
        let v5 = try UuidGenerator.newV5(namespace: Namespaces.dns, name: "test")
        XCTAssertNil(try UuidGenerator.getTimestamp(v5))
    }

    func testGetTimestampMatchesV6Timestamp() throws {
        let id = try UuidGenerator.newV6(unixMillis: rfcTestVectorMs)
        XCTAssertEqual(try UuidGenerator.getTimestamp(id), try UuidGenerator.v6Timestamp(id))
    }

    func testGetTimestampMatchesV7Timestamp() throws {
        let id = try UuidGenerator.newV7(unixMillis: rfcTestVectorMs)
        XCTAssertEqual(try UuidGenerator.getTimestamp(id), try UuidGenerator.v7Timestamp(id))
    }

    func testV7BatchReturnsCountUuidsSortedAndSharingTheTimestamp() throws {
        let ids = try UuidGenerator.newV7Batch(count: 1000, unixMillis: rfcTestVectorMs)
        XCTAssertEqual(ids.count, 1000)
        XCTAssertEqual(ids.map(\.uuidString), ids.map(\.uuidString).sorted())
        for id in ids {
            let timestamp = try UuidGenerator.v7Timestamp(id)
            XCTAssertEqual(timestamp.timeIntervalSince1970, Double(rfcTestVectorMs) / 1000, accuracy: 0.0001)
        }
    }

    func testV7BatchContinuesTheSameCounterSequenceAsIndividualCalls() throws {
        let before = try UuidGenerator.newV7(unixMillis: rfcTestVectorMs)
        let batch = try UuidGenerator.newV7Batch(count: 10, unixMillis: rfcTestVectorMs)
        let after = try UuidGenerator.newV7(unixMillis: rfcTestVectorMs)

        let ids = [before] + batch + [after]
        XCTAssertEqual(ids.map(\.uuidString), ids.map(\.uuidString).sorted())
    }

    func testV7BatchCountZeroReturnsEmptyArray() throws {
        XCTAssertEqual(try UuidGenerator.newV7Batch(count: 0, unixMillis: rfcTestVectorMs), [])
    }

    func testV7BatchOverflowTimestampThrows() {
        XCTAssertThrowsError(try UuidGenerator.newV7Batch(count: 1, unixMillis: 0x0001_0000_0000_0000)) { error in
            guard case UuidGenerator.Error.timestampOutOfRange = error else {
                XCTFail("expected timestampOutOfRange, got \(error)")
                return
            }
        }
    }

    func testV7ToSqlOrderRoundTripsThroughV7FromSqlOrder() throws {
        let id = try UuidGenerator.newV7(unixMillis: rfcTestVectorMs)
        let sqlOrdered = try UuidGenerator.v7ToSqlOrder(id)
        XCTAssertNotEqual(sqlOrdered, id)
        XCTAssertEqual(try UuidGenerator.v7FromSqlOrder(sqlOrdered), id)
    }

    func testV7ToSqlOrderPreservesVersionAndVariantAtOctets7And8() throws {
        let sqlOrdered = try UuidGenerator.v7ToSqlOrder(try UuidGenerator.newV7(unixMillis: rfcTestVectorMs))
        let bytes = sqlOrdered.rfcBytes
        XCTAssertEqual(bytes[7] & 0xF0, 0x70)
        XCTAssertEqual(bytes[8] & 0xC0, 0x80)
    }

    /// Replicates `System.Data.SqlTypes.SqlGuid.CompareTo`'s fixed byte significance order —
    /// the correctness oracle this project's C# test suite checks directly against the real
    /// type; no equivalent exists in Foundation to test against here, so this stands in for it,
    /// the same role the hand-rolled comparator in the Rust core's own test suite plays.
    private func sqlGuidCompare(_ a: [UInt8], _ b: [UInt8]) -> Bool {
        let significanceOrder = [10, 11, 12, 13, 14, 15, 8, 9, 6, 7, 4, 5, 0, 1, 2, 3]
        for i in significanceOrder {
            if a[i] != b[i] { return a[i] < b[i] }
        }
        return false
    }

    func testV7ToSqlOrderSortsByCreationOrderUnderSqlGuidComparison() throws {
        var ids: [UUID] = []
        for i in UInt64(0)..<200 {
            ids.append(try UuidGenerator.newV7(unixMillis: rfcTestVectorMs + i))
        }
        // Same-millisecond run, so the counter (not just the timestamp) has to sort correctly too.
        for _ in 0..<200 {
            ids.append(try UuidGenerator.newV7(unixMillis: rfcTestVectorMs + 1_000_000))
        }

        let sqlOrdered = try ids.map { try UuidGenerator.v7ToSqlOrder($0).rfcBytes }
        let sorted = sqlOrdered.sorted(by: sqlGuidCompare)

        XCTAssertEqual(sqlOrdered.count, sorted.count)
        for (a, b) in zip(sqlOrdered, sorted) {
            XCTAssertEqual(a, b)
        }
    }

    func testV6ToSqlOrderRoundTripsThroughV6FromSqlOrder() throws {
        let id = try UuidGenerator.newV6(unixMillis: rfcTestVectorMs)
        let sqlOrdered = try UuidGenerator.v6ToSqlOrder(id)
        XCTAssertNotEqual(sqlOrdered, id)
        XCTAssertEqual(try UuidGenerator.v6FromSqlOrder(sqlOrdered), id)
    }

    func testV6ToSqlOrderPreservesVersionAndVariant() throws {
        // Different offsets than v7's sql order — see v6ToSqlOrder's doc comment for why.
        let sqlOrdered = try UuidGenerator.v6ToSqlOrder(try UuidGenerator.newV6(unixMillis: rfcTestVectorMs))
        let bytes = sqlOrdered.rfcBytes
        XCTAssertEqual(bytes[8] & 0xF0, 0x60)
        XCTAssertEqual(bytes[6] & 0xC0, 0x80)
    }

    func testV6ToSqlOrderSortsByCreationOrderUnderSqlGuidComparisonForDistinctTimestamps() throws {
        // Unlike v7, v6 has no counter — two UUIDs at the same millisecond aren't guaranteed
        // to sort in creation order even in plain RFC order, so this only exercises strictly
        // increasing timestamps, where the timestamp alone determines order with no tie to break.
        var ids: [UUID] = []
        for i in UInt64(0)..<300 {
            ids.append(try UuidGenerator.newV6(unixMillis: rfcTestVectorMs + i))
        }

        let sqlOrdered = try ids.map { try UuidGenerator.v6ToSqlOrder($0).rfcBytes }
        let sorted = sqlOrdered.sorted(by: sqlGuidCompare)

        XCTAssertEqual(sqlOrdered.count, sorted.count)
        for (a, b) in zip(sqlOrdered, sorted) {
            XCTAssertEqual(a, b)
        }
    }

    // MARK: - Destination-buffer fills

    func testFillV7IntoUUIDArrayFillsCallersStorage() throws {
        var dst = [UUID](repeating: UUID(), count: 64)
        try UuidGenerator.fillV7(into: &dst, unixMillis: rfcTestVectorMs)
        for (i, id) in dst.enumerated() {
            let b = id.rfcBytes
            XCTAssertEqual(b[6] >> 4, 7, "item \(i) version")
            XCTAssertEqual(try UuidGenerator.v7UnixMillis(id), rfcTestVectorMs, "item \(i) timestamp")
        }
    }

    func testFillV7IntoUUIDArrayIsStrictlyIncreasing() throws {
        var dst = [UUID](repeating: UUID(), count: 256)
        try UuidGenerator.fillV7(into: &dst, unixMillis: rfcTestVectorMs)
        for i in 1..<dst.count {
            XCTAssertTrue(
                dst[i - 1].rfcBytes.lexicographicallyPrecedes(dst[i].rfcBytes),
                "items \(i - 1)/\(i) not in creation order")
        }
    }

    func testFillV6IntoUUIDArray() throws {
        var dst = [UUID](repeating: UUID(), count: 32)
        try UuidGenerator.fillV6(into: &dst, unixMillis: rfcTestVectorMs)
        for (i, id) in dst.enumerated() {
            XCTAssertEqual(id.rfcBytes[6] >> 4, 6, "item \(i) version")
        }
    }

    func testFillIntoUUIDArrayWithoutATimestampUsesTheCurrentTime() throws {
        let before = UInt64(Date().timeIntervalSince1970 * 1000)
        var v7 = [UUID](repeating: UUID(), count: 8)
        try UuidGenerator.fillV7(into: &v7)
        var v6 = [UUID](repeating: UUID(), count: 8)
        try UuidGenerator.fillV6(into: &v6)
        let after = UInt64(Date().timeIntervalSince1970 * 1000)
        for id in v7 {
            XCTAssertEqual(id.rfcBytes[6] >> 4, 7)
            XCTAssertTrue((before...after).contains(try UuidGenerator.v7UnixMillis(id)))
        }
        for id in v6 {
            XCTAssertEqual(id.rfcBytes[6] >> 4, 6)
            XCTAssertTrue((before...after).contains(try UuidGenerator.v6UnixMillis(id)))
        }
    }

    func testFillV7IntoRawBufferMatchesArrayForm() throws {
        let count = 16
        var raw = [UInt8](repeating: 0, count: count * 16)
        try raw.withUnsafeMutableBytes { buf in
            try UuidGenerator.fillV7(into: buf, unixMillis: rfcTestVectorMs)
        }
        for i in 0..<count {
            let id = UUID(rfcBytes: Array(raw[(i * 16)..<(i * 16 + 16)]))
            XCTAssertEqual(id.rfcBytes[6] >> 4, 7)
            XCTAssertEqual(try UuidGenerator.v7UnixMillis(id), rfcTestVectorMs)
        }
    }

    func testFillRejectsPartialUUIDBuffer() throws {
        var raw = [UInt8](repeating: 0, count: 17)
        try raw.withUnsafeMutableBytes { buf in
            XCTAssertThrowsError(try UuidGenerator.fillV7(into: buf, unixMillis: rfcTestVectorMs))
        }
    }

    func testFillEmptyIsANoOp() throws {
        var empty = [UUID]()
        XCTAssertNoThrow(try UuidGenerator.fillV7(into: &empty, unixMillis: rfcTestVectorMs))
    }

    // MARK: - Raw-byte SQL-order transforms

    func testV7ToSqlOrderBytesAgreesWithUUIDForm() throws {
        let id = try UuidGenerator.newV7(unixMillis: rfcTestVectorMs)
        let want = try UuidGenerator.v7ToSqlOrder(id).rfcBytes
        var got = id.rfcBytes
        try got.withUnsafeMutableBytes { try UuidGenerator.v7ToSqlOrder(bytes: $0) }
        XCTAssertEqual(got, want)
    }

    func testV6ToSqlOrderBytesAgreesWithUUIDForm() throws {
        let id = try UuidGenerator.newV6(unixMillis: rfcTestVectorMs)
        let want = try UuidGenerator.v6ToSqlOrder(id).rfcBytes
        var got = id.rfcBytes
        try got.withUnsafeMutableBytes { try UuidGenerator.v6ToSqlOrder(bytes: $0) }
        XCTAssertEqual(got, want)
    }

    func testSqlOrderBytesRoundTrips() throws {
        let id = try UuidGenerator.newV7(unixMillis: rfcTestVectorMs)
        let original = id.rfcBytes
        var b = original
        try b.withUnsafeMutableBytes { try UuidGenerator.v7ToSqlOrder(bytes: $0) }
        XCTAssertNotEqual(b, original)
        try b.withUnsafeMutableBytes { try UuidGenerator.v7FromSqlOrder(bytes: $0) }
        XCTAssertEqual(b, original)
    }

    func testSqlOrderBytesRejectsWrongSize() throws {
        var b = [UInt8](repeating: 0, count: 15)
        try b.withUnsafeMutableBytes { buf in
            XCTAssertThrowsError(try UuidGenerator.v7ToSqlOrder(bytes: buf))
        }
    }
}
