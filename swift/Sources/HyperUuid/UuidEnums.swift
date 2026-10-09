/// The byte order a UUID is held in, for the inspection and timestamp methods that take one.
///
/// The raw values are the native core's layout codes. The core reserves 0 for "no layout",
/// which a Swift enum has no way to hold, so every `UuidLayout` a caller can pass is a real
/// one and there is no invalid-layout error to throw.
public enum UuidLayout: UInt32, Sendable, CaseIterable {
    /// RFC 9562 order, what every other method in this library takes and returns.
    case rfc9562 = 1
    /// The order `v7ToSqlOrder` and `v6ToSqlOrder` return, which SQL Server's
    /// `uniqueidentifier` sorts by creation order. Defined for versions 6 and 7 only.
    case sqlServer = 2
}

/// The variant field of a UUID (RFC 9562 §4.1), which says how the rest of its bits are laid
/// out; see ``UuidGenerator/variant(_:)``. Only ``rfc9562`` has versions.
///
/// The raw values are the native core's variant codes, which reserve 0; the core never
/// returns it, so there is no case for it.
public enum UuidVariant: UInt32, Sendable, CaseIterable {
    /// `0xxx`: reserved, Network Computing System backward compatibility. Includes Nil.
    case ncs = 1
    /// `10xx`: the variant RFC 9562 (and RFC 4122 before it) specifies.
    case rfc9562 = 2
    /// `110x`: reserved, Microsoft Corporation backward compatibility.
    case microsoft = 3
    /// `111x`: reserved for future definition. Includes Max.
    case future = 4
}
