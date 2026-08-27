import Foundation

extension UUID {
    /// This UUID's 16 RFC 9562 network-byte-order bytes. `Foundation.UUID.uuid` (a `uuid_t`
    /// 16-tuple) already stores bytes in this order on every platform Foundation supports —
    /// verified here against RFC 9562's own test vectors in `UuidGeneratorTests`, the same way
    /// the Kotlin binding's `RfcBytes.kt` conversion is verified against `java.util.UUID`'s
    /// actual bit layout — so this is a plain reinterpretation, not a byte-order conversion.
    var rfcBytes: [UInt8] {
        let u = self.uuid
        return [
            u.0, u.1, u.2, u.3, u.4, u.5, u.6, u.7,
            u.8, u.9, u.10, u.11, u.12, u.13, u.14, u.15,
        ]
    }

    init(rfcBytes bytes: [UInt8]) {
        precondition(bytes.count == 16, "rfcBytes must be exactly 16 bytes")
        let u: uuid_t = (
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
        )
        self.init(uuid: u)
    }
}
