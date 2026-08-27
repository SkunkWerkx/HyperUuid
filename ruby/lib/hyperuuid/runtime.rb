require "fiddle"

module HyperUuid
  # Fiddle plumbing for the native libhyperuuid shared library — dlopen/dlsym plus a raw
  # C-ABI call, no runtime bridge (the same "no shim" positioning as the Go/Swift bindings'
  # purego/dlopen approach). Fiddle ships with every Ruby install; it's a plain gem
  # dependency here (see hyperuuid.gemspec) rather than a third-party one — mirroring Go's
  # "no cgo" and Python's ctypes-only stance.
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
        out = Fiddle::Pointer.malloc(16, Fiddle::RUBY_FREE)
        rc = functions[:new_v4].call(out)
        raise RandomSourceError, "uuid_new_v4 failed with code #{rc}" unless rc.zero?
        out[0, 16]
      end

      def new_v5(namespace_bytes, name_bytes)
        ns = Fiddle::Pointer.malloc(16, Fiddle::RUBY_FREE)
        ns[0, 16] = namespace_bytes
        name_ptr = name_bytes.empty? ? nil : Fiddle::Pointer.to_ptr(name_bytes)
        out = Fiddle::Pointer.malloc(16, Fiddle::RUBY_FREE)
        rc = functions[:new_v5].call(ns, name_ptr, name_bytes.bytesize, out)
        raise RandomSourceError, "uuid_new_v5 failed with code #{rc}" unless rc.zero?
        out[0, 16]
      end

      def new_v7(unix_millis)
        out = Fiddle::Pointer.malloc(16, Fiddle::RUBY_FREE)
        rc = functions[:new_v7].call(unix_millis, out)
        case rc
        when 0 then out[0, 16]
        when 2 then raise TimestampOutOfRangeError, "unix_millis must fit within the RFC 9562 48-bit field"
        else raise RandomSourceError, "uuid_new_v7 failed with code #{rc}"
        end
      end

      private

      # Loaded lazily and exactly once, mirroring the Go binding's sync.Once / Swift's lazy
      # static let — the native library and its function pointers live for the process's
      # lifetime, same as every other binding (never dlclose'd).
      def functions
        @mutex.synchronize { @functions ||= load_functions }
      end

      def load_functions
        rid, lib_name = NativePlatform.rid_and_library_name
        path = File.join(NATIVE_DIR, rid, lib_name)
        unless File.exist?(path)
          raise LoadError,
                "hyperuuid: #{path} not found (unsupported platform, or this gem was built without a native library for it)"
        end

        handle = Fiddle.dlopen(path)
        {
          new_v4: Fiddle::Function.new(handle["uuid_new_v4"], [Fiddle::TYPE_VOIDP], Fiddle::TYPE_INT),
          new_v5: Fiddle::Function.new(
            handle["uuid_new_v5"],
            [Fiddle::TYPE_VOIDP, Fiddle::TYPE_VOIDP, Fiddle::TYPE_UINT32_T, Fiddle::TYPE_VOIDP],
            Fiddle::TYPE_INT
          ),
          new_v7: Fiddle::Function.new(
            handle["uuid_new_v7"],
            [Fiddle::TYPE_UINT64_T, Fiddle::TYPE_VOIDP],
            Fiddle::TYPE_INT
          ),
        }
      end
    end
  end
end
