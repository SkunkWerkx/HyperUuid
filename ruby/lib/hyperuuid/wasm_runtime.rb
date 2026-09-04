require "wasmtime"

module HyperUuid
  # The WebAssembly backend: the same Rust core, compiled once to a `wasm32-wasip1` module
  # (lib/hyperuuid/native/wasm32-wasip1/hyperuuid.wasm) and run inside this process by the
  # `wasmtime` gem — no per-platform shared library needed, and nothing dlopen'd. This is the
  # inverse of ruby.wasm: not Ruby running inside a wasm sandbox, but a wasm module running
  # inside Ruby.
  #
  # Same integration shape as the Magnus extension: on require (after runtime.rb has defined
  # the Fiddle backend) this redefines the `HyperUuid::Runtime` singleton methods **in place**
  # — no delegation layer, no second surface. Everything above Runtime (the `Uuid` class, the
  # module doors, batch slicing) stays shared byte-for-byte across all three backends, and the
  # same `Runtime::RandomSourceError` / `Runtime::TimestampOutOfRangeError` classes carry the
  # same messages.
  #
  # Two things differ from the native backends, both forced by the sandbox:
  #
  # * A wasm guest only sees its own linear memory, so no Fiddle::Pointer can cross. Every
  #   buffer the core writes into is obtained from the module's own exported `malloc` (the
  #   wasi-libc allocator Rust's std already uses on this target) and read back with
  #   `Memory#read`. Using the guest's allocator rather than a host-picked offset is
  #   load-bearing: dlmalloc claims the tail of the initial memory on first use, and a batch
  #   written at a guessed offset was observed corrupted by the guest's next allocation.
  # * A `Wasmtime::Store` is single-threaded, so every call is serialized under one Mutex.
  #   One shared instance per process is also what keeps the core's v7 counter — which lives
  #   inside the instance — monotonic across threads and batches, exactly as the one dlopen'd
  #   library does natively.
  module Runtime
    WASM_MODULE_PATH = File.join(NATIVE_DIR, "wasm32-wasip1", "hyperuuid.wasm")
    # Development loop: the in-repo cargo build, the same fallback runtime.rb takes for the
    # native library, so `HYPERUUID_WASM=1 bundle exec rspec` needs nothing staged by hand.
    WASM_REPO_BUILD_PATH = File.expand_path(
      File.join(__dir__, "../../../rust/target/wasm32-wasip1/release/hyperuuid.wasm")
    )

    # One instantiated module: the exported functions, the exported memory, and the two
    # 16-byte scratch buffers (one in, one out) the single-item doors reuse for the life of
    # the process. Batches and v5 names get a `malloc`/`free` pair per call instead — one
    # allocation amortized over `count` IDs, the same shape as the Fiddle backend's per-batch
    # Pointer.malloc.
    class WasmInstance
      attr_reader :memory, :in, :out

      EXPORTS = %i[
        malloc free
        uuid_new_v4 uuid_new_v5 uuid_new_v6 uuid_v6_unix_millis uuid_new_v6_batch
        uuid_new_v7 uuid_v7_unix_millis uuid_new_v7_batch
        uuid_v7_to_sql_order uuid_v7_to_rfc_order uuid_v6_to_sql_order uuid_v6_to_rfc_order
      ].freeze

      def initialize(path)
        path = WASM_REPO_BUILD_PATH if !File.exist?(path) && File.exist?(WASM_REPO_BUILD_PATH)
        unless File.exist?(path)
          raise LoadError,
                "hyperuuid: #{path} not found (this gem was built without its WebAssembly module)"
        end

        engine = Wasmtime::Engine.new
        mod = Wasmtime::Module.from_file(engine, path)
        linker = Wasmtime::Linker.new(engine)
        # The module imports five WASI preview1 functions (random_get is the one that
        # matters: it is the core's only entropy source; the rest are wasi-libc's startup
        # plumbing). A default WasiConfig — no stdio, no filesystem, no environment — is all
        # any of them need.
        Wasmtime::WASI::P1.add_to_linker_sync(linker)
        store = Wasmtime::Store.new(engine, wasi_p1_config: Wasmtime::WasiConfig.new)
        instance = linker.instantiate(store, mod)

        @memory = instance.export("memory").to_memory
        @fn = EXPORTS.to_h { |name| [name, instance.export(name.to_s).to_func] }
        @in = malloc(16)
        @out = malloc(16)
      end

      def fn(name)
        @fn.fetch(name)
      end

      def malloc(size)
        ptr = @fn[:malloc].call(size)
        raise NoMemoryError, "hyperuuid: wasm malloc(#{size}) failed" if ptr.zero?
        ptr
      end

      def free(ptr)
        @fn[:free].call(ptr)
      end

      # Runs `block` with a `size`-byte guest buffer, freeing it afterwards even on error.
      def with_buffer(size)
        ptr = malloc(size)
        begin
          yield ptr
        ensure
          free(ptr)
        end
      end
    end

    @wasm_mutex = Mutex.new
    @wasm = nil

    class << self
      def new_v4
        wasm do |w|
          rc = w.fn(:uuid_new_v4).call(w.out)
          raise RandomSourceError, "uuid_new_v4 failed with code #{rc}" unless rc.zero?
          w.memory.read(w.out, 16)
        end
      end

      def new_v5(namespace_bytes, name_bytes)
        wasm do |w|
          w.memory.write(w.in, namespace_bytes)
          # A zero-length name still needs a valid (non-null) pointer inside the guest —
          # the `in` scratch, already holding the namespace, serves; nothing reads past
          # a zero length.
          rc =
            if name_bytes.empty?
              w.fn(:uuid_new_v5).call(w.in, w.in, 0, w.out)
            else
              w.with_buffer(name_bytes.bytesize) do |name|
                w.memory.write(name, name_bytes)
                w.fn(:uuid_new_v5).call(w.in, name, name_bytes.bytesize, w.out)
              end
            end
          raise RandomSourceError, "uuid_new_v5 failed with code #{rc}" unless rc.zero?
          w.memory.read(w.out, 16)
        end
      end

      def new_v6(unix_millis)
        wasm do |w|
          rc = w.fn(:uuid_new_v6).call(i64(unix_millis), w.out)
          case rc
          when 0 then w.memory.read(w.out, 16)
          when 2 then raise TimestampOutOfRangeError, "unix_millis does not fit the 60-bit v6 timestamp field"
          else raise RandomSourceError, "uuid_new_v6 failed with code #{rc}"
          end
        end
      end

      def v6_unix_millis(bytes)
        wasm do |w|
          w.memory.write(w.in, bytes)
          u64(w.fn(:uuid_v6_unix_millis).call(w.in))
        end
      end

      def new_v6_batch(count, unix_millis)
        return "" if count.zero?
        wasm do |w|
          w.with_buffer(count * 16) do |out|
            rc = w.fn(:uuid_new_v6_batch).call(i64(unix_millis), count, out)
            case rc
            when 0 then w.memory.read(out, count * 16)
            when 2 then raise TimestampOutOfRangeError, "unix_millis does not fit the 60-bit v6 timestamp field"
            else raise RandomSourceError, "uuid_new_v6_batch failed with code #{rc}"
            end
          end
        end
      end

      def new_v7(unix_millis)
        wasm do |w|
          rc = w.fn(:uuid_new_v7).call(i64(unix_millis), w.out)
          case rc
          when 0 then w.memory.read(w.out, 16)
          when 2 then raise TimestampOutOfRangeError, "unix_millis must fit within the RFC 9562 48-bit field"
          else raise RandomSourceError, "uuid_new_v7 failed with code #{rc}"
          end
        end
      end

      def v7_unix_millis(bytes)
        wasm do |w|
          w.memory.write(w.in, bytes)
          u64(w.fn(:uuid_v7_unix_millis).call(w.in))
        end
      end

      def new_v7_batch(count, unix_millis)
        return "" if count.zero?
        wasm do |w|
          w.with_buffer(count * 16) do |out|
            rc = w.fn(:uuid_new_v7_batch).call(i64(unix_millis), count, out)
            case rc
            when 0 then w.memory.read(out, count * 16)
            when 2 then raise TimestampOutOfRangeError, "unix_millis must fit within the RFC 9562 48-bit field"
            else raise RandomSourceError, "uuid_new_v7_batch failed with code #{rc}"
            end
          end
        end
      end

      def v7_to_sql_order(bytes)
        rewrite(:uuid_v7_to_sql_order, bytes)
      end

      def v7_to_rfc_order(bytes)
        rewrite(:uuid_v7_to_rfc_order, bytes)
      end

      def v6_to_sql_order(bytes)
        rewrite(:uuid_v6_to_sql_order, bytes)
      end

      def v6_to_rfc_order(bytes)
        rewrite(:uuid_v6_to_rfc_order, bytes)
      end

      private

      # The in-place byte-order rewrites: copy the caller's frozen bytes into the guest,
      # let the core rewrite them there, read the result back.
      def rewrite(export, bytes)
        wasm do |w|
          w.memory.write(w.in, bytes)
          w.fn(export).call(w.in)
          w.memory.read(w.in, 16)
        end
      end

      # Every call holds the one lock: the Store is single-threaded, and the lazily created
      # instance is shared by every thread for the life of the process (never torn down,
      # same as the dlopen'd library natively).
      def wasm
        @wasm_mutex.synchronize do
          @wasm ||= WasmInstance.new(WASM_MODULE_PATH)
          yield @wasm
        end
      end

      # wasm has no unsigned integers: a u64 crosses as an i64. The core's own range checks
      # (48-bit v7, 60-bit v6) still see the exact value the caller passed, they just see
      # it through a two's-complement reinterpretation on the way in and back.
      def i64(value)
        value >= 2**63 ? value - 2**64 : value
      end

      def u64(value)
        value.negative? ? value + 2**64 : value
      end
    end
  end
end
