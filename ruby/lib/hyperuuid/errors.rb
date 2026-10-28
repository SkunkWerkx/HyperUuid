module HyperUuid
  # Raised when the operating system's random source fails while a UUID is being minted —
  # the one way `new_v4`, `new_v7` and its batch forms can fail once their
  # arguments are valid. Carries the same message on every backend.
  class RandomSourceError < StandardError; end

  # Raised when a timestamp cannot be embedded in the version 7 UUID being minted: one past
  # RFC 9562's 48-bit millisecond field, or a negative one (a `Time` before the Unix epoch
  # included). Carries the same message on every backend.
  class TimestampOutOfRangeError < StandardError; end

  # Raised when the version 7 stream cannot advance without running more than 1000 ms ahead of
  # the supplied timestamp or past its last millisecond (2**44 - 2). Nothing is reserved.
  # Carries the same message on every backend.
  class CounterExhaustedError < StandardError; end
end
