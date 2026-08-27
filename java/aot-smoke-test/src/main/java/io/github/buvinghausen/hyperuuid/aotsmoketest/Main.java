package io.github.buvinghausen.hyperuuid.aotsmoketest;

import io.github.buvinghausen.hyperuuid.UuidGenerator;
import java.util.UUID;

/**
 * Native-image smoke test: proves {@code UuidGenerator}'s FFM downcalls survive GraalVM
 * Native Image ahead-of-time compilation to a real native binary — no JVM required to run it.
 * Build and run with {@code ./gradlew :aot-smoke-test:nativeRun}.
 */
public final class Main {
    private Main() {}

    public static void main(String[] args) {
        UUID v4 = UuidGenerator.newV4();
        require(v4.version() == 4, "expected v4 version 4, got " + v4.version());

        UUID v5 = UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "www.example.com");
        UUID expectedV5 = UUID.fromString("2ed6657d-e927-568b-95e1-2665a8aea6a2");
        require(v5.equals(expectedV5), "v5 did not match the RFC 9562 test vector: got " + v5);

        long rfcTestVectorMs = 1_645_557_742_000L;
        UUID v6 = UuidGenerator.newV6(rfcTestVectorMs);
        require(v6.version() == 6, "expected v6 version 6, got " + v6.version());
        require(
                UuidGenerator.v6UnixMillis(v6) == rfcTestVectorMs,
                "v6 timestamp round-trip failed: got " + UuidGenerator.v6UnixMillis(v6));

        UUID v7 = UuidGenerator.newV7(rfcTestVectorMs);
        require(v7.version() == 7, "expected v7 version 7, got " + v7.version());
        require(
                UuidGenerator.v7UnixMillis(v7) == rfcTestVectorMs,
                "v7 timestamp round-trip failed: got " + UuidGenerator.v7UnixMillis(v7));

        require(UuidGenerator.NIL.toString().equals("00000000-0000-0000-0000-000000000000"), "NIL mismatch");
        require(UuidGenerator.MAX.toString().equals("ffffffff-ffff-ffff-ffff-ffffffffffff"), "MAX mismatch");

        UUID[] v7Batch = UuidGenerator.newV7Batch(10, rfcTestVectorMs);
        require(v7Batch.length == 10, "expected 10 batch v7 UUIDs, got " + v7Batch.length);
        require(v7Batch[0].version() == 7, "expected batch v7 version 7, got " + v7Batch[0].version());

        System.out.println("hyperuuid AOT smoke test passed: v4=" + v4 + " v5=" + v5 + " v6=" + v6 + " v7="
                + v7 + " v7Batch[0]=" + v7Batch[0]);
    }

    private static void require(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }
}
