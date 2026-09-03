package io.github.skunkwerkx.hyperuuid;

import java.util.UUID;

/**
 * The one seam between {@link UuidGenerator}'s public surface and a non-FFM way of reaching
 * the Rust core. The FFM downcalls are not behind this: they stay inlined in
 * {@code UuidGenerator} exactly as before, and every public method there only checks one
 * {@code static final} reference against {@code null} before taking its native path. This
 * exists so the wasm implementation can be a separate class ({@link WasmBackend}) that is
 * never loaded, and whose GraalWasm dependency is never touched, unless it was actually
 * selected — the reference {@code UuidGenerator} holds is typed as this interface, and the
 * implementation is instantiated by name.
 *
 * <p>Every method carries the same contract, exceptions and messages as its
 * {@code UuidGenerator} twin; that is what lets the whole test suite run unchanged against
 * either path.
 */
interface Backend {
    /** A short, stable name for diagnostics and tests: {@code "wasm"}. */
    String name();

    UUID newV4();

    UUID newV5(UUID namespace, byte[] name);

    UUID newV6(long unixMillis);

    long v6UnixMillis(UUID uuid);

    UUID[] newV6Batch(int count, long unixMillis);

    UUID newV7(long unixMillis);

    long v7UnixMillis(UUID uuid);

    UUID[] newV7Batch(int count, long unixMillis);

    UUID v7ToSqlOrder(UUID uuid);

    UUID v7FromSqlOrder(UUID uuid);

    UUID v6ToSqlOrder(UUID uuid);

    UUID v6FromSqlOrder(UUID uuid);

    void fillV7(UUID[] destination, long unixMillis);

    void fillV6(UUID[] destination, long unixMillis);

    void fillV7(byte[] destination, long unixMillis);

    void fillV6(byte[] destination, long unixMillis);

    void v7ToSqlOrder(byte[] uuid);

    void v7FromSqlOrder(byte[] uuid);

    void v6ToSqlOrder(byte[] uuid);

    void v6FromSqlOrder(byte[] uuid);
}
