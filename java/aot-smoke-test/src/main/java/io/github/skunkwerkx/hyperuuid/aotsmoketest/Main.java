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
        UUID v7 = UuidGenerator.newV7(rfcTestVectorMs);
        require(v7.version() == 7, "expected v7 version 7, got " + v7.version());
        require(
                UuidGenerator.v7UnixMillis(v7) == rfcTestVectorMs,
                "v7 timestamp round-trip failed: got " + UuidGenerator.v7UnixMillis(v7));
        require(UuidGenerator.v7Timestamp(UuidGenerator.newV7(rfcTestVector)).equals(rfcTestVector), "v7 Instant");
        require(UuidGenerator.newV7().version() == 7, "v7 at the current time");

        // RFC 9562 Appendix A.5's version 6: only version 7 timestamps are read, so it has none.
        UUID v6 = UUID.fromString("1ec9414c-232a-6b00-b3c8-9f6bdeced846");
        require(UuidGenerator.getTimestamp(v6).isEmpty(), "getTimestamp(v6) should be empty");
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

        // The fills hand the core a buffer the caller owns: a UUID[] rebuilt from the bytes,
        // and a byte[] pinned and written in place — the heap-access half of the critical
        // downcall registration, which the scratch-segment doors above never touch.
        UUID[] filled = new UUID[4];
        UuidGenerator.fillV7(filled, rfcTestVectorMs);
        require(filled[3].version() == 7 && UuidGenerator.v7UnixMillis(filled[3]) == rfcTestVectorMs, "fillV7(UUID[])");
        UuidGenerator.fillV7(filled);
        require(filled[0].version() == 7, "fillV7(UUID[]) at the current time");

        byte[] raw = new byte[4 * 16];
        UuidGenerator.fillV7(raw, rfcTestVectorMs);
        require(
                uuidAt(raw, 48).version() == 7 && UuidGenerator.v7UnixMillis(uuidAt(raw, 48)) == rfcTestVectorMs,
                "fillV7(byte[])");
        UuidGenerator.fillV7(raw);
        require(uuidAt(raw, 0).version() == 7, "fillV7(byte[]) at the current time");

        // The two order-conversion downcalls ((ADDRESS)INT, linked through their own handle)
        // — a native-image build only registers what its static analysis actually observes
        // being called, so these have to be exercised for real or this smoke test can
        // silently miss exactly the gap it exists to catch. Their refusals too: a non-zero
        // return code has to come back as the binding's exception, with the bytes untouched.
        UUID v7Sql = UuidGenerator.v7ToSqlOrder(v7);
        require(UuidGenerator.v7FromSqlOrder(v7Sql).equals(v7), "v7 SQL-order round-trip failed");
        // Fixed inputs only: a random RFC-ordered v7 carries the SQL layout's 8 one time in 16,
        // and a random SQL-ordered one passes for an RFC v7 as often, so neither is refused
        // reliably. RFC 9562 A.6's v7 and its SQL form are refused every time.
        UUID rfcA6 = UUID.fromString("017f22e2-79b0-7cc3-98c4-dc0c0c07398f");
        UUID rfcA6Sql = UUID.fromString("0c07398f-dc0c-8c84-b30d-017f22e279b0");
        require(UuidGenerator.v7ToSqlOrder(rfcA6).equals(rfcA6Sql), "RFC A.6 v7 SQL form");
        try {
            UuidGenerator.v7FromSqlOrder(rfcA6);
            throw new IllegalStateException("v7FromSqlOrder accepted an RFC-ordered v7");
        } catch (IllegalArgumentException expected) {
            require(
                    "the value is not a UUID of the version this conversion takes".equals(expected.getMessage()),
                    "v7FromSqlOrder refusal message");
        }
        try {
            UuidGenerator.v7ToSqlOrder(rfcA6Sql);
            throw new IllegalStateException("v7ToSqlOrder accepted an already SQL-ordered v7");
        } catch (IllegalArgumentException expected) {
            require(
                    "the value is not a UUID of the version this conversion takes".equals(expected.getMessage()),
                    "v7ToSqlOrder refusal message");
        }
        byte[] refused = bytesOf(v6);
        try {
            UuidGenerator.v7ToSqlOrder(refused);
            throw new IllegalStateException("v7ToSqlOrder(byte[]) accepted a v6");
        } catch (IllegalArgumentException expected) {
            require(Arrays.equals(refused, bytesOf(v6)), "a refused v7ToSqlOrder(byte[]) changed its bytes");
        }

        // And their raw-byte forms, in place on a caller's own sixteen bytes.
        byte[] v7Bytes = bytesOf(v7);
        UuidGenerator.v7ToSqlOrder(v7Bytes);
        require(Arrays.equals(v7Bytes, bytesOf(v7Sql)), "v7ToSqlOrder(byte[]) disagrees with the UUID form");
        UuidGenerator.v7FromSqlOrder(v7Bytes);
        require(Arrays.equals(v7Bytes, bytesOf(v7)), "v7 raw-byte SQL-order round-trip failed");

        // The inspection exports: three more downcall signatures ((ADDRESS, INT)INT,
        // (ADDRESS, INT, INT)INT, (ADDRESS, INT)LONG), plus (ADDRESS)INT for the variant,
        // over both the scratch segment (UUID forms) and a pinned caller array (byte[] forms).
        require(UuidGenerator.version(v7) == 7, "version(v7)");
        require(UuidGenerator.version(bytesOf(v6)) == 6, "version(byte[])");
        require(UuidGenerator.version(v7Sql, UuidLayout.SQL_SERVER) == 7, "version(v7Sql, SQL_SERVER)");
        require(UuidGenerator.version(bytesOf(v7Sql), UuidLayout.SQL_SERVER) == 7, "version(byte[], SQL_SERVER)");
        require(UuidGenerator.variant(v4) == UuidVariant.RFC_9562, "variant(v4)");
        require(UuidGenerator.variant(bytesOf(UuidGenerator.MAX)) == UuidVariant.FUTURE, "variant(byte[] MAX)");
        require(UuidGenerator.isRfc(v5, 5) && !UuidGenerator.isRfc(v5, 4), "isRfc(v5)");
        require(UuidGenerator.isRfc(v7Sql, 7, UuidLayout.SQL_SERVER), "isRfc(v7Sql, 7, SQL_SERVER)");
        require(UuidGenerator.isRfc(bytesOf(v7Sql), 7, UuidLayout.SQL_SERVER), "isRfc(byte[], 7, SQL_SERVER)");
        require(UuidGenerator.v7UnixMillis(v7Sql, UuidLayout.SQL_SERVER) == rfcTestVectorMs, "v7UnixMillis(SQL)");
        require(UuidGenerator.v7Timestamp(v7Sql, UuidLayout.SQL_SERVER).equals(rfcTestVector), "v7Timestamp(SQL)");
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

        System.out.println("hyperuuid AOT smoke test passed: v4=" + v4 + " v5=" + v5 + " v7=" + v7 + " v7Batch[0]="
                + v7Batch[0] + " v7Sql=" + v7Sql);
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
