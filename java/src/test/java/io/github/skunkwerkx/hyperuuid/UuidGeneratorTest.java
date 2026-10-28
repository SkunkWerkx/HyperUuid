package io.github.skunkwerkx.hyperuuid;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Optional;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.stream.Collectors;
import java.util.stream.IntStream;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.function.Executable;

class UuidGeneratorTest {

    @Test
    void v4HasVersionAndVariantBitsSet() {
        UUID id = UuidGenerator.newV4();
        assertEquals(4, id.version());
        assertEquals(2, id.variant());
    }

    @Test
    void v4IsNonDeterministic() {
        Set<UUID> results = IntStream.range(0, 100)
                .mapToObj(i -> UuidGenerator.newV4())
                .collect(Collectors.toCollection(HashSet::new));
        assertEquals(100, results.size());
    }

    // RFC 9562 Appendix A.4 official test vector.
    @Test
    void v5MatchesRfcTestVector() {
        UUID id = UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "www.example.com");
        assertEquals(UUID.fromString("2ed6657d-e927-568b-95e1-2665a8aea6a2"), id);
    }

    // Python's `uuid` standard library documentation test vector.
    @Test
    void v5MatchesPythonDocsVector() {
        UUID id = UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "python.org");
        assertEquals(UUID.fromString("886313e1-3b8a-5372-9b90-0c9aee199e5d"), id);
    }

    @Test
    void v5IsDeterministic() {
        UUID a = UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "same-name");
        UUID b = UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "same-name");
        assertEquals(a, b);
    }

    @Test
    void v5DifferentNamespacesDiffer() {
        UUID dns = UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "test");
        UUID url = UuidGenerator.newV5(UuidGenerator.Namespaces.URL, "test");
        assertNotEquals(dns, url);
    }

    // RFC 9562 Appendix A.6: 2022-02-22T19:22:22Z = 1645557742000 ms since epoch.
    private static final long RFC_TEST_VECTOR_MS = 1_645_557_742_000L;

    // Moves the v7 stream to the present, so the fixed-vector calls below are minted out of it
    // and carry their timestamp exactly whatever the test order. The rule: no test
    // supplies more than now + 500 ms, and current-time bounds allow + 1000 ms.
    @BeforeAll
    static void primeStream() {
        UuidGenerator.newV7();
    }

    // The v7 stream is process-global and follows the clock, so a test of ordering mints at
    // the present: the fixed vector above is years behind the stream and is minted out of it,
    // unordered, though it carries its timestamp exactly. No test supplies more than
    // now + 500 ms, and current-time bounds allow + 1000 ms.

    @Test
    void nilIsAllZeroBits() {
        assertEquals("00000000-0000-0000-0000-000000000000", UuidGenerator.NIL.toString());
    }

    @Test
    void maxIsAllOneBits() {
        assertEquals("ffffffff-ffff-ffff-ffff-ffffffffffff", UuidGenerator.MAX.toString());
    }

    @Test
    void v7EmbedsTheTimestamp() {
        UUID id = UuidGenerator.newV7(RFC_TEST_VECTOR_MS);
        long embeddedMs = (id.getMostSignificantBits() >>> 16) & 0xFFFF_FFFF_FFFFL;
        assertEquals(RFC_TEST_VECTOR_MS, embeddedMs);
    }

    @Test
    void v7HasVersionAndVariantBitsSet() {
        UUID id = UuidGenerator.newV7(RFC_TEST_VECTOR_MS);
        assertEquals(7, id.version());
        assertEquals(2, id.variant());
    }

    @Test
    void v7OverflowTimestampThrows() {
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.newV7(0x0001_0000_0000_0000L));
    }

    @Test
    void v7SameMillisecondBatchIsMonotonicallyOrdered() {
        long now = System.currentTimeMillis();
        List<UUID> ids =
                IntStream.range(0, 100).mapToObj(i -> UuidGenerator.newV7(now)).collect(Collectors.toList());
        List<UUID> sorted = ids.stream().sorted().collect(Collectors.toList());
        assertEquals(sorted, ids);
    }

    @Test
    void v7CurrentTimestampIsEmbedded() {
        long before = System.currentTimeMillis();
        UUID id = UuidGenerator.newV7();
        long after = System.currentTimeMillis();

        long embeddedMs = (id.getMostSignificantBits() >>> 16) & 0xFFFF_FFFF_FFFFL;
        // The stream may stamp a later ms that an earlier test moved it to.
        assertTrue(embeddedMs >= before && embeddedMs <= after + 1_000);
    }

    @Test
    void v7TimestampRecoversTheExactMillisecond() {
        UUID id = UuidGenerator.newV7(RFC_TEST_VECTOR_MS);
        assertEquals(Instant.ofEpochMilli(RFC_TEST_VECTOR_MS), UuidGenerator.v7Timestamp(id));
    }

    @Test
    void v7TimestampRoundTripsZeroAndTheRfc48BitMax() {
        assertEquals(Instant.ofEpochMilli(0), UuidGenerator.v7Timestamp(UuidGenerator.newV7(0)));

        long maxMs = 0x0000_FFFF_FFFF_FFFFL;
        assertEquals(Instant.ofEpochMilli(maxMs), UuidGenerator.v7Timestamp(UuidGenerator.newV7(maxMs)));
    }

    @Test
    void newV7FromInstantMatchesNewV7FromTheEquivalentMillis() {
        Instant instant = Instant.ofEpochMilli(RFC_TEST_VECTOR_MS);
        UUID byInstant = UuidGenerator.newV7(instant);
        assertEquals(RFC_TEST_VECTOR_MS, UuidGenerator.v7UnixMillis(byInstant));
    }

    @Test
    void getTimestampReturnsEmptyForNonTimeBasedVersions() {
        assertFalse(UuidGenerator.getTimestamp(UuidGenerator.newV4()).isPresent());
        assertFalse(UuidGenerator.getTimestamp(UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "test"))
                .isPresent());
    }

    // Only version 7 timestamps are read: RFC 9562 A.5's version 6 has none.
    @Test
    void getTimestampReturnsEmptyForAV6() {
        UUID v6 = UUID.fromString("1ec9414c-232a-6b00-b3c8-9f6bdeced846");
        assertEquals(6, v6.version());
        assertEquals(Optional.empty(), UuidGenerator.getTimestamp(v6));
    }

    @Test
    void getTimestampMatchesV7Timestamp() {
        UUID id = UuidGenerator.newV7(RFC_TEST_VECTOR_MS);
        assertEquals(Optional.of(UuidGenerator.v7Timestamp(id)), UuidGenerator.getTimestamp(id));
    }

    @Test
    void v7BatchReturnsCountUuidsSortedAndSharingTheTimestamp() {
        UUID[] ids = UuidGenerator.newV7Batch(1000, RFC_TEST_VECTOR_MS);
        assertEquals(1000, ids.length);
        UUID[] sorted = ids.clone();
        Arrays.sort(sorted);
        assertEquals(Arrays.asList(sorted), Arrays.asList(ids));
        for (UUID id : ids) {
            assertEquals(Instant.ofEpochMilli(RFC_TEST_VECTOR_MS), UuidGenerator.v7Timestamp(id));
        }
    }

    @Test
    void v7BatchContinuesTheSameCounterSequenceAsIndividualCalls() {
        long now = System.currentTimeMillis();
        UUID before = UuidGenerator.newV7(now);
        UUID[] batch = UuidGenerator.newV7Batch(10, now);
        UUID after = UuidGenerator.newV7(now);

        List<UUID> ids = new ArrayList<>();
        ids.add(before);
        ids.addAll(List.of(batch));
        ids.add(after);
        List<UUID> sorted = new ArrayList<>(ids);
        Collections.sort(sorted);
        assertEquals(sorted, ids);
    }

    @Test
    void v7BatchCountZeroReturnsEmptyArray() {
        assertEquals(0, UuidGenerator.newV7Batch(0, RFC_TEST_VECTOR_MS).length);
    }

    @Test
    void v7BatchOverflowTimestampThrows() {
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.newV7Batch(1, 0x0001_0000_0000_0000L));
    }

    @Test
    void v7ToSqlOrderRoundTripsThroughV7FromSqlOrder() {
        UUID id = UuidGenerator.newV7(RFC_TEST_VECTOR_MS);
        UUID sqlOrdered = UuidGenerator.v7ToSqlOrder(id);
        assertNotEquals(id, sqlOrdered);
        assertEquals(id, UuidGenerator.v7FromSqlOrder(sqlOrdered));
    }

    // The SQL layout is an RFC 9562 version 8: the 8 at octet 7's top nibble and the variant at
    // octet 8's top two bits, where SQL Server's mixed-endian text form shows them.
    @Test
    void v7ToSqlOrderWritesVersion8AndTheVariantAtOctets7And8() {
        UUID sqlOrdered = UuidGenerator.v7ToSqlOrder(UuidGenerator.newV7(RFC_TEST_VECTOR_MS));
        byte[] bytes = RfcBytes.toRfcBytes(sqlOrdered);
        assertEquals(0x80, bytes[7] & 0xF0);
        assertEquals((byte) 0x80, (byte) (bytes[8] & 0xC0));
    }

    /**
     * Replicates {@code System.Data.SqlTypes.SqlGuid.CompareTo}'s fixed byte significance
     * order — the correctness oracle this project's C# test suite checks directly against the
     * real type; no JVM equivalent exists to test against here, so this stands in for it.
     */
    private static int sqlGuidCompare(byte[] a, byte[] b) {
        int[] significanceOrder = {10, 11, 12, 13, 14, 15, 8, 9, 6, 7, 4, 5, 0, 1, 2, 3};
        for (int i : significanceOrder) {
            int cmp = Integer.compare(a[i] & 0xFF, b[i] & 0xFF);
            if (cmp != 0) {
                return cmp;
            }
        }
        return 0;
    }

    @Test
    void v7ToSqlOrderSortsByCreationOrderUnderSqlGuidComparison() {
        List<UUID> ids = new ArrayList<>();
        long now = System.currentTimeMillis();
        for (long i = 0; i < 200; i++) {
            ids.add(UuidGenerator.newV7(now + i));
        }
        // Same-millisecond run, so the counter (not just the timestamp) has to sort correctly too.
        for (int i = 0; i < 200; i++) {
            ids.add(UuidGenerator.newV7(now + 200));
        }

        List<byte[]> sqlOrdered = ids.stream()
                .map(UuidGenerator::v7ToSqlOrder)
                .map(RfcBytes::toRfcBytes)
                .collect(Collectors.toList());
        List<byte[]> sorted = new ArrayList<>(sqlOrdered);
        sorted.sort(UuidGeneratorTest::sqlGuidCompare);

        assertEquals(sqlOrdered.size(), sorted.size());
        for (int i = 0; i < sqlOrdered.size(); i++) {
            assertArrayEquals(sqlOrdered.get(i), sorted.get(i));
        }
    }

    // ---- SQL-order refusals ------------------------------------------------------------

    private static final String WRONG_VERSION = "the value is not a UUID of the version this conversion takes";

    private static void assertRefused(String expected, Executable call) {
        assertEquals(
                expected, assertThrows(IllegalArgumentException.class, call).getMessage());
    }

    // The layout holds all 122 of a v7's free bits, so every timestamp newV7 takes converts,
    // up to the 48-bit field's last millisecond, and comes back exactly.
    @Test
    void v7ToSqlOrderTakesEveryTimestamp() {
        for (long ms : new long[] {0L, RFC_TEST_VECTOR_MS, 1L << 47, 0x0000_FFFF_FFFF_FFFFL}) {
            UUID id = UuidGenerator.newV7(ms);
            UUID sql = UuidGenerator.v7ToSqlOrder(id);
            assertEquals(7, UuidGenerator.version(sql, UuidLayout.SQL_SERVER), "ms " + ms);
            assertEquals(ms, UuidGenerator.v7UnixMillis(sql, UuidLayout.SQL_SERVER), "ms " + ms);
            assertEquals(id, UuidGenerator.v7FromSqlOrder(sql), "ms " + ms);
        }
    }

    @Test
    void theConversionsRefuseTheWrongVersion() {
        // RFC 9562 A.5's v6, from another generator: never converted, in either direction.
        UUID v6 = UUID.fromString("1ec9414c-232a-6b00-b3c8-9f6bdeced846");
        UUID v4 = UuidGenerator.newV4();
        assertRefused(WRONG_VERSION, () -> UuidGenerator.v7ToSqlOrder(v6));
        assertRefused(WRONG_VERSION, () -> UuidGenerator.v7ToSqlOrder(v4));
        assertRefused(WRONG_VERSION, () -> UuidGenerator.v7FromSqlOrder(v6));
        // RFC-ordered values: a fixed vector (RFC 9562 A.6), since a random one carries the
        // SQL layout's 8 at octet 7 one time in 16.
        assertRefused(
                WRONG_VERSION,
                () -> UuidGenerator.v7FromSqlOrder(UUID.fromString("017f22e2-79b0-7cc3-98c4-dc0c0c07398f")));
        // And its SQL form, fixed for the same reason, refused going in again.
        assertRefused(
                WRONG_VERSION,
                () -> UuidGenerator.v7ToSqlOrder(UUID.fromString("0c07398f-dc0c-8c84-b30d-017f22e279b0")));
        // A converted v7 with its 8 put back to a 7 (the layout as 0.8.0 wrote it) is not
        // read as SQL order.
        UUID v7Sql = UuidGenerator.v7ToSqlOrder(UuidGenerator.newV7(RFC_TEST_VECTOR_MS));
        UUID sevenInsteadOfEight =
                new UUID((v7Sql.getMostSignificantBits() & ~0xF0L) | 0x70L, v7Sql.getLeastSignificantBits());
        assertEquals(0, UuidGenerator.version(sevenInsteadOfEight, UuidLayout.SQL_SERVER));
        assertRefused(WRONG_VERSION, () -> UuidGenerator.v7FromSqlOrder(sevenInsteadOfEight));
        // Not checked the other way: a SQL-ordered value's octet 6 holds key bits, which read
        // as an RFC 7 one time in 16, and then converting it "again" is not refused —
        // the order a value is in is the caller's to track.
    }

    @Test
    void aRefusedRawByteConversionLeavesTheBytesUnchanged() {
        // RFC 9562 A.5's v6: fixed, so its octet 7 never happens to carry the SQL layout's 8.
        byte[] v6 = RfcBytes.toRfcBytes(UUID.fromString("1ec9414c-232a-6b00-b3c8-9f6bdeced846"));
        byte[] original = v6.clone();
        assertRefused(WRONG_VERSION, () -> UuidGenerator.v7ToSqlOrder(v6));
        assertRefused(WRONG_VERSION, () -> UuidGenerator.v7FromSqlOrder(v6));
        assertArrayEquals(original, v6);
    }

    // ---- Destination-buffer fills ----------------------------------------------------

    @Test
    void fillV7FillsTheCallersArray() {
        UUID[] dst = new UUID[64];
        UuidGenerator.fillV7(dst, RFC_TEST_VECTOR_MS);
        for (int i = 0; i < dst.length; i++) {
            assertEquals(7, dst[i].version(), "item " + i + " version");
            assertEquals(RFC_TEST_VECTOR_MS, UuidGenerator.v7UnixMillis(dst[i]), "item " + i + " timestamp");
        }
    }

    @Test
    void fillV7IsStrictlyIncreasing() {
        UUID[] dst = new UUID[256];
        UuidGenerator.fillV7(dst, System.currentTimeMillis());
        for (int i = 1; i < dst.length; i++) {
            assertTrue(dst[i - 1].compareTo(dst[i]) < 0, "items " + (i - 1) + "/" + i + " out of order");
        }
    }

    @Test
    void fillV7BytesMatchesTheArrayForm() {
        int count = 16;
        byte[] raw = new byte[count * 16];
        UuidGenerator.fillV7(raw, RFC_TEST_VECTOR_MS);
        for (int i = 0; i < count; i++) {
            byte[] one = Arrays.copyOfRange(raw, i * 16, i * 16 + 16);
            UUID id = RfcBytes.fromRfcBytes(one);
            assertEquals(7, id.version(), "item " + i + " version");
            assertEquals(RFC_TEST_VECTOR_MS, UuidGenerator.v7UnixMillis(id), "item " + i + " timestamp");
        }
    }

    @Test
    void fillV7SurvivesAnotherThreadWritingTheSameArray() throws Exception {
        // A race on the array's contents is the caller's to have, and must cost them the bytes,
        // not the process. The FFM backend hands the core the caller's own array (critical
        // linking), and Windows' ProcessPrng fail-fasts the process when another thread writes
        // the memory it is filling, so the core's batch fill does not hand it the caller's memory.
        byte[] raw = new byte[16 * 20_000];
        AtomicBoolean done = new AtomicBoolean();
        ExecutorService writers = Executors.newFixedThreadPool(2);
        try {
            List<Future<?>> running = new ArrayList<>();
            for (int w = 0; w < 2; w++) {
                running.add(writers.submit(() -> {
                    while (!done.get()) {
                        Arrays.fill(raw, (byte) 0);
                    }
                }));
            }
            for (int i = 0; i < 100; i++) {
                UuidGenerator.fillV7(raw, RFC_TEST_VECTOR_MS);
            }
            done.set(true);
            for (Future<?> f : running) {
                f.get();
            }
        } finally {
            done.set(true);
            writers.shutdownNow();
        }
    }

    @Test
    void fillBytesRejectsAPartialUuid() {
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.fillV7(new byte[17], RFC_TEST_VECTOR_MS));
    }

    @Test
    void fillEmptyIsANoOp() {
        UuidGenerator.fillV7(new UUID[0], RFC_TEST_VECTOR_MS);
        UuidGenerator.fillV7(new byte[0], RFC_TEST_VECTOR_MS);
    }

    // ---- Raw-byte SQL-order transforms -----------------------------------------------

    @Test
    void v7ToSqlOrderBytesAgreesWithTheUuidForm() {
        UUID id = UuidGenerator.newV7(RFC_TEST_VECTOR_MS);
        byte[] want = RfcBytes.toRfcBytes(UuidGenerator.v7ToSqlOrder(id));
        byte[] got = RfcBytes.toRfcBytes(id);
        UuidGenerator.v7ToSqlOrder(got);
        assertArrayEquals(want, got);
    }

    @Test
    void sqlOrderBytesRoundTrips() {
        UUID id = UuidGenerator.newV7(RFC_TEST_VECTOR_MS);
        byte[] original = RfcBytes.toRfcBytes(id);
        byte[] b = original.clone();
        UuidGenerator.v7ToSqlOrder(b);
        assertFalse(Arrays.equals(original, b), "sql order did not change the bytes");
        UuidGenerator.v7FromSqlOrder(b);
        assertArrayEquals(original, b);
    }

    @Test
    void sqlOrderBytesRejectsAWrongSizedBuffer() {
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.v7ToSqlOrder(new byte[15]));
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.v7FromSqlOrder(new byte[17]));
    }

    // ---- The v5 name, in every form it can arrive -------------------------------------

    @Test
    void v5OfTheEmptyNameHashesTheNamespaceAlone() {
        // The one input that crosses as the ABI's NULL — there are no bytes to pin — so it
        // gets its own vector: Python's uuid.uuid5(uuid.NAMESPACE_DNS, "").
        UUID expected = UUID.fromString("4ebd0208-8328-5d69-8c44-ec50939c0967");
        assertEquals(expected, UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, ""));
        assertEquals(expected, UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, new byte[0]));
    }

    @Test
    void v5OverloadsAgreeOnTheSameBytes() {
        String name = "www.example.com";
        UUID expected = UUID.fromString("2ed6657d-e927-568b-95e1-2665a8aea6a2");
        assertEquals(
                expected, UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, name.getBytes(StandardCharsets.UTF_8)));
        assertEquals(expected, UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, name, StandardCharsets.US_ASCII));
        // The charset is part of the name: the same text in another encoding is another UUID.
        assertNotEquals(expected, UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, name, StandardCharsets.UTF_16LE));
    }

    // ---- Batch sizes and timestamps the core cannot be handed ---------------------------

    @Test
    void batchCountOutsideWhatOneBatchCanCarryIsRejected() {
        // count * 16 is the size of the buffer the core fills. Negative counts, and counts
        // whose product no longer fits an int — 1 << 28 wraps it to exactly zero — have to
        // be refused before that multiplication, not discovered by the core writing past
        // a buffer that came out too small.
        int[] refused = {
            -1, Integer.MIN_VALUE, -(1 << 28), UuidGenerator.MAX_BATCH + 1, 1 << 28, (1 << 28) + 1, Integer.MAX_VALUE,
        };
        for (int count : refused) {
            assertThrows(
                    IllegalArgumentException.class,
                    () -> UuidGenerator.newV7Batch(count, RFC_TEST_VECTOR_MS),
                    "v7 count " + count);
            assertThrows(IllegalArgumentException.class, () -> UuidGenerator.newV7Batch(count), "v7 count " + count);
            assertThrows(
                    IllegalArgumentException.class, () -> UuidGenerator.requireBatchCount(count), "count " + count);
        }
        // The same check guards a UUID[] destination's length; an array that long is not
        // something a test should allocate, so the limit itself is what is pinned here.
        assertEquals(Integer.MAX_VALUE / 16, UuidGenerator.MAX_BATCH);
        UuidGenerator.requireBatchCount(UuidGenerator.MAX_BATCH);
        UuidGenerator.requireBatchCount(0);
    }

    @Test
    void fillRejectsATimestampItsVersionCannotHold() {
        long past48Bits = 0x0001_0000_0000_0000L;
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.fillV7(new UUID[4], past48Bits));
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.fillV7(new UUID[4], -1L));

        // The raw-byte forms too, and a refused fill leaves the caller's buffer alone.
        byte[] raw = new byte[64];
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.fillV7(raw, past48Bits));
        assertArrayEquals(new byte[64], raw);
    }

    // ---- The core itself ----------------------------------------------------------------

    @Test
    void nativeVersionIsThisBindingsOwn() {
        // The probe decodes the packed major.minor.patch of the core actually loaded — the
        // platform library — which must be the one this binding is built
        // against: build.gradle.kts hands its own version to the test JVM, and rust/Cargo.toml
        // moves with it.
        // A CI override may append a prerelease tag (a -ci.N suffix, per build.gradle.kts); the
        // core carries only major.minor.patch, so compare that much.
        var expected = System.getProperty("hyperuuid.version").split("-", 2)[0];
        assertEquals(expected, UuidGenerator.nativeVersion());
    }

    @Test
    void isAvailableAgreesWithNativeVersion() {
        // The suite only ever runs with a core staged, so the probe says so — and it says so
        // by the same crossing nativeVersion() makes, so the two cannot disagree.
        assertTrue(UuidGenerator.isAvailable());
        assertDoesNotThrow(UuidGenerator::nativeVersion);
    }

    @Test
    void withoutACoreTheProbeAnswersFalseAndTheDoorsThrowTheLoadFailure() throws Exception {
        // A second JVM with the compiled classes but no native library on its classpath: the
        // README's contract is that isAvailable() answers false there and never throws, and
        // that a generating method throws ExceptionInInitializerError, then
        // NoClassDefFoundError, rather than anything wrapped around them.
        var classes = String.join(
                java.io.File.pathSeparator,
                Path.of(UuidGenerator.class
                                .getProtectionDomain()
                                .getCodeSource()
                                .getLocation()
                                .toURI())
                        .toString(),
                Path.of(NoCore.class
                                .getProtectionDomain()
                                .getCodeSource()
                                .getLocation()
                                .toURI())
                        .toString());
        var java = ProcessHandle.current().info().command().orElse("java");
        var process = new ProcessBuilder(
                        java, "--enable-native-access=ALL-UNNAMED", "-cp", classes, NoCore.class.getName())
                .redirectErrorStream(true)
                .start();
        var output = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        assertEquals(0, process.waitFor(), output);
        assertTrue(
                output.contains("doors: java.lang.ExceptionInInitializerError java.lang.NoClassDefFoundError"), output);
        assertTrue(output.contains("probe: false"), output);
    }

    @Test
    void counterExhaustionInAChildJvm() throws Exception {
        // The stream is process-global, so driving it to exhaustion happens in a JVM of its
        // own: this one's stream stays where the other tests left it. The child gets this
        // JVM's classpath, which carries the freshly staged core, and its native-access flag.
        var java = ProcessHandle.current().info().command().orElse("java");
        var log = Files.createTempFile("hyperuuid-exhaustion", ".log");
        try {
            var process = new ProcessBuilder(
                            java,
                            "--enable-native-access=ALL-UNNAMED",
                            "-cp",
                            System.getProperty("java.class.path"),
                            ExhaustionMain.class.getName())
                    .redirectErrorStream(true)
                    .redirectOutput(log.toFile())
                    .start();
            // Output goes to a file, not a pipe, so a wait with a deadline cannot deadlock on a
            // full pipe; on expiry the child is killed and the test fails.
            if (!process.waitFor(120, TimeUnit.SECONDS)) {
                process.destroyForcibly();
                throw new AssertionError("the exhaustion child JVM did not finish in 120 s");
            }
            var output = Files.readString(log, StandardCharsets.UTF_8);
            assertEquals(0, process.exitValue(), output);
            assertTrue(output.contains("exhaustion script done"), output);
        } finally {
            Files.deleteIfExists(log);
        }
    }

    /**
     * The child JVM's program: the exhaustion script every binding runs. Exits 0 only when each
     * step ends in an {@link IllegalStateException} with the exact message.
     */
    static final class ExhaustionMain {
        private static final long M = 17_592_186_044_414L; // 2^44 - 2
        private static final String MESSAGE = "version 7 counter exhausted: the stream cannot advance without "
                + "running more than 1000 ms ahead of the supplied timestamp or past its last millisecond";

        private static void expectExhausted(String door, Runnable call) {
            try {
                call.run();
            } catch (IllegalStateException e) {
                if (!MESSAGE.equals(e.getMessage())) {
                    throw new AssertionError(door + ": wrong message: " + e.getMessage(), e);
                }
                return;
            }
            throw new AssertionError(door + ": did not throw");
        }

        public static void main(String[] args) {
            // 1. One single at M succeeds.
            UuidGenerator.newV7(M);
            // 2. Batches until the first error: the batch site (at most 1,025 of them).
            boolean batchFailed = false;
            for (int i = 0; i < 1_025 && !batchFailed; i++) {
                try {
                    UuidGenerator.newV7Batch(65_536, M);
                } catch (IllegalStateException e) {
                    if (!MESSAGE.equals(e.getMessage())) {
                        throw new AssertionError("batch site: wrong message: " + e.getMessage(), e);
                    }
                    batchFailed = true;
                }
            }
            if (!batchFailed) {
                throw new AssertionError("1,025 batches never exhausted the stream");
            }
            // 3. Singles until the first error: the single site (a failed batch can leave up to
            // 1,023 steps).
            boolean singleFailed = false;
            for (int i = 0; i < 1_024 && !singleFailed; i++) {
                try {
                    UuidGenerator.newV7(M);
                } catch (IllegalStateException e) {
                    if (!MESSAGE.equals(e.getMessage())) {
                        throw new AssertionError("single site: wrong message: " + e.getMessage(), e);
                    }
                    singleFailed = true;
                }
            }
            if (!singleFailed) {
                throw new AssertionError("1,024 singles never exhausted the stream");
            }
            // 4. Every other v7 door fails at once.
            expectExhausted("newV7(Instant)", () -> UuidGenerator.newV7(Instant.ofEpochMilli(M)));
            expectExhausted("newV7Batch(1, ms)", () -> UuidGenerator.newV7Batch(1, M));
            expectExhausted("fillV7(UUID[], ms)", () -> UuidGenerator.fillV7(new UUID[1], M));
            expectExhausted("fillV7(byte[], ms)", () -> UuidGenerator.fillV7(new byte[16], M));
            System.out.println("exhaustion script done");
        }
    }

    /** The second JVM's program: the doors first, so the first one meets the initializer. */
    static final class NoCore {
        public static void main(String[] args) {
            var thrown = new StringBuilder("doors:");
            for (int i = 0; i < 2; i++) {
                try {
                    UuidGenerator.newV4();
                    thrown.append(" none");
                } catch (Throwable t) {
                    thrown.append(' ').append(t.getClass().getName());
                }
            }
            System.out.println(thrown);
            System.out.println("probe: " + UuidGenerator.isAvailable());
        }
    }

    @Test
    void doorsStayCorrectAcrossThreadsOnPerThreadScratch() throws Exception {
        // The per-call confined arena became per-thread scratch (UuidGenerator.Scratch), which
        // turns "thread-confined" from a structural guarantee into a claim — so prove it. Each
        // thread mints and reads back values only it knows, thousands of times; any
        // cross-thread bleed of the 16-byte in/out segments surfaces as a timestamp, a
        // name-based UUID or a byte order that belongs to another thread.
        int threads = 8;
        int iterations = 2_000;
        ExecutorService pool = Executors.newFixedThreadPool(threads);
        List<Future<?>> running = new ArrayList<>();
        try {
            for (int t = 0; t < threads; t++) {
                int id = t;
                running.add(pool.submit(() -> {
                    for (int i = 0; i < iterations; i++) {
                        long mine = RFC_TEST_VECTOR_MS + id * 1_000_000L + i;
                        UUID v7 = UuidGenerator.newV7(mine);
                        assertEquals(mine, UuidGenerator.v7UnixMillis(v7));
                        assertEquals(v7, UuidGenerator.v7FromSqlOrder(UuidGenerator.v7ToSqlOrder(v7)));
                        String name = "thread-" + id + "-item-" + i;
                        assertEquals(
                                UuidGenerator.newV5(UuidGenerator.Namespaces.URL, name),
                                UuidGenerator.newV5(UuidGenerator.Namespaces.URL, name));
                    }
                }));
            }
            for (Future<?> task : running) {
                task.get();
            }
        } finally {
            pool.shutdownNow();
        }
    }
}
