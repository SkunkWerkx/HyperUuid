package io.github.skunkwerkx.hyperuuid;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.ByteOrder;
import java.time.Instant;
import java.util.Optional;
import java.util.UUID;
import org.graalvm.polyglot.Context;
import org.graalvm.polyglot.Source;
import org.graalvm.polyglot.Value;
import org.graalvm.polyglot.io.ByteSequence;

/**
 * The Rust core as a {@code wasm32-wasip1} module, run inside the JVM by
 * <a href="https://www.graalvm.org/webassembly/">GraalWasm</a>. No native binary, no
 * {@code java.lang.foreign} downcall: the eighteen {@code uuid_*} functions and
 * {@code hyperuuid_version} that {@link UuidGenerator} downcalls into natively are called
 * through the polyglot API instead, on the module bundled at
 * {@code /native/wasm32-wasip1/hyperuuid.wasm}.
 *
 * <p><b>Memory protocol.</b> A wasm guest only sees its own linear memory, so nothing here
 * can hand the core a pointer into a Java array the way the FFM path pins a {@code byte[]}.
 * Every buffer the core fills is obtained from the module's own exported {@code malloc}
 * (wasi-libc's, which is also what Rust's allocator sits on for this target) and read back
 * out through the exported {@code memory}. Using the guest allocator rather than a
 * host-picked offset is load-bearing: dlmalloc claims the tail of the initial memory on
 * first use, and a batch written at a "free-looking" offset past the data segments was
 * observed being corrupted by the very next allocation. Two 16-byte scratch buffers are
 * allocated once for the single-UUID doors; variable-size inputs and batch destinations use
 * one grow-only buffer that is only ever reallocated upward.
 *
 * <p><b>Threading.</b> One {@link Context} and one module instance serve the whole process,
 * and every call is serialized on this object's monitor — a polyglot context does not
 * permit concurrent multi-threaded access, and one instance is also what keeps the core's
 * process-wide v7 counter one sequence rather than one per thread. The FFM path has no such
 * lock; that is one of the real costs of this backend, alongside the per-call price
 * measured in the README.
 *
 * <p><b>WASI.</b> The module imports five {@code wasi_snapshot_preview1} functions
 * ({@code random_get} for entropy, plus {@code environ_*}, {@code fd_write} and
 * {@code proc_exit} from wasi-libc's startup and panic paths). GraalWasm's built-in preview 1
 * implementation supplies them ({@code wasm.Builtins=wasi_snapshot_preview1}); nothing is
 * preopened, so the guest sees no files, no environment and no arguments.
 *
 * <p>Instantiated by name from {@link UuidGenerator} so that {@code org.graalvm.polyglot} is
 * only ever loaded when this backend was selected.
 */
final class WasmBackend implements Backend {
    static final String RESOURCE_PATH = "/native/wasm32-wasip1/hyperuuid.wasm";

    // What a consumer who selected this backend without its optional dependencies is told.
    // A compile-time constant, so UuidGenerator can name it without loading this class.
    static final String GRAALWASM_MISSING = "hyperuuid: the wasm backend needs GraalWasm on the "
            + "classpath — add org.graalvm.polyglot:polyglot and org.graalvm.polyglot:wasm "
            + "(the latter is a POM-type dependency)";

    private static final ByteOrder BIG_ENDIAN = ByteOrder.BIG_ENDIAN;

    private final Context context;
    private final Value exports;
    private final Value memory;

