package io.github.skunkwerkx.hyperuuid;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.charset.StandardCharsets;
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
import java.util.stream.Collectors;
import java.util.stream.IntStream;
import org.junit.jupiter.api.Test;

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

    @Test
    void v6EmbedsTheTimestamp() {
        UUID id = UuidGenerator.newV6(RFC_TEST_VECTOR_MS);
        assertEquals(Instant.ofEpochMilli(RFC_TEST_VECTOR_MS), UuidGenerator.v6Timestamp(id));
    }

    @Test
    void v6HasVersionAndVariantBitsSet() {
        UUID id = UuidGenerator.newV6(RFC_TEST_VECTOR_MS);
        assertEquals(6, id.version());
        assertEquals(2, id.variant());
    }

    @Test
    void v6SetsTheNodeIdMulticastBit() {
        UUID id = UuidGenerator.newV6(RFC_TEST_VECTOR_MS);
        long lsb = id.getLeastSignificantBits();
        int nodeFirstOctet = (int) ((lsb >>> 40) & 0xFF);
        assertEquals(1, nodeFirstOctet & 0x01);
    }

    @Test
    void v6IsNonDeterministicWithinTheSameMillisecond() {
        Set<UUID> results = IntStream.range(0, 100)
                .mapToObj(i -> UuidGenerator.newV6(RFC_TEST_VECTOR_MS))
                .collect(Collectors.toCollection(HashSet::new));
        assertEquals(100, results.size());
    }

    @Test
    void v6BatchReturnsCountUuidsSharingTheTimestamp() {
        UUID[] ids = UuidGenerator.newV6Batch(10, RFC_TEST_VECTOR_MS);
        assertEquals(10, ids.length);
        for (UUID id : ids) {
            assertEquals(6, id.version());
            assertEquals(Instant.ofEpochMilli(RFC_TEST_VECTOR_MS), UuidGenerator.v6Timestamp(id));
        }
    }

    @Test
    void v6BatchProducesPairwiseDistinctUuids() {
        UUID[] ids = UuidGenerator.newV6Batch(100, RFC_TEST_VECTOR_MS);
        assertEquals(100, Set.of(ids).size());
    }

    @Test
    void v6BatchCountZeroReturnsEmptyArray() {
        assertEquals(0, UuidGenerator.newV6Batch(0, RFC_TEST_VECTOR_MS).length);
    }

    @Test
    void v6BatchOverflowTimestampThrows() {
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.newV6Batch(1, Long.MAX_VALUE));
    }

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
        List<UUID> ids = IntStream.range(0, 100)
                .mapToObj(i -> UuidGenerator.newV7(RFC_TEST_VECTOR_MS))
                .collect(Collectors.toList());
        List<UUID> sorted = ids.stream().sorted().collect(Collectors.toList());
        assertEquals(sorted, ids);
    }

    @Test
    void v7CurrentTimestampIsEmbedded() {
        long before = System.currentTimeMillis();
        UUID id = UuidGenerator.newV7();
        long after = System.currentTimeMillis();

        long embeddedMs = (id.getMostSignificantBits() >>> 16) & 0xFFFF_FFFF_FFFFL;
        assertTrue(embeddedMs >= before && embeddedMs <= after);
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
    void newV6FromInstantMatchesNewV6FromTheEquivalentMillis() {
        Instant instant = Instant.ofEpochMilli(RFC_TEST_VECTOR_MS);
        UUID byInstant = UuidGenerator.newV6(instant);
        assertEquals(RFC_TEST_VECTOR_MS, UuidGenerator.v6UnixMillis(byInstant));
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

    @Test
    void getTimestampMatchesV6Timestamp() {
        UUID id = UuidGenerator.newV6(RFC_TEST_VECTOR_MS);
        assertEquals(Optional.of(UuidGenerator.v6Timestamp(id)), UuidGenerator.getTimestamp(id));
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
        UUID before = UuidGenerator.newV7(RFC_TEST_VECTOR_MS);
        UUID[] batch = UuidGenerator.newV7Batch(10, RFC_TEST_VECTOR_MS);
        UUID after = UuidGenerator.newV7(RFC_TEST_VECTOR_MS);

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

    @Test
    void v7ToSqlOrderPreservesVersionAndVariantAtOctets7And8() {
        UUID sqlOrdered = UuidGenerator.v7ToSqlOrder(UuidGenerator.newV7(RFC_TEST_VECTOR_MS));
        byte[] bytes = RfcBytes.toRfcBytes(sqlOrdered);
        assertEquals(0x70, bytes[7] & 0xF0);
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
        for (long i = 0; i < 200; i++) {
            ids.add(UuidGenerator.newV7(RFC_TEST_VECTOR_MS + i));
        }
        // Same-millisecond run, so the counter (not just the timestamp) has to sort correctly too.
        for (int i = 0; i < 200; i++) {
            ids.add(UuidGenerator.newV7(RFC_TEST_VECTOR_MS + 1_000_000));
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

    @Test
    void v6ToSqlOrderRoundTripsThroughV6FromSqlOrder() {
        UUID id = UuidGenerator.newV6(RFC_TEST_VECTOR_MS);
        UUID sqlOrdered = UuidGenerator.v6ToSqlOrder(id);
        assertNotEquals(id, sqlOrdered);
        assertEquals(id, UuidGenerator.v6FromSqlOrder(sqlOrdered));
    }

    @Test
    void v6ToSqlOrderPreservesVersionAndVariant() {
        // Different offsets than v7's sql order — see v6ToSqlOrder's doc comment for why.
        UUID sqlOrdered = UuidGenerator.v6ToSqlOrder(UuidGenerator.newV6(RFC_TEST_VECTOR_MS));
        byte[] bytes = RfcBytes.toRfcBytes(sqlOrdered);
        assertEquals(0x60, bytes[8] & 0xF0);
        assertEquals((byte) 0x80, (byte) (bytes[6] & 0xC0));
    }

    @Test
    void v6ToSqlOrderSortsByCreationOrderUnderSqlGuidComparisonForDistinctTimestamps() {
        // Unlike v7, v6 has no counter — two UUIDs at the same millisecond aren't guaranteed
        // to sort in creation order even in plain RFC order, so this only exercises strictly
        // increasing timestamps, where the timestamp alone determines order with no tie to break.
        List<UUID> ids = new ArrayList<>();
        for (long i = 0; i < 300; i++) {
            ids.add(UuidGenerator.newV6(RFC_TEST_VECTOR_MS + i));
        }

        List<byte[]> sqlOrdered = ids.stream()
                .map(UuidGenerator::v6ToSqlOrder)
                .map(RfcBytes::toRfcBytes)
                .collect(Collectors.toList());
        List<byte[]> sorted = new ArrayList<>(sqlOrdered);
        sorted.sort(UuidGeneratorTest::sqlGuidCompare);

        assertEquals(sqlOrdered.size(), sorted.size());
        for (int i = 0; i < sqlOrdered.size(); i++) {
            assertArrayEquals(sqlOrdered.get(i), sorted.get(i));
        }
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
        UuidGenerator.fillV7(dst, RFC_TEST_VECTOR_MS);
        for (int i = 1; i < dst.length; i++) {
            assertTrue(dst[i - 1].compareTo(dst[i]) < 0, "items " + (i - 1) + "/" + i + " out of order");
        }
    }

    @Test
    void fillV6FillsTheCallersArray() {
        UUID[] dst = new UUID[32];
        UuidGenerator.fillV6(dst, RFC_TEST_VECTOR_MS);
        for (int i = 0; i < dst.length; i++) {
            assertEquals(6, dst[i].version(), "item " + i + " version");
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
    void fillBytesRejectsAPartialUuid() {
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.fillV7(new byte[17], RFC_TEST_VECTOR_MS));
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.fillV6(new byte[15], RFC_TEST_VECTOR_MS));
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
    void v6ToSqlOrderBytesAgreesWithTheUuidForm() {
        UUID id = UuidGenerator.newV6(RFC_TEST_VECTOR_MS);
        byte[] want = RfcBytes.toRfcBytes(UuidGenerator.v6ToSqlOrder(id));
        byte[] got = RfcBytes.toRfcBytes(id);
        UuidGenerator.v6ToSqlOrder(got);
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
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.v6FromSqlOrder(new byte[17]));
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
    void v6OverflowTimestampThrows() {
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.newV6(Long.MAX_VALUE));
    }

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
            assertThrows(
                    IllegalArgumentException.class,
                    () -> UuidGenerator.newV6Batch(count, RFC_TEST_VECTOR_MS),
                    "v6 count " + count);
            assertThrows(IllegalArgumentException.class, () -> UuidGenerator.newV7Batch(count), "v7 count " + count);
            assertThrows(IllegalArgumentException.class, () -> UuidGenerator.newV6Batch(count), "v6 count " + count);
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
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.fillV6(new UUID[4], Long.MAX_VALUE));

        // The raw-byte forms too, and a refused fill leaves the caller's buffer alone.
        byte[] raw = new byte[64];
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.fillV7(raw, past48Bits));
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.fillV6(raw, Long.MAX_VALUE));
        assertArrayEquals(new byte[64], raw);
    }

    @Test
    void fillV6BytesWritesWholeRfcOrderedUuids() {
        int count = 16;
        byte[] raw = new byte[count * 16];
        UuidGenerator.fillV6(raw, RFC_TEST_VECTOR_MS);
        for (int i = 0; i < count; i++) {
            UUID id = RfcBytes.fromRfcBytes(raw, i * 16);
            assertEquals(6, id.version(), "item " + i + " version");
            assertEquals(RFC_TEST_VECTOR_MS, UuidGenerator.v6UnixMillis(id), "item " + i + " timestamp");
        }
    }

    // ---- The core itself ----------------------------------------------------------------

    @Test
    void nativeVersionIsThisBindingsOwn() {
        // The probe decodes the packed major.minor.patch of the core actually loaded — the
        // platform library or the wasm module — which must be the one this binding is built
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
                        UUID v6 = UuidGenerator.newV6(mine);
                        assertEquals(mine, UuidGenerator.v6UnixMillis(v6));
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
