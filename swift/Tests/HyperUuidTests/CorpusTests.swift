import Foundation
import XCTest

@testable import HyperUuid

/// Replays the shared conformance corpus (`corpus/*.json` at the repository root), the same
/// files the Rust core's own suite replays, through this binding's public API. Every value
/// crosses as a `UUID` the way a caller holds one: its `uuid` bytes are the corpus hex in
/// either layout, because a SQL-ordered `UUID` is exactly what `v7ToSqlOrder` returns, with
/// the SQL Server wire bytes as its own. A vector that fails here is a break in the
/// cross-language contract, and for `sql_order.json` a change to data already persisted in
/// SQL Server.
final class CorpusTests: XCTestCase {
    /// `corpus/` found by walking up from this file; `nil` under WASI, where the test module
    /// runs sandboxed with no view of the source tree.
    private static let corpusDirectory: URL? = {
        #if os(WASI)
            return nil
        #else
            // A device run (Android, through adb) has the corpus pushed beside the test
            // bundle, nowhere above the host path #filePath names.
            if let pushed = ProcessInfo.processInfo.environment["HYPERUUID_CORPUS"] {
                return URL(fileURLWithPath: pushed)
            }
            var dir = URL(fileURLWithPath: #filePath)
            while true {
                let corpus = dir.appendingPathComponent("corpus")
                var isDirectory: ObjCBool = false
                if FileManager.default.fileExists(atPath: corpus.path, isDirectory: &isDirectory), isDirectory.boolValue
                {
                    return corpus
                }
                let parent = dir.deletingLastPathComponent()
                if parent.path == dir.path { break }
                dir = parent
            }
            fatalError("corpus directory not found above \(#filePath)")
        #endif
    }()

    private func corpus<T: Decodable>(_ name: String, as type: T.Type) throws -> [T] {
        guard let dir = Self.corpusDirectory else { throw XCTSkip("no source tree under WASI") }
        return try JSONDecoder().decode([T].self, from: Data(contentsOf: dir.appendingPathComponent(name)))
    }

    private static func hex(_ text: String) -> [UInt8] {
        let digits = Array(text.utf8)
        precondition(digits.count % 2 == 0, "odd-length hex \(text)")
        return stride(from: 0, to: digits.count, by: 2).map {
            UInt8(String(decoding: digits[$0..<$0 + 2], as: UTF8.self), radix: 16)!
        }
    }

    private static func layout(_ name: String) -> UuidLayout {
        switch name {
        case "rfc9562": return .rfc9562
        case "sql_server": return .sqlServer
        default: fatalError("unknown layout \(name)")
        }
    }

    private struct V5Vector: Decodable {
        let namespace: String
        let name: String?
        let name_hex: String
        let expect: String
    }

    private struct SqlOrderVector: Decodable {
        let version: Int
        let rfc: String
        let sql: String
    }

    private struct TimestampVector: Decodable {
        let uuid: String
        let layout: String
        let version: Int
        let unix_millis: UInt64?
    }

    private struct InspectVector: Decodable {
        let uuid: String
        let layout: String
        let version: Int
        let is_rfc: Bool
        let variant: String?
    }

    func testV5Corpus() throws {
        for vector in try corpus("v5.json", as: V5Vector.self) {
            let ns: UUID
            switch vector.namespace {
            case "dns": ns = Namespaces.dns
            case "url": ns = Namespaces.url
            case "oid": ns = Namespaces.oid
            case "x500": ns = Namespaces.x500
            default: fatalError("unknown namespace \(vector.namespace)")
            }
            let expected = UUID(rfcBytes: Self.hex(vector.expect))
            XCTAssertEqual(
                try UuidGenerator.newV5(namespace: ns, name: Self.hex(vector.name_hex)), expected, "\(vector)")
            if let name = vector.name {
                XCTAssertEqual(try UuidGenerator.newV5(namespace: ns, name: name), expected, "\(vector)")
            }
        }
    }

    func testSqlOrderCorpus() throws {
        for vector in try corpus("sql_order.json", as: SqlOrderVector.self) {
            let rfcBytes = Self.hex(vector.rfc)
            let sqlBytes = Self.hex(vector.sql)
            let rfc = UUID(rfcBytes: rfcBytes)
            let sql = UUID(rfcBytes: sqlBytes)
            let toSql: (UUID) throws -> UUID
            let toRfc: (UUID) throws -> UUID
            let toSqlBytes: (UnsafeMutableRawBufferPointer) throws -> Void
            let toRfcBytes: (UnsafeMutableRawBufferPointer) throws -> Void
            switch vector.version {
            case 6:
                (toSql, toRfc) = (UuidGenerator.v6ToSqlOrder, UuidGenerator.v6FromSqlOrder)
                (toSqlBytes, toRfcBytes) = (UuidGenerator.v6ToSqlOrder(bytes:), UuidGenerator.v6FromSqlOrder(bytes:))
            case 7:
                (toSql, toRfc) = (UuidGenerator.v7ToSqlOrder, UuidGenerator.v7FromSqlOrder)
                (toSqlBytes, toRfcBytes) = (UuidGenerator.v7ToSqlOrder(bytes:), UuidGenerator.v7FromSqlOrder(bytes:))
            default: fatalError("no SQL order for version \(vector.version)")
            }
            XCTAssertEqual(try toSql(rfc), sql, "\(vector)")
            XCTAssertEqual(try toRfc(sql), rfc, "\(vector)")
            // What reaches SQL Server is the value's own sixteen bytes; they must be the wire bytes.
            XCTAssertEqual(try toSql(rfc).rfcBytes, sqlBytes, "\(vector)")

            // The raw-byte doors, in place.
            var bytes = rfcBytes
            try bytes.withUnsafeMutableBytes { try toSqlBytes($0) }
            XCTAssertEqual(bytes, sqlBytes, "\(vector)")
            try bytes.withUnsafeMutableBytes { try toRfcBytes($0) }
            XCTAssertEqual(bytes, rfcBytes, "\(vector)")
        }
    }

    func testTimestampCorpus() throws {
        var sawYear10889 = false
        for vector in try corpus("timestamp.json", as: TimestampVector.self) {
            let layout = Self.layout(vector.layout)
            let id = UUID(rfcBytes: Self.hex(vector.uuid))
            let expected = vector.unix_millis.map { Date(timeIntervalSince1970: Double($0) / 1000) }
            XCTAssertEqual(try UuidGenerator.version(id, layout: layout), vector.version, "\(vector)")
            XCTAssertEqual(try UuidGenerator.getTimestamp(id, layout: layout), expected, "\(vector)")
            if layout == .rfc9562 {
                XCTAssertEqual(try UuidGenerator.getTimestamp(id), expected, "\(vector)")
            }
            guard let millis = vector.unix_millis, let date = expected else { continue }

            // A Date holds every 48-bit v7 timestamp, up to 2^48 - 1 ms in year 10889.
            if millis == (1 << 48) - 1 {
                var calendar = Calendar(identifier: .gregorian)
                calendar.timeZone = TimeZone(identifier: "UTC")!
                XCTAssertEqual(calendar.component(.year, from: date), 10889, "\(vector)")
                XCTAssertEqual((date.timeIntervalSince1970 * 1000).rounded(), Double(millis), "\(vector)")
                sawYear10889 = true
            }

            switch vector.version {
            case 6:
                XCTAssertEqual(try UuidGenerator.v6UnixMillis(id, layout: layout), millis, "\(vector)")
                XCTAssertEqual(try UuidGenerator.v6Timestamp(id, layout: layout), date, "\(vector)")
                if layout == .rfc9562 {
                    XCTAssertEqual(try UuidGenerator.v6UnixMillis(id), millis, "\(vector)")
                    XCTAssertEqual(try UuidGenerator.v6Timestamp(id), date, "\(vector)")
                }
            case 7:
                XCTAssertEqual(try UuidGenerator.v7UnixMillis(id, layout: layout), millis, "\(vector)")
                XCTAssertEqual(try UuidGenerator.v7Timestamp(id, layout: layout), date, "\(vector)")
                if layout == .rfc9562 {
                    XCTAssertEqual(try UuidGenerator.v7UnixMillis(id), millis, "\(vector)")
                    XCTAssertEqual(try UuidGenerator.v7Timestamp(id), date, "\(vector)")
                }
            default:
                XCTFail("a timestamp on version \(vector.version): \(vector)")
            }
        }
        XCTAssertTrue(sawYear10889, "the corpus no longer has the 2^48 - 1 ms row")
    }

    func testInspectCorpus() throws {
        for vector in try corpus("inspect.json", as: InspectVector.self) {
            let layout = Self.layout(vector.layout)
            let bytes = Self.hex(vector.uuid)
            let id = UUID(rfcBytes: bytes)

            XCTAssertEqual(try UuidGenerator.version(id, layout: layout), vector.version, "\(vector)")
            XCTAssertEqual(
                try UuidGenerator.isRfc(id, version: vector.version, layout: layout), vector.is_rfc, "\(vector)")
            try bytes.withUnsafeBytes { raw in
                XCTAssertEqual(try UuidGenerator.version(bytes: raw, layout: layout), vector.version, "\(vector)")
                XCTAssertEqual(
                    try UuidGenerator.isRfc(bytes: raw, version: vector.version, layout: layout), vector.is_rfc,
                    "\(vector)")
            }
            for other in 0...15 where other != vector.version {
                XCTAssertFalse(try UuidGenerator.isRfc(id, version: other, layout: layout), "\(vector) isRfc(\(other))")
            }

            guard let variant = vector.variant else { continue }
            let expected: UuidVariant
            switch variant {
            case "ncs": expected = .ncs
            case "rfc9562": expected = .rfc9562
            case "microsoft": expected = .microsoft
            case "future": expected = .future
            default: fatalError("unknown variant \(variant)")
            }
            XCTAssertEqual(try UuidGenerator.variant(id), expected, "\(vector)")
            try bytes.withUnsafeBytes { raw in
                XCTAssertEqual(try UuidGenerator.variant(bytes: raw), expected, "\(vector)")
            }
            XCTAssertEqual(try UuidGenerator.version(id), vector.version, "\(vector)")
            XCTAssertEqual(try UuidGenerator.isRfc(id, version: vector.version), vector.is_rfc, "\(vector)")
        }
    }
}