    // Each export resolved once. Measured under GraalVM's JIT: a cached Value's execute()
    // plus one 16-byte readBuffer is the cheapest shape of this call (137 ns for uuid_new_v7
    // including the monitor), against 306 ns for invokeMember by name plus two readBufferLong
    // reads — the name lookup and each extra crossing are what cost, not the wasm.
    private final Value mallocFn;
    private final Value freeFn;
    private final Value newV4Fn;
    private final Value newV5Fn;
    private final Value newV6Fn;
    private final Value v6UnixMillisFn;
    private final Value newV6BatchFn;
    private final Value newV7Fn;
    private final Value v7UnixMillisFn;
    private final Value newV7BatchFn;
    private final Value v7ToSqlOrderFn;
    private final Value v7ToRfcOrderFn;
    private final Value v6ToSqlOrderFn;
    private final Value v6ToRfcOrderFn;
    private final Value versionFn;
    private final Value uuidVersionFn;
    private final Value uuidVariantFn;
    private final Value uuidIsRfcFn;
    private final Value getTimestampFn;
    private final Value v6UnixMillisInFn;
    private final Value v7UnixMillisInFn;

    // Sixteen bytes read back from the guest per single-UUID door; guarded by the same
    // monitor as every call, so one array serves the process.
    private final byte[] readback = new byte[16];

    // 16-byte in and out scratch for the single-UUID doors, allocated once for the process.
    private final int scratchIn;
    private final int scratchOut;

    // One grow-only buffer for everything variable-sized: a v5 name, a batch destination.
    private int bulkPtr;
    private int bulkCapacity;

