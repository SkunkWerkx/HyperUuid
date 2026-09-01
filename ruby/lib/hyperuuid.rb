require "time"

require_relative "hyperuuid/uuid"
require_relative "hyperuuid/namespaces"
require_relative "hyperuuid/native_platform"
require_relative "hyperuuid/runtime"

# RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, calling
# directly into the native libhyperuuid shared library via Fiddle — no runtime bridge, no
# extra gem dependency. Bundles a native build for every supported platform (see
# HyperUuid::NativePlatform) and picks the right one at runtime, the same trick the Go/
# Java bindings use since RubyGems has no per-platform native selection wired up here.
module HyperUuid
  # This gem's own version — distinct from the RFC 9562 UUID *versions* (v4/v5/v6/v7) the
  # rest of this module generates.
  VERSION = "0.1.1"

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

  # Converts +value+ to a Unix-epoch millisecond integer: +nil+ becomes the current time, a
  # +Time+ is converted exactly (via its own Rational seconds, avoiding float rounding), and
  # anything else (an Integer millisecond count) passes through unchanged. Shared by every
  # `new_v6`/`new_v7`/batch door below so a caller can pass either a `Time` or a raw
  # millisecond count interchangeably.
  private_class_method def self.unix_millis_from(value)
    case value
    when nil then Process.clock_gettime(Process::CLOCK_REALTIME, :millisecond)
    when Time then (value.to_r * 1000).to_i
    else value
    end
  end

  # Creates a time-sortable UUID version 6 (RFC 9562 §5.6), a field-compatible reordering of
  # version 1 for better sort/index locality. Defaults to the current time; pass an explicit
  # `Time` or Unix-epoch millisecond integer to embed a specific time instead. `clock_seq` and
  # `node` are randomly generated on every call — unlike version 7, there is no monotonic
  # counter, so calls within the same millisecond are not guaranteed to sort in creation order.
  def self.new_v6(unix_millis = nil)
    Uuid.new(Runtime.new_v6(unix_millis_from(unix_millis)))
  end

  # Creates `count` time-sortable version 6 UUIDs sharing one timestamp capture — one FFI call
  # and one random-bytes fetch instead of `count` of each. Defaults to the current time; pass
  # an explicit `Time` or Unix-epoch millisecond integer to embed a specific time instead.
  def self.new_v6_batch(count, unix_millis = nil)
    bytes = Runtime.new_v6_batch(count, unix_millis_from(unix_millis))
    Array.new(count) { |i| Uuid.new(bytes[i * 16, 16]) }
  end

  # Creates a time-sortable UUID version 7 (RFC 9562 §6.2). Defaults to the current time; pass
  # an explicit `Time` or Unix-epoch millisecond integer (non-negative, fitting in 48 bits) to
  # embed a specific time instead.
  def self.new_v7(unix_millis = nil)
    Uuid.new(Runtime.new_v7(unix_millis_from(unix_millis)))
  end

  # Creates `count` time-sortable version 7 UUIDs sharing one timestamp capture and one
  # contiguous block of the monotonic counter — one FFI call and one random-bytes fetch
  # instead of `count` of each. Defaults to the current time; pass an explicit `Time` or
  # Unix-epoch millisecond integer to embed a specific time instead.
  def self.new_v7_batch(count, unix_millis = nil)
    bytes = Runtime.new_v7_batch(count, unix_millis_from(unix_millis))
    Array.new(count) { |i| Uuid.new(bytes[i * 16, 16]) }
  end

  # Returns `count` version 7 UUIDs as one binary String of raw RFC 9562-ordered bytes,
  # 16 per UUID, instead of an Array of Uuid objects.
  #
  # Roughly 11x faster than #new_v7_batch for a 1000-UUID batch (about 35 us versus 400 us).
  # The difference is not the native call — that is identical — it is that #new_v7_batch then
  # allocates `count` Uuid objects and `count` String slices on top of it. This hands back the
  # bytes the native core already produced, untouched.
  #
  # Use it when bytes are the destination: a BYTEA/uniqueidentifier bind parameter, a wire
  # format, a bulk COPY. If you need Uuid objects, keep using #new_v7_batch — slicing this
  # String into them yourself just moves the same allocations into your own code.
  #
  # Slice it with `bytes[i * 16, 16]`, which is what #new_v7_batch does internally.
  def self.new_v7_batch_bytes(count, unix_millis = nil)
    Runtime.new_v7_batch(count, unix_millis_from(unix_millis))
  end

  # Returns `count` version 6 UUIDs as one binary String of raw RFC 9562-ordered bytes,
  # 16 per UUID. The version 6 counterpart to #new_v7_batch_bytes, with the same rationale and
  # the same guidance about when it is the right call.
  #
  # clock_seq and node are independently random per item; unlike version 7 there is no
  # monotonic counter, so items minted in the same millisecond are not guaranteed to sort in
  # creation order.
  def self.new_v6_batch_bytes(count, unix_millis = nil)
    Runtime.new_v6_batch(count, unix_millis_from(unix_millis))
  end
end

# --- backend selection: the Magnus extension, when present, replaces the Runtime methods
# above in place (no delegation layer) — Fiddle's measured per-call marshalling floor drops
# to an ordinary extension call, while everything above Runtime (Uuid, the module doors,
# batch slicing) stays shared byte-for-byte between backends. The pure-Fiddle definitions
# remain the universal zero-compile fallback; precompiled platform gems are how the
# extension ships without ever making a consumer compile anything. Set HYPERUUID_PURE=1 to
# force Fiddle.
HyperUuid::BACKEND =
  if ENV["HYPERUUID_PURE"]
    :fiddle
  else
    # Two layouts, and both have to work. A released platform gem is a "fat" gem carrying one
    # extension per supported Ruby ABI under lib/hyperuuid/<minor>/ (see the Rakefile's
    # native:gem task for why an ABI-per-file is unavoidable — Magnus has no `abi3`
    # equivalent). CI's in-job staging and a local `cargo build --release --features ruby`
    # instead drop a single extension flat at lib/. Trying the versioned path first and the
    # flat one second means neither has to know the other exists.
    #
    # A miss on both is not an error: it means this Ruby/platform combination has no
    # precompiled extension, which is precisely what the Fiddle backend below is for.
    begin
      require "hyperuuid/#{RUBY_VERSION[/\d+\.\d+/]}/hyperuuid_native"
      :native
    rescue LoadError
      begin
        require "hyperuuid_native"
        :native
      rescue LoadError
        :fiddle
      end
    end
  end
