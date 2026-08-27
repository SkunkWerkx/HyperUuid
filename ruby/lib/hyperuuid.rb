# RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, calling
# directly into the native libhyperuuid shared library via Fiddle — no runtime bridge, no
# extra gem dependency. Bundles a native build for every supported platform (see
# HyperUuid::NativePlatform) and picks the right one at runtime, the same trick the Go/
# Java bindings use since RubyGems has no per-platform native selection wired up here.
require "time"

require_relative "hyperuuid/uuid"
require_relative "hyperuuid/namespaces"
require_relative "hyperuuid/native_platform"
require_relative "hyperuuid/runtime"

module HyperUuid
  VERSION = "0.1.0"

  # Creates a random UUID version 4 (RFC 9562 §5.4).
  def self.new_v4
    Uuid.new(Runtime.new_v4)
  end

  # Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a name. The
  # same (namespace, name) pair always produces the same UUID. `name` may be a text String
  # (encoded as UTF-8) or already-raw ASCII-8BIT bytes, which are used as-is.
  def self.new_v5(namespace, name)
    name_bytes =
      if name.encoding == Encoding::ASCII_8BIT
        name
      else
        name.encode(Encoding::UTF_8).dup.force_encoding(Encoding::BINARY)
      end
    Uuid.new(Runtime.new_v5(namespace.bytes, name_bytes))
  end

  # Creates a time-sortable UUID version 6 (RFC 9562 §5.6), a field-compatible reordering of
  # version 1 for better sort/index locality. Defaults to the current time; pass an explicit
  # Unix-epoch millisecond timestamp to embed a specific time instead. `clock_seq` and `node`
  # are randomly generated on every call — unlike version 7, there is no monotonic counter, so
  # calls within the same millisecond are not guaranteed to sort in creation order.
  def self.new_v6(unix_millis = nil)
    unix_millis ||= (Time.now.to_r * 1000).to_i
    Uuid.new(Runtime.new_v6(unix_millis))
  end

  # Creates `count` time-sortable version 6 UUIDs sharing one timestamp capture — one FFI call
  # and one random-bytes fetch instead of `count` of each. Defaults to the current time.
  def self.new_v6_batch(count, unix_millis = nil)
    unix_millis ||= (Time.now.to_r * 1000).to_i
    bytes = Runtime.new_v6_batch(count, unix_millis)
    Array.new(count) { |i| Uuid.new(bytes[i * 16, 16]) }
  end

  # Creates a time-sortable UUID version 7 (RFC 9562 §6.2). Defaults to the current time; pass
  # an explicit Unix-epoch millisecond timestamp (non-negative, fitting in 48 bits) to embed a
  # specific time instead.
  def self.new_v7(unix_millis = nil)
    unix_millis ||= (Time.now.to_r * 1000).to_i
    Uuid.new(Runtime.new_v7(unix_millis))
  end

  # Creates `count` time-sortable version 7 UUIDs sharing one timestamp capture and one
  # contiguous block of the monotonic counter — one FFI call and one random-bytes fetch
  # instead of `count` of each. Defaults to the current time.
  def self.new_v7_batch(count, unix_millis = nil)
    unix_millis ||= (Time.now.to_r * 1000).to_i
    bytes = Runtime.new_v7_batch(count, unix_millis)
    Array.new(count) { |i| Uuid.new(bytes[i * 16, 16]) }
  end
end
