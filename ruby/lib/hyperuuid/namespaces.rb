module HyperUuid
  # Well-known namespace UUIDs defined in RFC 9562 Section 6.6.
  module Namespaces
    # The DNS namespace UUID.
    DNS = Uuid.parse("6ba7b810-9dad-11d1-80b4-00c04fd430c8")
    # The URL namespace UUID.
    URL = Uuid.parse("6ba7b811-9dad-11d1-80b4-00c04fd430c8")
    # The ISO OID namespace UUID.
    OID = Uuid.parse("6ba7b812-9dad-11d1-80b4-00c04fd430c8")
    # The X.500 DN namespace UUID.
    X500 = Uuid.parse("6ba7b814-9dad-11d1-80b4-00c04fd430c8")
  end
end
