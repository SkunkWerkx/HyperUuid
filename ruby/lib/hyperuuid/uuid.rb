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
