import Foundation
import HyperUuid

// Every native entry point, crossed once through the public API, with the answers checked.
// Exits non-zero on the first thing that is wrong, so the build-and-run is the test.

func check(_ condition: Bool, _ what: String) {
    guard condition else {
        print("FAILED: \(what)")
        exit(1)
    }
}

func versionNibble(_ uuid: UUID) -> UInt8 { uuid.uuid.6 >> 4 }

do {
    check(UuidGenerator.isAvailable, "isAvailable")
    let version = try UuidGenerator.nativeVersion()
    check(version.split(separator: ".").count == 3, "nativeVersion is major.minor.patch, got \(version)")

    // v4
    let v4 = try UuidGenerator.newV4()
    check(versionNibble(v4) == 4, "v4 version nibble")
    check(try UuidGenerator.newV4() != v4, "two v4s differ")

    // v5: RFC 9562 Appendix A.4's vector.
    let v5 = try UuidGenerator.newV5(namespace: Namespaces.dns, name: "www.example.com")
    check(v5.uuidString.lowercased() == "2ed6657d-e927-568b-95e1-2665a8aea6a2", "v5 known answer, got \(v5)")

    // v6 and v7: an explicit timestamp reads back, the clock form is recent, a batch is
    // whole, and the SQL Server byte order round-trips.
    let millis: UInt64 = 1_700_000_000_000
    let now = UInt64(Date().timeIntervalSince1970 * 1000)

    let v6 = try UuidGenerator.newV6(unixMillis: millis)
    check(versionNibble(v6) == 6, "v6 version nibble")
    check(try UuidGenerator.v6UnixMillis(v6) == millis, "v6 timestamp round-trip")
    check(try UuidGenerator.v6UnixMillis(try UuidGenerator.newV6()) >= now - 5_000, "v6 at the current time")
    let v6Batch = try UuidGenerator.newV6Batch(count: 64, unixMillis: millis)
    check(v6Batch.count == 64 && Set(v6Batch).count == 64, "v6 batch of 64 distinct")
    check(try UuidGenerator.v6FromSqlOrder(try UuidGenerator.v6ToSqlOrder(v6)) == v6, "v6 SQL order round-trip")

    let v7 = try UuidGenerator.newV7(unixMillis: millis)
    check(versionNibble(v7) == 7, "v7 version nibble")
    check(try UuidGenerator.v7UnixMillis(v7) == millis, "v7 timestamp round-trip")
    check(try UuidGenerator.v7UnixMillis(try UuidGenerator.newV7()) >= now - 5_000, "v7 at the current time")
    let v7Batch = try UuidGenerator.newV7Batch(count: 64, unixMillis: millis)
    check(v7Batch.count == 64 && Set(v7Batch).count == 64, "v7 batch of 64 distinct")
    check(v7Batch == v7Batch.sorted { $0.uuidString < $1.uuidString }, "v7 batch is in order")
    check(try UuidGenerator.v7FromSqlOrder(try UuidGenerator.v7ToSqlOrder(v7)) == v7, "v7 SQL order round-trip")

    // An out-of-range timestamp is the native core's refusal, surfaced as this binding's error.
    do {
        _ = try UuidGenerator.newV7(unixMillis: 1 << 48)
        check(false, "a 49-bit timestamp was accepted")
    } catch UuidGenerator.Error.timestampOutOfRange {
    }

    print("hyperuuid \(version) smoke test passed: v4=\(v4) v5=\(v5) v6=\(v6) v7=\(v7)")
} catch {
    print("FAILED: \(error)")
    exit(1)
}
