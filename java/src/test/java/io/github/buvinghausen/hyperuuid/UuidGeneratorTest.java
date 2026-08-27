package io.github.buvinghausen.hyperuuid;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.time.Instant;
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
}
