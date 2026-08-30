import Foundation

/// Well-known namespace UUIDs defined in RFC 9562 Section 6.6.
public enum Namespaces {
    /// The DNS namespace UUID.
    public static let dns = UUID(uuidString: "6ba7b810-9dad-11d1-80b4-00c04fd430c8")!
    /// The URL namespace UUID.
    public static let url = UUID(uuidString: "6ba7b811-9dad-11d1-80b4-00c04fd430c8")!
    /// The ISO OID namespace UUID.
    public static let oid = UUID(uuidString: "6ba7b812-9dad-11d1-80b4-00c04fd430c8")!
    /// The X.500 DN namespace UUID.
    public static let x500 = UUID(uuidString: "6ba7b814-9dad-11d1-80b4-00c04fd430c8")!
}
