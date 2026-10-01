module HyperUuid
  # Raised when the operating system's random source fails while a UUID is being minted —
  # the one way `new_v4`, `new_v6`, `new_v7` and their batch forms can fail once their
  # arguments are valid. Carries the same message on every backend.
  class RandomSourceError < StandardError; end

  # Raised when a timestamp cannot be embedded in the UUID being minted: a version 7 value
  # past RFC 9562's 48-bit millisecond field, a version 6 value past its 60-bit field, or
  # any negative one (a `Time` before the Unix epoch included). Carries the same message on
  # every backend.
  class TimestampOutOfRangeError < StandardError; end
end
