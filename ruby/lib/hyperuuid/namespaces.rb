module HyperUuid
  # Well-known namespace UUIDs defined in RFC 9562 Section 6.6.
  module Namespaces
    DNS = Uuid.parse("6ba7b810-9dad-11d1-80b4-00c04fd430c8")
    URL = Uuid.parse("6ba7b811-9dad-11d1-80b4-00c04fd430c8")
    OID = Uuid.parse("6ba7b812-9dad-11d1-80b4-00c04fd430c8")
    X500 = Uuid.parse("6ba7b814-9dad-11d1-80b4-00c04fd430c8")
  end
end
