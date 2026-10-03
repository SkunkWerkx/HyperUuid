require_relative "hyperuuid/errors"
require_relative "hyperuuid/uuid"
require_relative "hyperuuid/namespaces"
require_relative "hyperuuid/native_platform"
require_relative "hyperuuid/runtime"

# RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation from one
# Rust core, reached through whichever of two backends this install can load: the core
# linked straight into a Magnus native extension (shipped precompiled in the platform gems),
# or the native libhyperuuid shared library called through Fiddle (the universal gem bundles
# one per supported platform, picked by HyperUuid::NativePlatform). No runtime bridge in
# either, and nothing compiled at install time. HyperUuid::BACKEND names the one that loaded; the selection logic
# is at the bottom of this file.
#
# The doors below — with Uuid, Namespaces and the two exception classes — are the public
# surface, and are shared by every backend, argument validation included: a caller bug raises
# the same error whichever backend is live.
module HyperUuid
  # This gem's own version — distinct from the RFC 9562 UUID *versions* (v4/v5/v6/v7) the
  # rest of this module generates.
  VERSION = "0.6.0"

  # The widest batch count, and the longest v5 name in bytes, the native ABI carries (a u32).
  # (The millisecond count is a u64; unix_millis_from refuses anything wider.)
  U32_MAX = 0xFFFF_FFFF
  private_constant :U32_MAX

  # Creates a random UUID version 4 (RFC 9562 §5.4).
  def self.new_v4
    Uuid.new(Runtime.new_v4, true)
  end

  # Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a name. The
  # same (namespace, name) pair always produces the same UUID. `name` may be a text String
  # (encoded as UTF-8) or already-raw ASCII-8BIT bytes, which are used as-is; it may be empty.
  #
  # @raise [TypeError] if +namespace+ isn't a HyperUuid::Uuid or +name+ isn't a String.
  def self.new_v5(namespace, name)
    raise TypeError, "namespace must be a HyperUuid::Uuid; got #{namespace.class}" unless namespace.is_a?(Uuid)
    raise TypeError, "name must be a String; got #{name.class}" unless name.is_a?(String)

    name_bytes =
      if name.encoding == Encoding::ASCII_8BIT
        name
      else
        name.encode(Encoding::UTF_8).dup.force_encoding(Encoding::BINARY)
      end
    raise ArgumentError, "name must be at most #{U32_MAX} bytes" if name_bytes.bytesize > U32_MAX

    Uuid.new(Runtime.new_v5(namespace.bytes, name_bytes), true)
  end

  # Converts +value+ to a Unix-epoch millisecond integer: +nil+ becomes the current time, a
  # +Time+ is converted exactly (via its own Rational seconds, avoiding float rounding), and
  # an Integer millisecond count passes through unchanged. Shared by every
  # `new_v6`/`new_v7`/batch door below so a caller can pass either a `Time` or a raw
  # millisecond count interchangeably.
  #
  # This is also where a caller bug is caught once, for every backend: anything else is a
  # TypeError, and a count that cannot cross the ABI as a u64 at all — negative (a `Time`
  # before the epoch included) or past 64 bits — is the same TimestampOutOfRangeError, with
  # the same +out_of_range+ message, the core raises for a value it can see but not embed.
  # Left to the backends those cases diverged: a RangeError from the extension, a wrapped
  # value from Fiddle.
  #
  # The two common arguments go first and cheapest: no argument is the clock, which needs no
  # range check, and an Integer is checked by its bit length rather than against 2**64 - 1: a
  # comparison with a number that size is a bignum comparison, and cost more than the mint.
  private_class_method def self.unix_millis_from(value, out_of_range)
    return Process.clock_gettime(Process::CLOCK_REALTIME, :millisecond) if value.nil?

    millis =
      if value.is_a?(Integer) then value
      elsif value.is_a?(Time) then (value.to_r * 1000).floor
      else raise TypeError, "unix_millis must be a Time, an Integer or nil; got #{value.class}"
      end
    raise TimestampOutOfRangeError, out_of_range unless millis >= 0 && millis.bit_length <= 64

    millis
  end

  # Checks a batch +count+ once, for every backend: an Integer the ABI's u32 can carry.
  private_class_method def self.batch_count(count)
    raise TypeError, "count must be an Integer; got #{count.class}" unless count.is_a?(Integer)
    raise ArgumentError, "count must be between 0 and #{U32_MAX}; got #{count}" unless count.between?(0, U32_MAX)

    count
  end

  # Creates a time-sortable UUID version 6 (RFC 9562 §5.6), a field-compatible reordering of
  # version 1 for better sort/index locality. Defaults to the current time; pass an explicit
  # `Time` or Unix-epoch millisecond integer to embed a specific time instead. `clock_seq` and
  # `node` are randomly generated on every call — unlike version 7, there is no monotonic
  # counter, so calls within the same millisecond are not guaranteed to sort in creation order.
  #
  # @raise [TimestampOutOfRangeError] if the time is negative or past the 60-bit v6 field.
  # @raise [TypeError] if +unix_millis+ isn't a Time, an Integer or nil.
  def self.new_v6(unix_millis = nil)
    Uuid.new(Runtime.new_v6(unix_millis_from(unix_millis, Runtime::V6_TIMESTAMP_OUT_OF_RANGE)), true)
  end

  # Creates `count` time-sortable version 6 UUIDs sharing one timestamp capture — one FFI call
  # and one random-bytes fetch instead of `count` of each. Defaults to the current time; pass
  # an explicit `Time` or Unix-epoch millisecond integer to embed a specific time instead.
  #
  # @raise [TypeError, ArgumentError] if +count+ isn't an Integer between 0 and 2**32 - 1.
  # @raise [TimestampOutOfRangeError] if the time is negative or past the 60-bit v6 field.
  def self.new_v6_batch(count, unix_millis = nil)
    bytes = new_v6_batch_bytes(count, unix_millis)
    Array.new(count) { |i| Uuid.new(bytes[i * 16, 16], true) }
  end

  # Creates a time-sortable UUID version 7 (RFC 9562 §6.2). Defaults to the current time; pass
  # an explicit `Time` or Unix-epoch millisecond integer (non-negative, fitting in 48 bits) to
  # embed a specific time instead.
  #
  # @raise [TimestampOutOfRangeError] if the time is negative or past the 48-bit v7 field.
  # @raise [TypeError] if +unix_millis+ isn't a Time, an Integer or nil.
  def self.new_v7(unix_millis = nil)
    Uuid.new(Runtime.new_v7(unix_millis_from(unix_millis, Runtime::V7_TIMESTAMP_OUT_OF_RANGE)), true)
  end

  # Creates `count` time-sortable version 7 UUIDs sharing one timestamp capture and one
  # contiguous block of the monotonic counter — one FFI call and one random-bytes fetch
  # instead of `count` of each. Defaults to the current time; pass an explicit `Time` or
  # Unix-epoch millisecond integer to embed a specific time instead.
  #
  # @raise [TypeError, ArgumentError] if +count+ isn't an Integer between 0 and 2**32 - 1.
  # @raise [TimestampOutOfRangeError] if the time is negative or past the 48-bit v7 field.
  def self.new_v7_batch(count, unix_millis = nil)
    bytes = new_v7_batch_bytes(count, unix_millis)
    Array.new(count) { |i| Uuid.new(bytes[i * 16, 16], true) }
  end

  # Returns `count` version 7 UUIDs as one binary String of raw RFC 9562-ordered bytes,
  # 16 per UUID, instead of an Array of Uuid objects.
  #
  # Far faster than #new_v7_batch for a large batch — the README's "Bulk generation into
  # bytes" section carries the measured figures. The difference is not the native call —
  # that is identical — it is that #new_v7_batch then allocates `count` Uuid objects and
  # their byte Strings on top of it. This hands back the bytes the native core already
  # produced, untouched.
  #
  # Use it when bytes are the destination: a BYTEA/uniqueidentifier bind parameter, a wire
  # format, a bulk COPY. If you need Uuid objects, keep using #new_v7_batch — slicing this
  # String into them yourself just moves the same allocations into your own code.
  #
  # Slice it with `bytes[i * 16, 16]`, which is what #new_v7_batch does internally.
  #
  # @raise [TypeError, ArgumentError] if +count+ isn't an Integer between 0 and 2**32 - 1.
  # @raise [TimestampOutOfRangeError] if the time is negative or past the 48-bit v7 field.
  def self.new_v7_batch_bytes(count, unix_millis = nil)
    count = batch_count(count)
    Runtime.new_v7_batch(count, unix_millis_from(unix_millis, Runtime::V7_TIMESTAMP_OUT_OF_RANGE))
  end

  # Returns `count` version 6 UUIDs as one binary String of raw RFC 9562-ordered bytes,
  # 16 per UUID. The version 6 counterpart to #new_v7_batch_bytes, with the same rationale and
  # the same guidance about when it is the right call.
  #
  # clock_seq and node are independently random per item; unlike version 7 there is no
  # monotonic counter, so items minted in the same millisecond are not guaranteed to sort in
  # creation order.
  #
  # @raise [TypeError, ArgumentError] if +count+ isn't an Integer between 0 and 2**32 - 1.
  # @raise [TimestampOutOfRangeError] if the time is negative or past the 60-bit v6 field.
  def self.new_v6_batch_bytes(count, unix_millis = nil)
    count = batch_count(count)
    Runtime.new_v6_batch(count, unix_millis_from(unix_millis, Runtime::V6_TIMESTAMP_OUT_OF_RANGE))
  end

  # The version of the native core actually loaded, as "major.minor.patch" — read from the
  # library itself (its `hyperuuid_version` export), not from this gem, so a consumer can
  # prove the core behind the doors is the one this binding was built against (and name the
  # mismatch against HyperUuid::VERSION when it is not). The cheapest possible probe that the
  # backend resolved at all: takes nothing, mints nothing, cannot fail once the backend loads.
  #
  # @raise [LoadError] if no backend could be loaded for this platform.
  def self.native_version
    word = Runtime.packed_version
    "#{word >> 16}.#{(word >> 8) & 0xFF}.#{word & 0xFF}"
  end

  # Whether a backend actually loaded and exports the ABI this binding was built against —
  # what a consumer with a fallback of its own (SecureRandom.uuid, say) checks before
  # committing to these doors. Probed once (a native_version round trip), cached, and never
  # raises: a missing shared library, an unsupported platform, or an older core without the
  # version export all answer false. The doors themselves keep their own behavior — the
  # first call on an unavailable backend raises its precise error — this only answers the
  # question quietly.
  def self.available?
    return @available unless @available.nil?

    @available = begin
      native_version.is_a?(String)
    rescue LoadError, StandardError
      false
    end
  end
