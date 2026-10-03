require "fiddle"

module HyperUuid
  # Fiddle plumbing for the native libhyperuuid shared library — dlopen/dlsym plus a raw
  # C-ABI call, no runtime bridge (the same "no shim" positioning as the Swift binding's
  # dlopen approach). Fiddle ships with every Ruby install; it's a plain gem dependency here
  # (see hyperuuid.gemspec) rather than a third-party one — mirroring Python's
  # zero-dependency PyO3 wheels.
  #
  # A Ruby gem's files are plain files on disk once installed, so native/{rid}/{lib} can be
  # dlopen'd directly, with no extraction step.
  module Runtime
    # The package's two exceptions live on HyperUuid itself (errors.rb). These are the names
    # they had through 0.3.0, kept as aliases of the very same classes so a
    # `rescue HyperUuid::Runtime::TimestampOutOfRangeError` written against an earlier
    # release still catches.
    RandomSourceError = HyperUuid::RandomSourceError
    TimestampOutOfRangeError = HyperUuid::TimestampOutOfRangeError

    # One copy of each out-of-range message for the Fiddle backend and for the
    # doors' own range check in hyperuuid.rb; the Magnus extension (rust/src/ruby_ext.rs)
    # carries the same text, and spec/native_backend_spec.rb pins that the two agree.
    V6_TIMESTAMP_OUT_OF_RANGE = "unix_millis does not fit the 60-bit v6 timestamp field"
    V7_TIMESTAMP_OUT_OF_RANGE = "unix_millis must fit within the RFC 9562 48-bit field"

    NATIVE_DIR = File.join(__dir__, "native")

    @mutex = Mutex.new
    @functions = nil

    class << self
      def new_v4
        out = scratch
        rc = functions[:new_v4].call(out)
        raise random_source_failure("uuid_new_v4") unless rc.zero?
        out[0, 16]
      end

      def new_v5(namespace_bytes, name_bytes)
        out = scratch
        # Fiddle passes a String's own bytes for void* (read-only, no copy of them) — the
        # same crossing every other input here uses. An empty name crosses as it is, a
        # valid pointer with length 0, never as nil: NULL is not part of this export's
        # contract.
        rc = functions[:new_v5].call(namespace_bytes, name_bytes, name_bytes.bytesize, out)
        raise random_source_failure("uuid_new_v5") unless rc.zero?
        out[0, 16]
      end

      def new_v6(unix_millis)
        out = scratch
        rc = functions[:new_v6].call(unix_millis, out)
        case rc
        when 0 then out[0, 16]
        when 2 then raise TimestampOutOfRangeError, V6_TIMESTAMP_OUT_OF_RANGE
        else raise random_source_failure("uuid_new_v6")
        end
      end

      def v6_unix_millis(bytes)
        functions[:v6_unix_millis].call(bytes)
      end

      def new_v6_batch(count, unix_millis)
        return "" if count.zero?
        out = Fiddle::Pointer.malloc(count * 16, Fiddle::RUBY_FREE)
        rc = functions[:new_v6_batch].call(unix_millis, count, out)
        case rc
        when 0 then out[0, count * 16]
        when 2 then raise TimestampOutOfRangeError, V6_TIMESTAMP_OUT_OF_RANGE
        else raise random_source_failure("uuid_new_v6_batch")
        end
      end

      def new_v7(unix_millis)
        out = scratch
        rc = functions[:new_v7].call(unix_millis, out)
        case rc
        when 0 then out[0, 16]
        when 2 then raise TimestampOutOfRangeError, V7_TIMESTAMP_OUT_OF_RANGE
        else raise random_source_failure("uuid_new_v7")
        end
      end

      def v7_unix_millis(bytes)
        functions[:v7_unix_millis].call(bytes)
      end

      def new_v7_batch(count, unix_millis)
        return "" if count.zero?
        out = Fiddle::Pointer.malloc(count * 16, Fiddle::RUBY_FREE)
        rc = functions[:new_v7_batch].call(unix_millis, count, out)
        case rc
        when 0 then out[0, count * 16]
        when 2 then raise TimestampOutOfRangeError, V7_TIMESTAMP_OUT_OF_RANGE
        else raise random_source_failure("uuid_new_v7_batch")
        end
      end

      def v7_to_sql_order(bytes)
        rewrite(:v7_to_sql_order, bytes)
      end

      def v7_to_rfc_order(bytes)
        rewrite(:v7_to_rfc_order, bytes)
      end

      def v6_to_sql_order(bytes)
        rewrite(:v6_to_sql_order, bytes)
      end

      def v6_to_rfc_order(bytes)
        rewrite(:v6_to_rfc_order, bytes)
      end

      # The loaded core's version word, major << 16 | minor << 8 | patch, straight from the
      # library's zero-argument hyperuuid_version export. HyperUuid.native_version unpacks
      # it; like every method here, each backend replaces this one in place.
      def packed_version
        functions[:version].call
      end

      # The one failure every generating export shares, with the one message every backend
      # raises for it: the export's name, and that the system's random source failed.
      def random_source_failure(export)
        RandomSourceError.new("#{export}: the system random source failed")
      end

      private

      # The shared library to dlopen: this install's native/{rid}/{lib}, or — the
      # development loop — the in-repo cargo build, exactly what the other bindings' local
      # staging does (HyperCast's runtime has had this since its first release). Nil when
      # neither exists.
      def library_path
        rid, lib_name = NativePlatform.rid_and_library_name
        path = File.join(NATIVE_DIR, rid, lib_name)
        return path if File.exist?(path)

        repo_build = File.expand_path(File.join(__dir__, "../../../rust/target/release", lib_name))
        File.exist?(repo_build) ? repo_build : nil
      end

      # Why Fiddle found nothing to load. A precompiled platform gem is the one install where
      # that is by design rather than a gap: it carries only its Magnus extensions, and the
      # Fiddle backend is reached there only by forcing it (HYPERUUID_PURE) or because none
      # of its extensions loaded — a gem RubyGems matched to a Ruby it was not built for,
      # such as the glibc Linux gem that `gem install` on RubyGems 3.x picks on Alpine. So
      # that case names its fix, the universal gem, which carries every platform's library,
      # instead of a missing path that reads like a packaging bug. Both arguments are
      # parameters only so the specs can ask for every wording.
      def missing_library_message(gem_platform = Gem.loaded_specs["hyperuuid"]&.platform,
                                  forced = ENV.key?("HYPERUUID_PURE"))
        rid, lib_name = NativePlatform.rid_and_library_name
        missing = File.join(NATIVE_DIR, rid, lib_name)
        if gem_platform && gem_platform.to_s != Gem::Platform::RUBY
          reason =
            if forced
              "HYPERUUID_PURE forces the Fiddle backend (unset it to use the extension)"
            else
              "none of its extensions loads on this Ruby (#{RUBY_VERSION}, #{RUBY_PLATFORM})"
            end
          "hyperuuid: this #{gem_platform} platform gem carries only Magnus extensions, no " \
            "Fiddle library, and #{reason}. The universal gem has the Fiddle backend for every " \
            "platform: `gem install hyperuuid --platform ruby`, or Bundler's force_ruby_platform " \
            "(#{missing} not found)"
        else
          "hyperuuid: #{missing} not found (unsupported platform, or this gem was built " \
            "without a native library for it)"
        end
      end

      # One 16-byte scratch allocation per thread, reused by every single-item call —
      # Fiddle::Pointer.malloc(..., RUBY_FREE) registers a GC finalizer per call, measured
      # (in HyperCast, same mechanism) as the dominant per-call cost by an order of
      # magnitude. Batches keep a per-call buffer: one malloc amortized over `count` IDs.
      def scratch
        Thread.current[:hyperuuid_scratch] ||= Fiddle::Pointer.malloc(16, Fiddle::RUBY_FREE)
      end

      # The in-place byte-order rewrites are the one shape that must copy in: the native
      # call genuinely mutates the buffer, and the input String is frozen.
      def rewrite(symbol, bytes)
        buf = scratch
        buf[0, 16] = bytes
        functions[symbol].call(buf)
        buf[0, 16]
      end

      # Loaded lazily and exactly once, mirroring Java's class initializer / Swift's lazy
      # static let — the native library and its function pointers live for the process's
      # lifetime, same as every other binding (never dlclose'd). The unsynchronized read is
      # the hot path; the mutex only guards the one-time load (a benign race — idempotent).
      def functions
        @functions || @mutex.synchronize { @functions ||= load_functions }
      end

      def load_functions
        path = library_path
        raise LoadError, missing_library_message if path.nil?

        handle = Fiddle.dlopen(path)
        {
          new_v4: Fiddle::Function.new(handle["uuid_new_v4"], [Fiddle::TYPE_VOIDP], Fiddle::TYPE_INT),
          new_v5: Fiddle::Function.new(
            handle["uuid_new_v5"],
            [Fiddle::TYPE_VOIDP, Fiddle::TYPE_VOIDP, Fiddle::TYPE_UINT32_T, Fiddle::TYPE_VOIDP],
            Fiddle::TYPE_INT
          ),
          new_v6: Fiddle::Function.new(
            handle["uuid_new_v6"],
            [Fiddle::TYPE_UINT64_T, Fiddle::TYPE_VOIDP],
            Fiddle::TYPE_INT
          ),
          v6_unix_millis: Fiddle::Function.new(
            handle["uuid_v6_unix_millis"],
            [Fiddle::TYPE_VOIDP],
            Fiddle::TYPE_UINT64_T
          ),
          new_v6_batch: Fiddle::Function.new(
            handle["uuid_new_v6_batch"],
            [Fiddle::TYPE_UINT64_T, Fiddle::TYPE_UINT32_T, Fiddle::TYPE_VOIDP],
            Fiddle::TYPE_INT
          ),
          new_v7: Fiddle::Function.new(
            handle["uuid_new_v7"],
            [Fiddle::TYPE_UINT64_T, Fiddle::TYPE_VOIDP],
            Fiddle::TYPE_INT
          ),
          v7_unix_millis: Fiddle::Function.new(
            handle["uuid_v7_unix_millis"],
            [Fiddle::TYPE_VOIDP],
            Fiddle::TYPE_UINT64_T
          ),
          new_v7_batch: Fiddle::Function.new(
            handle["uuid_new_v7_batch"],
            [Fiddle::TYPE_UINT64_T, Fiddle::TYPE_UINT32_T, Fiddle::TYPE_VOIDP],
            Fiddle::TYPE_INT
          ),
          v7_to_sql_order: Fiddle::Function.new(
            handle["uuid_v7_to_sql_order"],
            [Fiddle::TYPE_VOIDP],
            Fiddle::TYPE_VOID
          ),
          v7_to_rfc_order: Fiddle::Function.new(
            handle["uuid_v7_to_rfc_order"],
            [Fiddle::TYPE_VOIDP],
            Fiddle::TYPE_VOID
          ),
          v6_to_sql_order: Fiddle::Function.new(
            handle["uuid_v6_to_sql_order"],
            [Fiddle::TYPE_VOIDP],
            Fiddle::TYPE_VOID
          ),
          v6_to_rfc_order: Fiddle::Function.new(
            handle["uuid_v6_to_rfc_order"],
            [Fiddle::TYPE_VOIDP],
            Fiddle::TYPE_VOID
          ),
          # The one export that mints nothing: the zero-argument version probe.
          version: Fiddle::Function.new(handle["hyperuuid_version"], [], Fiddle::TYPE_UINT32_T),
        }
      end
    end
  end
end
