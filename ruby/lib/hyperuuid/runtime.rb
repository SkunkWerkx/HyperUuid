require "fiddle"

module HyperUuid
  # Fiddle plumbing for the native libhyperuuid shared library — dlopen/dlsym plus a raw
  # C-ABI call, no runtime bridge (the same "no shim" positioning as the Go/Swift bindings'
  # purego/dlopen approach). Fiddle ships with every Ruby install; it's a plain gem
  # dependency here (see hyperuuid.gemspec) rather than a third-party one — mirroring Go's
  # "no cgo" and Python's zero-dependency PyO3 wheels.
  #
  # Unlike the Go/Swift bindings, which embed their native builds inside a single compiled
  # archive and must extract to a temp file before dlopen can see a real path, a Ruby gem's
  # files are already plain files on disk once installed — native/{rid}/{lib} can be
  # dlopen'd directly, no extraction step needed.
  module Runtime
    class RandomSourceError < StandardError; end
    class TimestampOutOfRangeError < StandardError; end

    NATIVE_DIR = File.join(__dir__, "native")

    @mutex = Mutex.new
    @functions = nil

    class << self
      def new_v4
        out = scratch
        rc = functions[:new_v4].call(out)
        raise RandomSourceError, "uuid_new_v4 failed with code #{rc}" unless rc.zero?
        out[0, 16]
      end

      def new_v5(namespace_bytes, name_bytes)
        out = scratch
        # Fiddle passes a String's bytes for void* directly (read-only) — no Pointer
        # wrapper, no copy — the same zero-copy crossing every other input here uses.
        name = name_bytes.empty? ? nil : name_bytes
        rc = functions[:new_v5].call(namespace_bytes, name, name_bytes.bytesize, out)
        raise RandomSourceError, "uuid_new_v5 failed with code #{rc}" unless rc.zero?
        out[0, 16]
      end

      def new_v6(unix_millis)
        out = scratch
        rc = functions[:new_v6].call(unix_millis, out)
        case rc
        when 0 then out[0, 16]
        when 2 then raise TimestampOutOfRangeError, "unix_millis does not fit the 60-bit v6 timestamp field"
        else raise RandomSourceError, "uuid_new_v6 failed with code #{rc}"
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
        when 2 then raise TimestampOutOfRangeError, "unix_millis does not fit the 60-bit v6 timestamp field"
        else raise RandomSourceError, "uuid_new_v6_batch failed with code #{rc}"
        end
      end

      def new_v7(unix_millis)
        out = scratch
        rc = functions[:new_v7].call(unix_millis, out)
        case rc
        when 0 then out[0, 16]
        when 2 then raise TimestampOutOfRangeError, "unix_millis must fit within the RFC 9562 48-bit field"
        else raise RandomSourceError, "uuid_new_v7 failed with code #{rc}"
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
        when 2 then raise TimestampOutOfRangeError, "unix_millis must fit within the RFC 9562 48-bit field"
        else raise RandomSourceError, "uuid_new_v7_batch failed with code #{rc}"
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

      # Whether this platform has a shared library for Fiddle to dlopen at all: a known RID
      # and the file actually present — in this install, or in the in-repo cargo build the
      # dev loop falls back to. Backend selection (hyperuuid.rb) asks this before falling
      # back to the WebAssembly backend, which needs neither.
      def fiddle_library_available?
        !library_path.nil?
      rescue NativePlatform::UnsupportedPlatformError
        false
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

      # Loaded lazily and exactly once, mirroring the Go binding's sync.Once / Swift's lazy
      # static let — the native library and its function pointers live for the process's
      # lifetime, same as every other binding (never dlclose'd). The unsynchronized read is
      # the hot path; the mutex only guards the one-time load (a benign race — idempotent).
      def functions
        @functions || @mutex.synchronize { @functions ||= load_functions }
      end

      def load_functions
        path = library_path
        if path.nil?
          rid, lib_name = NativePlatform.rid_and_library_name
          raise LoadError,
                "hyperuuid: #{File.join(NATIVE_DIR, rid, lib_name)} not found (unsupported " \
                "platform, or this gem was built without a native library for it)"
        end

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
        }
      end
    end
  end
end
