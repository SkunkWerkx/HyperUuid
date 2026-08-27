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
        UUID v7 = UuidGenerator.newV7(rfcTestVectorMs);
        require(v7.version() == 7, "expected v7 version 7, got " + v7.version());
        require(
                UuidGenerator.v7UnixMillis(v7) == rfcTestVectorMs,
                "v7 timestamp round-trip failed: got " + UuidGenerator.v7UnixMillis(v7));

        System.out.println("hyperuuid AOT smoke test passed: v4=" + v4 + " v5=" + v5 + " v7=" + v7);
    }

    private static void require(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }
}
