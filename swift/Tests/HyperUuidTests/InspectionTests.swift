import Foundation
import XCTest

@testable import HyperUuid

/// Version/variant inspection, the layout-aware doors and the v7 batch limit against values
/// minted at run time, by this library and by Foundation; ``CorpusTests`` pins the fixed
/// vectors.
final class InspectionTests: XCTestCase {
    private let ms: UInt64 = 1_645_557_742_000

    func testFoundationAndLibraryValuesReportTheirVersionAndTheRfcVariant() throws {
        let values: [(UUID, Int)] = [
            (UUID(), 4), (try UuidGenerator.newV4(), 4),
            (try UuidGenerator.newV5(namespace: Namespaces.dns, name: "x"), 5),
            (try UuidGenerator.newV6(unixMillis: ms), 6), (try UuidGenerator.newV7(unixMillis: ms), 7),
        ]
        for (id, version) in values {
            XCTAssertEqual(try UuidGenerator.version(id), version, "\(id)")
            XCTAssertEqual(try UuidGenerator.version(id), Int(id.uuid.6 >> 4), "\(id)")  // the bits agree
            XCTAssertEqual(try UuidGenerator.variant(id), .rfc9562, "\(id)")
            XCTAssertTrue(try UuidGenerator.isRfc(id, version: version), "\(id)")
            XCTAssertFalse(try UuidGenerator.isRfc(id, version: version == 7 ? 6 : 7), "\(id)")
        }
    }

    func testNilAndMaxAreClassifiedAsTheRfcSays() throws {
        XCTAssertEqual(try UuidGenerator.version(WellKnownUuids.nilUUID), 0)
        XCTAssertEqual(try UuidGenerator.variant(WellKnownUuids.nilUUID), .ncs)
        XCTAssertEqual(try UuidGenerator.version(WellKnownUuids.maxUUID), 15)
        XCTAssertEqual(try UuidGenerator.variant(WellKnownUuids.maxUUID), .future)
        XCTAssertFalse(try UuidGenerator.isRfc(WellKnownUuids.nilUUID, version: 0))
        XCTAssertFalse(try UuidGenerator.isRfc(WellKnownUuids.maxUUID, version: 15))
    }

    func testAVersionOutsideTheNibbleNeverMatches() throws {
        let id = try UuidGenerator.newV7(unixMillis: ms)
        let bytes = id.rfcBytes
        for version in [7 + 256, 7 + (1 << 32), -1, Int.min, Int.max] {
            XCTAssertFalse(try UuidGenerator.isRfc(id, version: version), "\(version)")
            try bytes.withUnsafeBytes { raw in
                XCTAssertFalse(try UuidGenerator.isRfc(bytes: raw, version: version), "\(version)")
            }
        }
    }

    // In SQL order a v6's random clock_seq sits where a v7's version nibble does and reads as 7
    // one time in 16; enough draws that a confusion would surface.
    func testSqlOrderedV6AndV7AreValidatedAndReadInPlaceWithoutConfusion() throws {
        let date = Date(timeIntervalSince1970: Double(ms) / 1000)
        for _ in 0..<2048 {
            let six = try UuidGenerator.v6ToSqlOrder(try UuidGenerator.newV6(unixMillis: ms))
            let seven = try UuidGenerator.v7ToSqlOrder(try UuidGenerator.newV7(unixMillis: ms))

            XCTAssertEqual(try UuidGenerator.version(six, layout: .sqlServer), 6)
            XCTAssertEqual(try UuidGenerator.version(seven, layout: .sqlServer), 7)
            XCTAssertTrue(try UuidGenerator.isRfc(six, version: 6, layout: .sqlServer))
            XCTAssertFalse(try UuidGenerator.isRfc(six, version: 7, layout: .sqlServer))
            XCTAssertTrue(try UuidGenerator.isRfc(seven, version: 7, layout: .sqlServer))
            XCTAssertFalse(try UuidGenerator.isRfc(seven, version: 6, layout: .sqlServer))

            XCTAssertEqual(try UuidGenerator.v6UnixMillis(six, layout: .sqlServer), ms)
            XCTAssertEqual(try UuidGenerator.v7UnixMillis(seven, layout: .sqlServer), ms)
            XCTAssertEqual(try UuidGenerator.getTimestamp(six, layout: .sqlServer), date)
            XCTAssertEqual(try UuidGenerator.getTimestamp(seven, layout: .sqlServer), date)
            // Read straight from SQL order matches permuting back first.
            XCTAssertEqual(
                try UuidGenerator.v7UnixMillis(seven, layout: .sqlServer),
                try UuidGenerator.v7UnixMillis(try UuidGenerator.v7FromSqlOrder(seven)))
            // The raw form takes the bytes SQL Server stores, which are the value's own.
            try seven.rfcBytes.withUnsafeBytes { raw in
                XCTAssertEqual(try UuidGenerator.version(bytes: raw, layout: .sqlServer), 7)
            }
            try six.rfcBytes.withUnsafeBytes { raw in
                XCTAssertTrue(try UuidGenerator.isRfc(bytes: raw, version: 6, layout: .sqlServer))
            }
        }
    }

