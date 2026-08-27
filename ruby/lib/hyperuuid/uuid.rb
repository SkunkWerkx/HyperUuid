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

    # Converts an RFC 9562-ordered version 6 or 7 UUID to the byte order SQL Server's
    # `uniqueidentifier` needs on the wire to sort by creation order. Dispatches on `version`
    # the same way #timestamp does.
    #
    # `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a
    # `uniqueidentifier` column — doesn't compare a GUID's 16 bytes left to right; it uses a
    # fixed, non-sequential byte significance order (octets 10,11,12,13,14,15,8,9,6,7,4,5,
    # 0,1,2,3, most significant first). Computed once in the native Rust core and verified
    # there (and independently, against the real SqlGuid comparator, in this project's C#
    # test suite); this binding calls the same native functions rather than reimplementing
    # the byte math.
    #
    # For v7, this moves the timestamp and counter — the two fields that determine creation
    # order — into that comparison's most-significant bytes, and moves the trailing entropy,
    # which carries no ordering information, into the least-significant ones as one intact
    # block. For v6, which has no monotonic counter the way v7 does, the only field that
    # determines creation order is the 60-bit timestamp itself, so that moves into the most
    # significant bytes instead, with `clock_seq`/`node` (independently random per call, not
    # a counter, so no ordering value either way) relocated into the rest. v6's much simpler
    # byte layout needs no bit-level repacking to do this — just whole-octet-group
    # relocation — unlike v7's, and its version/variant land at different sql-order offsets
    # as a result (octet 8's top nibble / octet 6's top two bits, not 7/8).
    #
    # **v6-specific caveat, unlike v7:** two version 6 UUIDs minted at the same millisecond
    # have identical timestamp bits, so they aren't guaranteed to sort in creation order any
    # more than plain RFC order already does — a pre-existing RFC 9562 v6 limitation, not one
    # this transform introduces.
    #
    # Meaningful only for a genuine version 6 or 7 UUID.
    def to_sql_order
      case version
      when 7 then self.class.new(Runtime.v7_to_sql_order(bytes))
      when 6 then self.class.new(Runtime.v6_to_sql_order(bytes))
      else raise ArgumentError, "to_sql_order is only defined for version 6 or 7 UUIDs, got version #{version}"
      end
    end

    # Inverse of #to_sql_order — converts a SQL-Server-ordered version 6 or 7 UUID back to
    # RFC 9562 order.
    #
    # A SQL-ordered value's version nibble sits at a different octet depending on which
    # version produced it (octet 7's top nibble = 7 for v7-sql-order, octet 8's top nibble =
    # 6 for v6-sql-order — #version itself assumes RFC order's octet 6 and can't tell these
    # apart), so this checks both fixed positions directly rather than calling #version.
    #
    # Order matters here and isn't arbitrary: octet 8 must be checked *first*. For v6-sql-order
    # it's deterministic (top nibble always 0x6, by construction), and for v7-sql-order it's
    # also deterministic but structurally excluded from ever reading 0x6 (its top two bits are
    # the fixed variant `10`, so the nibble only ever lands in 0x8-0xB) — no collision either
    # way. Octet 7, by contrast, is *not* safe to check first: for v7-sql-order it's
    # deterministically 0x7, but for v6-sql-order it holds `clock_seq`'s fully random low
    # byte, which has a real (~1-in-16) chance of a top nibble that also happens to read 0x7 —
    # confirmed by an actual test failure during development, not a hypothetical. Checking
    # octet 8 first rules v6 in or out unambiguously before octet 7's reading can matter.
    def from_sql_order
      octet8_version = (bytes.getbyte(8) >> 4) & 0x0F
      octet7_version = (bytes.getbyte(7) >> 4) & 0x0F
      if octet8_version == 6
        self.class.new(Runtime.v6_to_rfc_order(bytes))
      elsif octet7_version == 7
        self.class.new(Runtime.v7_to_rfc_order(bytes))
      else
        raise ArgumentError, "from_sql_order: not a recognized version 6 or 7 SQL-ordered UUID"
      end
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