end

# --- backend selection: the Magnus extension, when present, replaces the Runtime methods
# above in place (no delegation layer) — Fiddle's measured per-call marshalling floor drops
# to an ordinary extension call, while everything above Runtime (Uuid, the module doors,
# batch slicing) stays shared byte-for-byte between backends. The pure-Fiddle definitions
# remain the universal zero-compile fallback — the last resort, for a Ruby or a platform no
# platform gem covers; precompiled platform gems are how the extension ships without ever
# making a consumer compile anything, and they carry no Fiddle library at all.
#
# HYPERUUID_PURE forces Fiddle. It is a testing and diagnostic switch — CI runs the whole suite
# through it, and it is how a suspected extension bug is ruled in or out — not a setting a
# deployment needs. It is read for presence, not value, so "0" and the empty string force it
# too. Inside a platform gem there is no library for it to load: BACKEND still reads :fiddle,
# HyperUuid.available? answers false, and the first door call raises a LoadError naming the
# universal gem, which is where the Fiddle backend lives.
HyperUuid::BACKEND =
  if ENV["HYPERUUID_PURE"]
    :fiddle
  else
    # Two layouts, and both have to work. A released platform gem is a "fat" gem carrying one
    # extension per supported Ruby ABI under lib/hyperuuid/<minor>/ (see the Rakefile's
    # native:gem task for why an ABI-per-file is unavoidable — Magnus has no `abi3`
    # equivalent), and `rake native:dev` — the local dev loop — stages its own build at that
    # same versioned path. CI's in-job staging additionally drops a single extension flat at
    # lib/, which is also where a hand copy of `cargo ruby-ext`'s output goes (the alias only
    # builds into rust/target/ruby/release/; nothing but that rake task copies it here).
    # Trying the versioned path first and the flat one second means neither has to know the
    # other exists.
    #
    # A miss on both is not an error: it means this Ruby/platform combination has no
    # precompiled extension, which is precisely what the Fiddle backend below is for. A
    # platform with no shared library either still lands on Fiddle, so the first call raises
    # its own precise error — a "not found" LoadError naming the path, or
    # NativePlatform::UnsupportedPlatformError naming the platform. HyperUuid.available? is
    # the quiet way to ask.
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