    // Fixed values only: RFC-order bytes can genuinely form a SQL-ordered v7 (octet 8 already
    // holds the RFC variant and a random v4's octet 7 starts with 7 one time in 16), so a
    // random UUID() here would fail one run in sixteen. The layout is the caller's to know.
    func testNonSqlValuesHaveNoSqlVersion() throws {
        let values = [
            UUID(uuidString: "919108f7-52d1-4320-9bac-f847db4148a8")!,  // RFC 9562 A.3 (v4)
            WellKnownUuids.nilUUID, WellKnownUuids.maxUUID,
            try UuidGenerator.newV5(namespace: Namespaces.dns, name: "x"),
        ]
        for id in values {
            XCTAssertEqual(try UuidGenerator.version(id, layout: .sqlServer), 0, "\(id)")
            XCTAssertNil(try UuidGenerator.getTimestamp(id, layout: .sqlServer), "\(id)")
        }
    }

    // The core's 0 ("unspecified") and any undefined code have no UuidLayout, so no door can
    // be handed one; what is left to pin is that the enum admits only the two layouts.
    func testOnlyTheTwoDefinedLayoutsExist() {
        XCTAssertNil(UuidLayout(rawValue: 0))
        XCTAssertNil(UuidLayout(rawValue: 3))
        XCTAssertEqual(UuidLayout.allCases.map(\.rawValue), [1, 2])
        XCTAssertNil(UuidVariant(rawValue: 0))
        XCTAssertEqual(UuidVariant.allCases.map(\.rawValue), [1, 2, 3, 4])
    }

    func testTheRawByteFormsRequireExactly16Bytes() throws {
        for count in [0, 15, 17, 32] {
            let bytes = [UInt8](repeating: 0, count: count)
            try bytes.withUnsafeBytes { raw in
                for (what, call) in [
                    ("version", { _ = try UuidGenerator.version(bytes: raw) }),
                    ("variant", { _ = try UuidGenerator.variant(bytes: raw) }),
                    ("isRfc", { _ = try UuidGenerator.isRfc(bytes: raw, version: 7) }),
                ] as [(String, () throws -> Void)] {
                    XCTAssertThrowsError(try call(), "\(what) over \(count) bytes") { error in
                        guard case UuidGenerator.Error.bufferNotWholeUUIDs(count) = error else {
                            return XCTFail("\(what): \(error)")
                        }
                    }
                }
            }
        }
        // A zero-length buffer with no base address at all is refused the same way.
        let nilBase = UnsafeRawBufferPointer(start: nil, count: 0)
        XCTAssertThrowsError(try UuidGenerator.version(bytes: nilBase))
    }

    // MARK: - The v7 batch limit

    private func assertBatchTooLarge(_ count: Int, _ body: () throws -> Void, line: UInt = #line) {
        XCTAssertThrowsError(try body(), line: line) { error in
            guard case UuidGenerator.Error.batchTooLarge(count) = error else {
                return XCTFail("\(error)", line: line)
            }
            XCTAssertTrue("\(error)".contains("67108864"), "\(error)", line: line)
        }
    }

    func testMaxV7BatchIsTheCounterSpace() {
        XCTAssertEqual(UuidGenerator.maxV7Batch, 67_108_864)
    }

