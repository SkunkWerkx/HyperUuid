import Foundation
import XCTest

@testable import HyperUuid

final class UuidGeneratorTests: XCTestCase {
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
}
