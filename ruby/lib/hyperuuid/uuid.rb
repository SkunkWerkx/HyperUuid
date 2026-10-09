module HyperUuid
  # A parsed 16-byte RFC 9562 UUID value. Minimal by design — this gem has no runtime
  # dependency on the `uuid` gem, the same "no extra dependency" positioning as the Go
  # binding's lone google/uuid requirement and the Python binding's dependency-free PyO3
  # wheels.
  class Uuid
    include Comparable

    # The UUID's 16 raw bytes in RFC 9562 (big-endian) order.
    attr_reader :bytes

    # Wraps a raw 16-byte RFC 9562 (big-endian) UUID value. The bytes are copied, so the
    # String passed in stays the caller's.
    #
    # +owned+ is for this gem's own use: it says +bytes+ is a String the native core has
    # just produced — sixteen bytes, already ASCII-8BIT, held by nothing else — so there is
    # nothing to check, copy or re-tag, and that copy is most of what constructing one costs.
    #
    # @raise [ArgumentError] if +bytes+ isn't exactly 16 bytes.
    def initialize(bytes, owned = false)
      if owned
        @bytes = bytes.freeze
      else
        raise ArgumentError, "bytes must be exactly 16 bytes" unless bytes.bytesize == 16

        @bytes = bytes.dup.force_encoding(Encoding::BINARY).freeze
      end
    end

    # The RFC 9562 §5.9 Nil UUID — all 128 bits zero.
    NIL = new(("\x00" * 16).b).freeze

    # The RFC 9562 §5.10 Max UUID — all 128 bits one.
    MAX = new(("\xFF" * 16).b).freeze

    # The one text shape .parse accepts: 8-4-4-4-12 hex digits, either case, the four hyphens
    # exactly where #to_s puts them.
    HYPHENATED = /\A\h{8}-\h{4}-\h{4}-\h{4}-\h{12}\z/
    private_constant :HYPHENATED

    # The byte orders a Uuid can hold, as the +layout:+ keyword of #version, #rfc? and
    # #timestamp takes them, mapped to the native core's layout codes. +:rfc9562+ is RFC 9562
    # order, what every other method here takes and returns. +:sql_server+ is the order
    # #to_sql_order returns, defined for versions 6 and 7 only.
    LAYOUTS = { rfc9562: 1, sql_server: 2 }.freeze

    # The Symbols #variant returns, indexed by the native core's variant code (1-4; 0 is
    # reserved and never returned). The one place that code becomes a Symbol, for both backends.
    VARIANTS = [nil, :ncs, :rfc9562, :microsoft, :future].freeze
    private_constant :VARIANTS

    # Parses an 8-4-4-4-12 hyphenated hex UUID string — exactly the shape #to_s produces, in
    # either case. Nothing else parses: not the bare 32 hex digits, not hyphens anywhere but
    # those four positions, not braces or a `urn:uuid:` prefix.
    #
    # @raise [ArgumentError] if +string+ isn't a String in that shape.
    def self.parse(string)
      unless string.is_a?(String) && string.match?(HYPHENATED)
        raise ArgumentError, "invalid UUID string: #{string.inspect}"
      end

      new([string.delete("-")].pack("H*"))
    end

    # The version of this UUID, read by the native core in +layout+'s byte order.
    #
    # In +:rfc9562+ (the default) it is the version nibble (bits 48-51, the high nibble of
    # octet 6), 0 through 15: Nil reads as 0 and Max as 15. It says nothing about the variant;
    # #rfc? checks both. In +:sql_server+, the order #to_sql_order returns, it is 6 or 7 for a
    # SQL-ordered version 6 or 7 UUID and 0 when the bytes don't form one, since no other
    # version has a SQL Server order. It reads the bytes as given: an RFC-ordered value can
    # happen to form a SQL-ordered v6 or v7, so the layout is the caller's to know.
    #
    # @raise [ArgumentError] if +layout+ isn't one of LAYOUTS' keys.
    def version(layout: :rfc9562)
      Runtime.version(bytes, layout_code(layout))
    end

    # The variant of this UUID (RFC 9562 §4.1), read by the native core in RFC 9562 order, as a
    # Symbol: +:rfc9562+ (+10xx+, every UUID this gem mints, and the only variant that has
    # versions), +:ncs+ (+0xxx+, Nil among them), +:microsoft+ (+110x+) or +:future+ (+111x+,
    # Max among them).
    def variant
      VARIANTS.fetch(Runtime.variant(bytes))
    end

    # Whether this is an RFC 9562 UUID of version +version+, read in +layout+'s byte order —
    # the RFC 9562 variant and that version together, the guard to run before trusting a
    # value's version-specific fields. In +:sql_server+ only 6 and 7 can be true. A version
    # outside 0-15 is simply false.
    #
    # @raise [TypeError] if +version+ isn't an Integer.
    # @raise [ArgumentError] if +layout+ isn't one of LAYOUTS' keys.
    def rfc?(version, layout: :rfc9562)
      raise TypeError, "version must be an Integer; got #{version.class}" unless version.is_a?(Integer)

      code = layout_code(layout)
      version.between?(0, 15) && Runtime.is_rfc(bytes, version, code)
    end

    # The 8-4-4-4-12 hyphenated hex string representation.
    def to_s
      hex = bytes.unpack1("H*")
      "#{hex[0, 8]}-#{hex[8, 4]}-#{hex[12, 4]}-#{hex[16, 4]}-#{hex[20, 12]}"
    end
    alias_method :to_str, :to_s

    # The UTC timestamp embedded in an RFC 9562 version 6 or 7 UUID, held in +layout+'s byte
    # order: +:rfc9562+ (the default), or +:sql_server+ for a value #to_sql_order returned. One
    # native call that checks the variant as well as the version: a 6 or 7 in the version
    # nibble under any variant but +:rfc9562+ is not a version 6 or 7 UUID, so it has no
    # timestamp (#rfc? is the same check on its own).
    #
    # Raises by default for anything that isn't an RFC 9562 version 6 or 7 UUID; pass
    # `raise_on_mismatch: false` to get `nil` back instead — for a caller that doesn't already
    # know (or want to separately check) whether this UUID is time-based.
    #
    # @raise [ArgumentError] if this isn't an RFC 9562 version 6 or 7 UUID in +layout+ (unless
    #   +raise_on_mismatch+ is false), or +layout+ isn't one of LAYOUTS' keys.
    def timestamp(raise_on_mismatch: true, layout: :rfc9562)
      millis = Runtime.get_timestamp(bytes, layout_code(layout))
      return Time.at(millis / 1000, millis % 1000, :millisecond).utc if millis
      return nil unless raise_on_mismatch

      raise ArgumentError, timestamp_mismatch(layout)
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
      when 7 then self.class.new(Runtime.v7_to_sql_order(bytes), true)
      when 6 then self.class.new(Runtime.v6_to_sql_order(bytes), true)
      else raise ArgumentError, "to_sql_order is only defined for version 6 or 7 UUIDs, got version #{version}"
      end
    end

    # Inverse of #to_sql_order — converts a SQL-Server-ordered version 6 or 7 UUID back to
    # RFC 9562 order.
    #
    # A SQL-ordered value's version sits at a different octet depending on which version
    # produced it, so this asks the native core which one it is — `version(layout:
    # :sql_server)`, which checks each version's nibble together with its variant bits and
    # so never mistakes a v6 whose random `clock_seq` byte happens to read as a v7 nibble.
    #
    # The layout is the caller's to know: bytes alone cannot say which order they are in. A
    # value that was never SQL-ordered can still read as one, and is then converted rather
    # than refused (a random RFC-ordered v4 reads as a SQL-ordered v7 one time in 16, since its
    # octet 8 already carries the RFC variant and its octet 7 is random).
    #
    # @raise [ArgumentError] if this isn't a SQL-ordered version 6 or 7 UUID.
    def from_sql_order
      case Runtime.version(bytes, LAYOUTS[:sql_server])
      when 6 then self.class.new(Runtime.v6_to_rfc_order(bytes), true)
      when 7 then self.class.new(Runtime.v7_to_rfc_order(bytes), true)
      else raise ArgumentError, "from_sql_order: not a recognized version 6 or 7 SQL-ordered UUID"
      end
    end

    # Whether +other+ wraps the same 16 raw bytes.
    def ==(other)
      other.is_a?(Uuid) && bytes == other.bytes
    end
    alias_method :eql?, :==

    # Hash code consistent with #==, based on the raw bytes.
    def hash
      bytes.hash
    end

    # Byte-order comparison against +other+, or +nil+ if +other+ isn't a Uuid.
    def <=>(other)
      return nil unless other.is_a?(Uuid)

      bytes <=> other.bytes
    end

    # Debug representation, e.g. <tt>#<HyperUuid::Uuid ...></tt>.
    def inspect
      "#<HyperUuid::Uuid #{self}>"
    end

    private

    # Why #timestamp found none: the message is worked out only once it is going to be raised.
    def timestamp_mismatch(layout)
      return "timestamp: not a SQL-ordered version 6 or 7 UUID" if layout == :sql_server

      found = version
      message = "timestamp is only defined for version 6 or 7 UUIDs, got version #{found}"
      [6, 7].include?(found) ? "#{message} with the #{variant.inspect} variant" : message
    end

    # The native core's code for +layout+, the one place a layout is checked: an unknown one
    # is refused here, never passed through to the core.
    def layout_code(layout)
      LAYOUTS.fetch(layout) do
        raise ArgumentError, "layout must be one of #{LAYOUTS.keys.map(&:inspect).join(', ')}; got #{layout.inspect}"
      end
    end
  end
end