    // The limit is checked before anything is allocated or written, so the raw buffer past
    // the limit can claim a length nothing allocated: only its first 16 bytes exist, and
    // they stay untouched.
    func testAV7BatchPastTheCounterSpaceIsRefusedOnEveryDoor() throws {
        let tooMany = UuidGenerator.maxV7Batch + 1
        assertBatchTooLarge(tooMany) { _ = try UuidGenerator.newV7Batch(count: tooMany, unixMillis: ms) }
        assertBatchTooLarge(tooMany) { _ = try UuidGenerator.newV7Batch(count: tooMany) }

        var one = [UInt8](repeating: 0, count: 16)
        try one.withUnsafeMutableBytes { storage in
            let claimed = UnsafeMutableRawBufferPointer(start: storage.baseAddress, count: tooMany * 16)
            assertBatchTooLarge(tooMany) { try UuidGenerator.fillV7(into: claimed, unixMillis: ms) }
            assertBatchTooLarge(tooMany) { try UuidGenerator.fillV7(into: claimed) }
        }
        XCTAssertEqual(one, [UInt8](repeating: 0, count: 16))
    }

    // An array can't claim a length it doesn't have, so the [UUID] door costs a real 1 GiB.
    func testAV7FillOfAUUIDArrayPastTheCounterSpaceIsRefused() throws {
        try skipUnlessGiBTestsCanRun()
        let tooMany = UuidGenerator.maxV7Batch + 1
        var ids = [UUID](repeating: WellKnownUuids.nilUUID, count: tooMany)
        assertBatchTooLarge(tooMany) { try UuidGenerator.fillV7(into: &ids, unixMillis: ms) }
        assertBatchTooLarge(tooMany) { try UuidGenerator.fillV7(into: &ids) }
        XCTAssertEqual(ids[0], WellKnownUuids.nilUUID)
        XCTAssertEqual(ids[tooMany - 1], WellKnownUuids.nilUUID)
    }

    // Exactly the counter space: 1 GiB of bytes, in strictly increasing order end to end, and
    // at most a millisecond past the supplied timestamp (the roll-forward over the wrap).
    func testAV7BatchOfExactlyTheCounterSpaceIsStrictlyIncreasing() throws {
        try skipUnlessGiBTestsCanRun()
        let count = UuidGenerator.maxV7Batch
        let buffer = UnsafeMutableRawBufferPointer.allocate(byteCount: count * 16, alignment: 16)
        defer { buffer.deallocate() }
        try UuidGenerator.fillV7(into: buffer, unixMillis: ms)
        let base = buffer.baseAddress!
        var increasing = true
        for i in 1..<count where memcmp(base + (i - 1) * 16, base + i * 16, 16) >= 0 {
            increasing = false
            XCTFail("items \(i - 1)/\(i) not strictly increasing")
            break
        }
        XCTAssertTrue(increasing)
        let last = UUID(rfcBytes: Array(UnsafeRawBufferPointer(rebasing: buffer[(count - 1) * 16..<count * 16])))
        XCTAssertTrue((ms...ms + 1).contains(try UuidGenerator.v7UnixMillis(last)))
    }

    // A v6 batch has no counter limit, but its count must fit the native call's 32 bits;
    // past that it throws instead of trapping on the conversion, again before allocating.
    func testAV6BatchPastTheNativeCountIsRefusedInsteadOfTrapping() throws {
        guard Int.bitWidth == 64 else { throw XCTSkip("a count past UInt32.max needs a 64-bit Int") }
        let tooMany = Int(UInt32.max) + 1
        XCTAssertThrowsError(try UuidGenerator.newV6Batch(count: tooMany, unixMillis: ms)) { error in
            guard case UuidGenerator.Error.batchNotAddressable(tooMany) = error else { return XCTFail("\(error)") }
        }
        var one = [UInt8](repeating: 0, count: 16)
        try one.withUnsafeMutableBytes { storage in
            let claimed = UnsafeMutableRawBufferPointer(start: storage.baseAddress, count: tooMany * 16)
            XCTAssertThrowsError(try UuidGenerator.fillV6(into: claimed, unixMillis: ms)) { error in
                guard case UuidGenerator.Error.batchNotAddressable(tooMany) = error else { return XCTFail("\(error)") }
            }
        }
        XCTAssertEqual(one, [UInt8](repeating: 0, count: 16))
    }

    // The two 1 GiB tests: skipped under WASI, whose 32-bit linear memory a gigabyte would
    // mostly fill, on Android, where the suite runs in an emulator whose memory a gigabyte
    // would crowd out, and anywhere else Int is 32 bits.
    private func skipUnlessGiBTestsCanRun() throws {
        #if os(WASI)
            throw XCTSkip("1 GiB is most of a wasm32 linear memory")
        #elseif os(Android)
            throw XCTSkip("1 GiB is too much of an emulator's memory")
        #else
            guard Int.bitWidth == 64 else { throw XCTSkip("1 GiB tests need a 64-bit address space") }
        #endif
    }
}