    WasmBackend() {
        byte[] module;
        try (InputStream in = WasmBackend.class.getResourceAsStream(RESOURCE_PATH)) {
            if (in == null) {
                throw new IllegalStateException(
                        RESOURCE_PATH + " classpath resource not found (this jar was built without the wasm module)");
            }
            module = in.readAllBytes();
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
        try {
            context = Context.newBuilder("wasm")
                    .option("wasm.Builtins", "wasi_snapshot_preview1")
                    .build();
        } catch (IllegalArgumentException | IllegalStateException noWasmLanguage) {
            // The polyglot API is on the classpath but nothing behind it can run wasm: the
            // half-added dependency, org.graalvm.polyglot:polyglot without :wasm.
            throw new IllegalStateException(GRAALWASM_MISSING, noWasmLanguage);
        }
        Value instance = context.eval(Source.newBuilder("wasm", ByteSequence.create(module), "hyperuuid")
                        .buildLiteral())
                .newInstance();
        exports = instance.getMember("exports");
        memory = exports.getMember("memory");
        mallocFn = export("malloc");
        freeFn = export("free");
        newV4Fn = export("uuid_new_v4");
        newV5Fn = export("uuid_new_v5");
        newV6Fn = export("uuid_new_v6");
        v6UnixMillisFn = export("uuid_v6_unix_millis");
        newV6BatchFn = export("uuid_new_v6_batch");
        newV7Fn = export("uuid_new_v7");
        v7UnixMillisFn = export("uuid_v7_unix_millis");
        newV7BatchFn = export("uuid_new_v7_batch");
        v7ToSqlOrderFn = export("uuid_v7_to_sql_order");
        v7ToRfcOrderFn = export("uuid_v7_to_rfc_order");
        v6ToSqlOrderFn = export("uuid_v6_to_sql_order");
        v6ToRfcOrderFn = export("uuid_v6_to_rfc_order");
        versionFn = export("hyperuuid_version");
        uuidVersionFn = export("uuid_version");
        uuidVariantFn = export("uuid_variant");
        uuidIsRfcFn = export("uuid_is_rfc");
        getTimestampFn = export("uuid_get_timestamp");
        v6UnixMillisInFn = export("uuid_v6_unix_millis_in");
        v7UnixMillisInFn = export("uuid_v7_unix_millis_in");
        scratchIn = malloc(16);
        scratchOut = malloc(16);
    }

    private Value export(String name) {
        Value fn = exports.getMember(name);
        if (fn == null || !fn.canExecute()) {
            throw new IllegalStateException("hyperuuid: wasm module does not export " + name);
        }
        return fn;
    }

    @Override
    public String name() {
        return "wasm";
    }

    // ---- guest memory -------------------------------------------------------------------

    private int malloc(int size) {
        int ptr = mallocFn.execute(size).asInt();
        if (ptr == 0) {
            throw new IllegalStateException("hyperuuid: wasm guest malloc(" + size + ") returned NULL");
        }
        return ptr;
    }

    /** A guest buffer of at least {@code size} bytes, reused across calls and only ever grown. */
    private int bulk(int size) {
        if (size > bulkCapacity) {
            if (bulkPtr != 0) {
                freeFn.execute(bulkPtr);
                bulkPtr = 0;
                bulkCapacity = 0;
            }
            bulkPtr = malloc(size);
            bulkCapacity = size;
        }
        return bulkPtr;
    }

    private void writeUuid(int ptr, UUID uuid) {
        memory.writeBufferLong(BIG_ENDIAN, ptr, uuid.getMostSignificantBits());
        memory.writeBufferLong(BIG_ENDIAN, ptr + 8, uuid.getLeastSignificantBits());
    }

    private UUID readUuid(int ptr) {
        memory.readBuffer(ptr, readback, 0, 16);
        return RfcBytes.fromRfcBytes(readback, 0);
    }

    private void writeBytes(int ptr, byte[] bytes) {
        // The polyglot buffer interface has a bulk read but no bulk write; eight bytes at a
        // time is the widest single write it offers.
        int i = 0;
        for (; i + 8 <= bytes.length; i += 8) {
            long v = 0;
            for (int j = 0; j < 8; j++) {
                v = (v << 8) | (bytes[i + j] & 0xFF);
            }
            memory.writeBufferLong(BIG_ENDIAN, ptr + i, v);
        }
        for (; i < bytes.length; i++) {
            memory.writeBufferByte(ptr + i, bytes[i]);
        }
    }

    private static int call(Value fn, Object... args) {
        return fn.execute(args).asInt();
    }

    // ---- the version probe, and the eighteen exports with UuidGenerator's exact error contract

    @Override
    public synchronized int version() {
        return versionFn.execute().asInt();
    }

    @Override
    public synchronized UUID newV4() {
        int rc = call(newV4Fn, scratchOut);
        if (rc != 0) {
            throw new IllegalStateException("uuid_new_v4 failed with code " + rc + " (random source failure)");
        }
        return readUuid(scratchOut);
    }

    @Override
    public synchronized UUID newV5(UUID namespace, byte[] name) {
        writeUuid(scratchIn, namespace);
        // A zero-length name is the ABI's NULL; the guest never dereferences it for length 0.
        int namePtr = 0;
        if (name.length != 0) {
            namePtr = bulk(name.length);
            writeBytes(namePtr, name);
        }
        int rc = call(newV5Fn, scratchIn, namePtr, name.length, scratchOut);
        if (rc != 0) {
            throw new IllegalStateException("uuid_new_v5 failed with code " + rc);
        }
        return readUuid(scratchOut);
    }

    @Override
    public synchronized UUID newV6(long unixMillis) {
        int rc = call(newV6Fn, unixMillis, scratchOut);
        if (rc == 2) {
            throw new IllegalArgumentException("unixMillis does not fit the 60-bit v6 timestamp field");
        }
        if (rc != 0) {
            throw new IllegalStateException("uuid_new_v6 failed with code " + rc + " (random source failure)");
        }
        return readUuid(scratchOut);
    }

    @Override
    public synchronized long v6UnixMillis(UUID uuid) {
        writeUuid(scratchIn, uuid);
        return v6UnixMillisFn.execute(scratchIn).asLong();
    }

    @Override
    public synchronized UUID[] newV6Batch(int count, long unixMillis) {
        if (count == 0) {
            return new UUID[0];
        }
        int out = fillBatch(
                newV6BatchFn,
                "uuid_new_v6_batch",
                count,
                unixMillis,
                "unixMillis does not fit the 60-bit v6 timestamp field");
        return readUuids(out, count, new UUID[count]);
    }

    @Override
    public synchronized UUID newV7(long unixMillis) {
        int rc = call(newV7Fn, unixMillis, scratchOut);
        if (rc == 2) {
            throw new IllegalArgumentException("unixMillis must be non-negative and fit within 48 bits");
        }
        if (rc != 0) {
            throw new IllegalStateException("uuid_new_v7 failed with code " + rc + " (random source failure)");
        }
        return readUuid(scratchOut);
    }

    @Override
    public synchronized long v7UnixMillis(UUID uuid) {
        writeUuid(scratchIn, uuid);
        return v7UnixMillisFn.execute(scratchIn).asLong();
    }

    @Override
    public synchronized UUID[] newV7Batch(int count, long unixMillis) {
        if (count == 0) {
            return new UUID[0];
        }
        int out = fillBatch(
                newV7BatchFn,
                "uuid_new_v7_batch",
                count,
                unixMillis,
                "unixMillis must be non-negative and fit within 48 bits");
        return readUuids(out, count, new UUID[count]);
    }

    @Override
    public synchronized UUID v7ToSqlOrder(UUID uuid) {
        return rewrite(v7ToSqlOrderFn, uuid);
    }

    @Override
    public synchronized UUID v7FromSqlOrder(UUID uuid) {
        return rewrite(v7ToRfcOrderFn, uuid);
    }

    @Override
    public synchronized UUID v6ToSqlOrder(UUID uuid) {
        return rewrite(v6ToSqlOrderFn, uuid);
    }

    @Override
    public synchronized UUID v6FromSqlOrder(UUID uuid) {
        return rewrite(v6ToRfcOrderFn, uuid);
    }

    @Override
    public synchronized void fillV7(UUID[] destination, long unixMillis) {
        if (destination.length == 0) {
            return;
        }
        int out = fillBatch(
                newV7BatchFn,
                "uuid_new_v7_batch",
                destination.length,
                unixMillis,
                "unixMillis does not fit this version's timestamp field");
        readUuids(out, destination.length, destination);
    }

    @Override
    public synchronized void fillV6(UUID[] destination, long unixMillis) {
        if (destination.length == 0) {
            return;
        }
        int out = fillBatch(
                newV6BatchFn,
                "uuid_new_v6_batch",
                destination.length,
                unixMillis,
                "unixMillis does not fit this version's timestamp field");
        readUuids(out, destination.length, destination);
    }

    @Override
    public synchronized void fillV7(byte[] destination, long unixMillis) {
        fillBytes(newV7BatchFn, "uuid_new_v7_batch", destination, unixMillis);
    }

    @Override
    public synchronized void fillV6(byte[] destination, long unixMillis) {
        fillBytes(newV6BatchFn, "uuid_new_v6_batch", destination, unixMillis);
    }

    @Override
    public synchronized void v7ToSqlOrder(byte[] uuid) {
        rewrite(v7ToSqlOrderFn, uuid);
    }

    @Override
    public synchronized void v7FromSqlOrder(byte[] uuid) {
        rewrite(v7ToRfcOrderFn, uuid);
    }

    @Override
    public synchronized void v6ToSqlOrder(byte[] uuid) {
        rewrite(v6ToSqlOrderFn, uuid);
    }

    @Override
    public synchronized void v6FromSqlOrder(byte[] uuid) {
        rewrite(v6ToRfcOrderFn, uuid);
    }

    @Override
    public synchronized int uuidVersion(UUID uuid, int layout) {
        writeUuid(scratchIn, uuid);
        return call(uuidVersionFn, scratchIn, layout);
    }

    @Override
    public synchronized int uuidVersion(byte[] uuid, int layout) {
        writeBytes(scratchIn, uuid);
        return call(uuidVersionFn, scratchIn, layout);
    }

    @Override
    public synchronized int uuidVariant(UUID uuid) {
        writeUuid(scratchIn, uuid);
        return call(uuidVariantFn, scratchIn);
    }

    @Override
    public synchronized int uuidVariant(byte[] uuid) {
        writeBytes(scratchIn, uuid);
        return call(uuidVariantFn, scratchIn);
    }

    @Override
    public synchronized boolean isRfc(UUID uuid, int version, int layout) {
        writeUuid(scratchIn, uuid);
        return call(uuidIsRfcFn, scratchIn, version, layout) != 0;
    }

    @Override
    public synchronized boolean isRfc(byte[] uuid, int version, int layout) {
        writeBytes(scratchIn, uuid);
        return call(uuidIsRfcFn, scratchIn, version, layout) != 0;
    }

    @Override
    public synchronized Optional<Instant> getTimestamp(UUID uuid, int layout) {
        writeUuid(scratchIn, uuid);
        // The u64 lands in the out scratch, in the guest's own (little-endian) byte order.
        if (call(getTimestampFn, scratchIn, layout, scratchOut) == 0) {
            return Optional.empty();
        }
        return Optional.of(Instant.ofEpochMilli(memory.readBufferLong(ByteOrder.LITTLE_ENDIAN, scratchOut)));
    }

    @Override
    public synchronized long v6UnixMillisIn(UUID uuid, int layout) {
        writeUuid(scratchIn, uuid);
        return v6UnixMillisInFn.execute(scratchIn, layout).asLong();
    }

    @Override
    public synchronized long v7UnixMillisIn(UUID uuid, int layout) {
        writeUuid(scratchIn, uuid);
        return v7UnixMillisInFn.execute(scratchIn, layout).asLong();
    }

    // ---- shared shapes ------------------------------------------------------------------

    /** Runs a batch export into the bulk buffer and returns its guest address. */
    private int fillBatch(Value fn, String name, int count, long unixMillis, String outOfRangeMessage) {
        // UuidGenerator has already checked this for every caller that takes a count; held
        // here too because this is the line that multiplies, and the guest would take a
        // wrapped product as a buffer far smaller than the batch it then writes.
        UuidGenerator.requireBatchCount(count);
        int out = bulk(count * 16);
        int rc = call(fn, unixMillis, count, out);
        if (rc != 0) {
            throw UuidGenerator.batchFailure(rc, name, count, outOfRangeMessage);
        }
        return out;
    }

    // One crossing for the whole batch, then plain Java over the bytes: a thousand
    // sixteen-byte reads would be a thousand host-to-guest calls.
    private UUID[] readUuids(int ptr, int count, UUID[] into) {
        byte[] bytes = new byte[count * 16];
        memory.readBuffer(ptr, bytes, 0, bytes.length);
        for (int i = 0; i < count; i++) {
            into[i] = RfcBytes.fromRfcBytes(bytes, i * 16);
        }
        return into;
    }

    private void fillBytes(Value fn, String name, byte[] destination, long unixMillis) {
        if (destination.length % 16 != 0) {
            throw new IllegalArgumentException(
                    "destination length must be a multiple of 16 (one whole UUID per 16 bytes); got "
                            + destination.length);
        }
        if (destination.length == 0) {
            return;
        }
        int out = fillBatch(
                fn,
                name,
                destination.length / 16,
                unixMillis,
                "unixMillis does not fit this version's timestamp field");
        memory.readBuffer(out, destination, 0, destination.length);
    }

    private UUID rewrite(Value fn, UUID uuid) {
        writeUuid(scratchIn, uuid);
        fn.execute(scratchIn);
        return readUuid(scratchIn);
    }

    private void rewrite(Value fn, byte[] uuid) {
        if (uuid.length != 16) {
            throw new IllegalArgumentException("a UUID is exactly 16 bytes; got " + uuid.length);
        }
        writeBytes(scratchIn, uuid);
        fn.execute(scratchIn);
        memory.readBuffer(scratchIn, uuid, 0, 16);
    }
}
