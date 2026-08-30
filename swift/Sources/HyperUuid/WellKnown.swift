import Foundation

/// The RFC 9562 §5.9 Nil and §5.10 Max UUIDs. Not named `Namespaces` alongside the §6.6
/// namespace constants — these aren't namespace UUIDs, just the type's own well-known values
/// — and not `nil`/`max` directly on `UuidGenerator`, since `nil` is a Swift keyword.
public enum WellKnownUuids {
    /// The RFC 9562 §5.9 Nil UUID — all 128 bits zero.
    public static let nilUUID = UUID(uuidString: "00000000-0000-0000-0000-000000000000")!
    /// The RFC 9562 §5.10 Max UUID — all 128 bits one.
    public static let maxUUID = UUID(uuidString: "ffffffff-ffff-ffff-ffff-ffffffffffff")!
}
