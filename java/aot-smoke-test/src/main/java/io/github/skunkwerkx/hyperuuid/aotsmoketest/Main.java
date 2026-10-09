package io.github.skunkwerkx.hyperuuid.aotsmoketest;

import io.github.skunkwerkx.hyperuuid.UuidGenerator;
import io.github.skunkwerkx.hyperuuid.UuidLayout;
import io.github.skunkwerkx.hyperuuid.UuidVariant;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.Arrays;
import java.util.Optional;
import java.util.UUID;

/**
 * Native-image smoke test: proves {@code UuidGenerator}'s FFM downcalls survive GraalVM
 * Native Image ahead-of-time compilation to a real native binary — no JVM required to run it.
 * Build and run with {@code ./gradlew :aot-smoke-test:nativeRun}; {@code :aot-smoke-test:run}
 * is the same program on an ordinary JVM.
 */
public final class Main {
    private Main() {}

    /**
     * Calls every public method of {@code UuidGenerator} — the probe, each generator in each
     * of its overloads, the batch and fill forms, and the byte-order conversions in both
     * their {@code UUID} and raw-byte forms, and the version/variant/layout inspection — and
     * fails on the first wrong answer.
     *
     * @param args ignored
     */
    public static void main(String[] args) {
        // The non-throwing gate first: under AOT a missing resource registration would make
        // the core unloadable, and this is the probe a consumer would notice that through.
        System.out.println("available: " + UuidGenerator.isAvailable());
        require(UuidGenerator.isAvailable(), "the core did not load");
        // Which interop path this binary took — "native" (FFM) or "wasm" (GraalWasm) — so a
        // -Dhyperuuid.backend=wasm run is visibly proving the path it claims to.
        System.out.println("backend: " + UuidGenerator.backend());
        // The version probe is the one export linked without the critical option, so it
        // needs its own downcall signature in reachability-metadata.json — this is what
        // proves the registration reached the binary.
        String version = UuidGenerator.nativeVersion();
        System.out.println("version: " + version);
        require(version.matches("\\d+\\.\\d+\\.\\d+"), "expected major.minor.patch, got " + version);

        UUID v4 = UuidGenerator.newV4();
        require(v4.version() == 4, "expected v4 version 4, got " + v4.version());

        UUID v5 = UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "www.example.com");
        UUID expectedV5 = UUID.fromString("2ed6657d-e927-568b-95e1-2665a8aea6a2");
        require(v5.equals(expectedV5), "v5 did not match the RFC 9562 test vector: got " + v5);
        byte[] name = "www.example.com".getBytes(StandardCharsets.UTF_8);
        require(UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, name).equals(expectedV5), "v5 byte[] overload");
        require(
                UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "www.example.com", StandardCharsets.US_ASCII)
                        .equals(expectedV5),
                "v5 charset overload");
        // The empty name is the one input that crosses as the ABI's NULL.
        require(
                UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "")
                        .equals(UUID.fromString("4ebd0208-8328-5d69-8c44-ec50939c0967")),
                "v5 of the empty name");

        long rfcTestVectorMs = 1_645_557_742_000L;
        Instant rfcTestVector = Instant.ofEpochMilli(rfcTestVectorMs);
        UUID v6 = UuidGenerator.newV6(rfcTestVectorMs);
        require(v6.version() == 6, "expected v6 version 6, got " + v6.version());
        require(
                UuidGenerator.v6UnixMillis(v6) == rfcTestVectorMs,
                "v6 timestamp round-trip failed: got " + UuidGenerator.v6UnixMillis(v6));
        require(UuidGenerator.v6Timestamp(UuidGenerator.newV6(rfcTestVector)).equals(rfcTestVector), "v6 Instant");
        require(UuidGenerator.newV6().version() == 6, "v6 at the current time");

        UUID v7 = UuidGenerator.newV7(rfcTestVectorMs);
        require(v7.version() == 7, "expected v7 version 7, got " + v7.version());
        require(
                UuidGenerator.v7UnixMillis(v7) == rfcTestVectorMs,
                "v7 timestamp round-trip failed: got " + UuidGenerator.v7UnixMillis(v7));
        require(UuidGenerator.v7Timestamp(UuidGenerator.newV7(rfcTestVector)).equals(rfcTestVector), "v7 Instant");
        require(UuidGenerator.newV7().version() == 7, "v7 at the current time");

        require(UuidGenerator.getTimestamp(v6).equals(Optional.of(rfcTestVector)), "getTimestamp(v6)");
        require(UuidGenerator.getTimestamp(v7).equals(Optional.of(rfcTestVector)), "getTimestamp(v7)");
        require(UuidGenerator.getTimestamp(v4).isEmpty(), "getTimestamp(v4) should be empty");
        // A 7 nibble under the Microsoft variant (110x) is no RFC version, so no timestamp.
        UUID microsoftV7 =
                new UUID(v7.getMostSignificantBits(), (v7.getLeastSignificantBits() & ~(0x7L << 61)) | (0x6L << 61));
        require(
                UuidGenerator.getTimestamp(microsoftV7).isEmpty(),
                "getTimestamp(Microsoft-variant v7) should be empty");

        require(UuidGenerator.NIL.toString().equals("00000000-0000-0000-0000-000000000000"), "NIL mismatch");
        require(UuidGenerator.MAX.toString().equals("ffffffff-ffff-ffff-ffff-ffffffffffff"), "MAX mismatch");

        UUID[] v7Batch = UuidGenerator.newV7Batch(10, rfcTestVectorMs);
        require(v7Batch.length == 10, "expected 10 batch v7 UUIDs, got " + v7Batch.length);
        require(v7Batch[0].version() == 7, "expected batch v7 version 7, got " + v7Batch[0].version());
        require(UuidGenerator.newV7Batch(3).length == 3, "v7 batch at the current time");

        UUID[] v6Batch = UuidGenerator.newV6Batch(10, rfcTestVectorMs);
        require(v6Batch.length == 10, "expected 10 batch v6 UUIDs, got " + v6Batch.length);
        require(v6Batch[0].version() == 6, "expected batch v6 version 6, got " + v6Batch[0].version());
        require(UuidGenerator.newV6Batch(3).length == 3, "v6 batch at the current time");

        // The fills hand the core a buffer the caller owns: a UUID[] rebuilt from the bytes,
        // and a byte[] pinned and written in place — the heap-access half of the critical
        // downcall registration, which the scratch-segment doors above never touch.
        UUID[] filled = new UUID[4];
        UuidGenerator.fillV7(filled, rfcTestVectorMs);
        require(filled[3].version() == 7 && UuidGenerator.v7UnixMillis(filled[3]) == rfcTestVectorMs, "fillV7(UUID[])");
        UuidGenerator.fillV6(filled, rfcTestVectorMs);
        require(filled[3].version() == 6 && UuidGenerator.v6UnixMillis(filled[3]) == rfcTestVectorMs, "fillV6(UUID[])");
        UuidGenerator.fillV7(filled);
        require(filled[0].version() == 7, "fillV7(UUID[]) at the current time");
        UuidGenerator.fillV6(filled);
        require(filled[0].version() == 6, "fillV6(UUID[]) at the current time");

        byte[] raw = new byte[4 * 16];
        UuidGenerator.fillV7(raw, rfcTestVectorMs);
        require(
                uuidAt(raw, 48).version() == 7 && UuidGenerator.v7UnixMillis(uuidAt(raw, 48)) == rfcTestVectorMs,
                "fillV7(byte[])");
        UuidGenerator.fillV6(raw, rfcTestVectorMs);
        require(
                uuidAt(raw, 48).version() == 6 && UuidGenerator.v6UnixMillis(uuidAt(raw, 48)) == rfcTestVectorMs,
                "fillV6(byte[])");
        UuidGenerator.fillV7(raw);
        require(uuidAt(raw, 0).version() == 7, "fillV7(byte[]) at the current time");
        UuidGenerator.fillV6(raw);
        require(uuidAt(raw, 0).version() == 6, "fillV6(byte[]) at the current time");

        // The four order-conversion downcalls share a distinct native signature
        // ((ADDRESS)void) that none of the calls above exercise — GraalVM's FFM reachability
        // metadata is per-signature, not per-function, but a native-image build only
        // registers what its static analysis actually observes being called, so these have
        // to be exercised for real or this smoke test can silently miss exactly the gap it
        // exists to catch.
        UUID v7Sql = UuidGenerator.v7ToSqlOrder(v7);
        require(UuidGenerator.v7FromSqlOrder(v7Sql).equals(v7), "v7 SQL-order round-trip failed");
        UUID v6Sql = UuidGenerator.v6ToSqlOrder(v6);
        require(UuidGenerator.v6FromSqlOrder(v6Sql).equals(v6), "v6 SQL-order round-trip failed");

        // And their raw-byte forms, in place on a caller's own sixteen bytes.
        byte[] v7Bytes = bytesOf(v7);
        UuidGenerator.v7ToSqlOrder(v7Bytes);
        require(Arrays.equals(v7Bytes, bytesOf(v7Sql)), "v7ToSqlOrder(byte[]) disagrees with the UUID form");
        UuidGenerator.v7FromSqlOrder(v7Bytes);
        require(Arrays.equals(v7Bytes, bytesOf(v7)), "v7 raw-byte SQL-order round-trip failed");
        byte[] v6Bytes = bytesOf(v6);
        UuidGenerator.v6ToSqlOrder(v6Bytes);
        require(Arrays.equals(v6Bytes, bytesOf(v6Sql)), "v6ToSqlOrder(byte[]) disagrees with the UUID form");
        UuidGenerator.v6FromSqlOrder(v6Bytes);
        require(Arrays.equals(v6Bytes, bytesOf(v6)), "v6 raw-byte SQL-order round-trip failed");

        // The inspection exports: three more downcall signatures ((ADDRESS, INT)INT,
        // (ADDRESS, INT, INT)INT, (ADDRESS, INT)LONG), plus (ADDRESS)INT for the variant,
        // over both the scratch segment (UUID forms) and a pinned caller array (byte[] forms).
        require(UuidGenerator.version(v7) == 7, "version(v7)");
        require(UuidGenerator.version(bytesOf(v6)) == 6, "version(byte[])");
        require(UuidGenerator.version(v7Sql, UuidLayout.SQL_SERVER) == 7, "version(v7Sql, SQL_SERVER)");
        require(UuidGenerator.version(bytesOf(v6Sql), UuidLayout.SQL_SERVER) == 6, "version(byte[], SQL_SERVER)");
        require(UuidGenerator.variant(v4) == UuidVariant.RFC_9562, "variant(v4)");
        require(UuidGenerator.variant(bytesOf(UuidGenerator.MAX)) == UuidVariant.FUTURE, "variant(byte[] MAX)");
        require(UuidGenerator.isRfc(v5, 5) && !UuidGenerator.isRfc(v5, 4), "isRfc(v5)");
        require(UuidGenerator.isRfc(v6Sql, 6, UuidLayout.SQL_SERVER), "isRfc(v6Sql, 6, SQL_SERVER)");
        require(UuidGenerator.isRfc(bytesOf(v7Sql), 7, UuidLayout.SQL_SERVER), "isRfc(byte[], 7, SQL_SERVER)");
        require(UuidGenerator.v7UnixMillis(v7Sql, UuidLayout.SQL_SERVER) == rfcTestVectorMs, "v7UnixMillis(SQL)");
        require(UuidGenerator.v6UnixMillis(v6Sql, UuidLayout.SQL_SERVER) == rfcTestVectorMs, "v6UnixMillis(SQL)");
        require(UuidGenerator.v7Timestamp(v7Sql, UuidLayout.SQL_SERVER).equals(rfcTestVector), "v7Timestamp(SQL)");
        require(UuidGenerator.v6Timestamp(v6Sql, UuidLayout.SQL_SERVER).equals(rfcTestVector), "v6Timestamp(SQL)");
        require(
                UuidGenerator.getTimestamp(v7Sql, UuidLayout.SQL_SERVER).equals(Optional.of(rfcTestVector)),
                "getTimestamp(v7Sql, SQL_SERVER)");
        require(
                UuidGenerator.getTimestamp(UuidGenerator.NIL, UuidLayout.SQL_SERVER)
                        .isEmpty(),
                "getTimestamp(NIL, SQL_SERVER) should be empty");
        try {
            UuidGenerator.newV7Batch(UuidGenerator.MAX_V7_BATCH + 1, rfcTestVectorMs);
            throw new AssertionError("a v7 batch past MAX_V7_BATCH was not refused");
        } catch (IllegalArgumentException expected) {
            require(
                    expected.getMessage().contains(Integer.toString(UuidGenerator.MAX_V7_BATCH)),
                    "batch limit message");
        }

        System.out.println("hyperuuid AOT smoke test passed: v4=" + v4 + " v5=" + v5 + " v6=" + v6 + " v7="
                + v7 + " v7Batch[0]=" + v7Batch[0] + " v6Batch[0]=" + v6Batch[0] + " v7Sql=" + v7Sql
                + " v6Sql=" + v6Sql);
    }

    // java.util.UUID's two longs are RFC 9562 byte order already, most significant first.
    private static byte[] bytesOf(UUID uuid) {
        return ByteBuffer.allocate(16)
                .putLong(uuid.getMostSignificantBits())
                .putLong(uuid.getLeastSignificantBits())
                .array();
    }

    private static UUID uuidAt(byte[] bytes, int offset) {
        ByteBuffer buffer = ByteBuffer.wrap(bytes, offset, 16);
        return new UUID(buffer.getLong(), buffer.getLong());
    }

    private static void require(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }
}
