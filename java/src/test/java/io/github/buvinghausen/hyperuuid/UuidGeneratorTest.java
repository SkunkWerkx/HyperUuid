package io.github.buvinghausen.hyperuuid;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.time.Instant;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.UUID;
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
}
