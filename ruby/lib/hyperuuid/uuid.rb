module HyperUuid
  # A parsed 16-byte RFC 9562 UUID value. Minimal by design — this gem has no runtime
  # dependency on the `uuid` gem, the same "no extra dependency" positioning as the Go
  # binding's purego-only/no-cgo approach and the Python binding's stdlib-ctypes-only one.
  class Uuid
    include Comparable

    attr_reader :bytes

    def initialize(bytes)
      raise ArgumentError, "bytes must be exactly 16 bytes" unless bytes.bytesize == 16
      @bytes = bytes.dup.force_encoding(Encoding::BINARY).freeze
    end

    # The RFC 9562 §5.9 Nil UUID — all 128 bits zero.
    NIL = new(("\x00" * 16).b).freeze

    # The RFC 9562 §5.10 Max UUID — all 128 bits one.
    MAX = new(("\xFF" * 16).b).freeze

    def self.parse(string)
      hex = string.delete("-")
      raise ArgumentError, "invalid UUID string: #{string.inspect}" unless hex.match?(/\A[0-9a-fA-F]{32}\z/)
      new([hex].pack("H*"))
    end

    def version
      (bytes.getbyte(6) >> 4) & 0x0F
    end

    def variant
      (bytes.getbyte(8) >> 6) & 0b11
    end

    def to_s
      hex = bytes.unpack1("H*")
      "#{hex[0, 8]}-#{hex[8, 4]}-#{hex[12, 4]}-#{hex[16, 4]}-#{hex[20, 12]}"
    end
    alias_method :to_str, :to_s

    # The UTC timestamp embedded in a version 6 or 7 UUID's timestamp field. Only meaningful
    # when `version` is 6 or 7 — the RFC 9562 bit layout doesn't distinguish "not a time-based
    # UUID" from "time-based UUID with a very early timestamp", so the caller is responsible
    # for checking `version` first if that matters.
    def timestamp
      millis =
        case version
        when 6 then Runtime.v6_unix_millis(bytes)
        when 7 then Runtime.v7_unix_millis(bytes)
        else raise ArgumentError, "timestamp is only defined for version 6 or 7 UUIDs, got version #{version}"
        end
      Time.at(millis / 1000, millis % 1000, :millisecond).utc
    end

    # Converts an RFC 9562-ordered version 7 UUID to the byte order SQL Server's
    # `uniqueidentifier` needs on the wire to sort by creation order.
    #
    # `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a
    # `uniqueidentifier` column — doesn't compare a GUID's 16 bytes left to right; it uses a
    # fixed, non-sequential byte significance order (octets 10,11,12,13,14,15,8,9,6,7,4,5,
    # 0,1,2,3, most significant first). This moves the timestamp and counter — the two fields
    # that determine creation order — into that comparison's most-significant bytes, and moves
    # the trailing entropy, which carries no ordering information, into the least-significant
    # ones as one intact block. Computed once in the native Rust core and verified there (and
    # independently, against the real SqlGuid comparator, in this project's C# test suite);
    # this binding calls the same native function rather than reimplementing the byte math.
    #
    # Meaningful only for a genuine version 7 UUID.
    def to_sql_order
      self.class.new(Runtime.v7_to_sql_order(bytes))
    end

    # Inverse of #to_sql_order — converts a SQL-Server-ordered version 7 UUID back to RFC 9562 order.
    def from_sql_order
      self.class.new(Runtime.v7_to_rfc_order(bytes))
    end

    def ==(other)
      other.is_a?(Uuid) && bytes == other.bytes
    end
    alias_method :eql?, :==

    def hash
      bytes.hash
    end

    def <=>(other)
      return nil unless other.is_a?(Uuid)
      bytes <=> other.bytes
    end

    def inspect
      "#<HyperUuid::Uuid #{self}>"
    end
  end
end
