package io.github.skunkwerkx.hyperuuid;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.time.Instant;
import java.util.Arrays;
import java.util.Optional;
import java.util.UUID;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.function.Executable;

/**
 * Version/variant inspection and the layout-aware doors against values minted at run time, by
 * this library and by the JDK; {@link CorpusTest} pins the fixed vectors.
 */
class InspectionTest {
    private static final long MS = 1_645_557_742_000L;

    // Moves the v7 stream to the present, so the fixed-vector calls below are minted out of it
    // and carry their timestamp exactly whatever the test order. The rule: no test
    // supplies more than now + 500 ms, and current-time bounds allow + 1000 ms.
    @BeforeAll
    static void primeStream() {
        UuidGenerator.newV7();
    }

    @Test
    void jdkAndLibraryValuesReportTheirVersionAndTheRfcVariant() {
        Object[][] cases = {
            {UUID.randomUUID(), 4},
            {UUID.nameUUIDFromBytes(new byte[] {1, 2, 3}), 3},
            {UuidGenerator.newV4(), 4},
            {UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "x"), 5},
            {UUID.fromString("1ec9414c-232a-6b00-b3c8-9f6bdeced846"), 6}, // RFC 9562 A.5, from another generator
            {UuidGenerator.newV7(MS), 7},
        };
        for (Object[] c : cases) {
            UUID id = (UUID) c[0];
            int version = (Integer) c[1];
            assertEquals(version, UuidGenerator.version(id), id.toString());
            assertEquals(id.version(), UuidGenerator.version(id), "the JDK agrees: " + id);
            assertEquals(UuidVariant.RFC_9562, UuidGenerator.variant(id), id.toString());
            assertEquals(2, id.variant(), "the JDK agrees: " + id);
            assertTrue(UuidGenerator.isRfc(id, version), id.toString());
            assertFalse(UuidGenerator.isRfc(id, version == 7 ? 6 : 7), id.toString());
            assertEquals(version, UuidGenerator.version(RfcBytes.toRfcBytes(id)), id.toString());
            assertEquals(UuidVariant.RFC_9562, UuidGenerator.variant(RfcBytes.toRfcBytes(id)), id.toString());
            assertTrue(UuidGenerator.isRfc(RfcBytes.toRfcBytes(id), version), id.toString());
        }
    }

    @Test
    void nilAndMaxAreClassifiedAsTheRfcSays() {
        assertEquals(0, UuidGenerator.version(UuidGenerator.NIL));
        assertEquals(UuidVariant.NCS, UuidGenerator.variant(UuidGenerator.NIL));
        assertEquals(15, UuidGenerator.version(UuidGenerator.MAX));
        assertEquals(UuidVariant.FUTURE, UuidGenerator.variant(UuidGenerator.MAX));
        assertFalse(UuidGenerator.isRfc(UuidGenerator.NIL, 0));
        assertFalse(UuidGenerator.isRfc(UuidGenerator.MAX, 15));
    }

    @Test
    void enumsCarryTheCoreCodes() {
        assertEquals(0, UuidLayout.UNSPECIFIED.code());
        assertEquals(1, UuidLayout.RFC_9562.code());
        assertEquals(2, UuidLayout.SQL_SERVER.code());
        assertEquals(0, UuidVariant.UNSPECIFIED.code());
        assertEquals(1, UuidVariant.NCS.code());
        assertEquals(2, UuidVariant.RFC_9562.code());
        assertEquals(3, UuidVariant.MICROSOFT.code());
        assertEquals(4, UuidVariant.FUTURE.code());
    }

    @Test
    void aVersionOutsideTheNibbleNeverMatches() {
        UUID id = UuidGenerator.newV7(MS);
        for (int version : new int[] {7 + 256, 16, -1, Integer.MIN_VALUE, Integer.MAX_VALUE}) {
            assertFalse(UuidGenerator.isRfc(id, version), "version " + version);
            assertFalse(UuidGenerator.isRfc(RfcBytes.toRfcBytes(id), version), "version " + version);
            assertFalse(UuidGenerator.isRfc(UuidGenerator.v7ToSqlOrder(id), version, UuidLayout.SQL_SERVER));
        }
    }

    // Every converted v7 reads as version 7 in place, whatever its random bits: enough draws
    // that a value the layout mis-read would surface.
    @Test
    void sqlOrderedV7IsValidatedAndReadInPlace() {
        Optional<Instant> at = Optional.of(Instant.ofEpochMilli(MS));
        for (int i = 0; i < 2048; i++) {
            UUID seven = UuidGenerator.v7ToSqlOrder(UuidGenerator.newV7(MS));

            assertEquals(7, UuidGenerator.version(seven, UuidLayout.SQL_SERVER));
            assertTrue(UuidGenerator.isRfc(seven, 7, UuidLayout.SQL_SERVER));
            assertFalse(UuidGenerator.isRfc(seven, 6, UuidLayout.SQL_SERVER));
            assertFalse(UuidGenerator.isRfc(seven, 8, UuidLayout.SQL_SERVER));

            assertEquals(MS, UuidGenerator.v7UnixMillis(seven, UuidLayout.SQL_SERVER));
            assertEquals(at, UuidGenerator.getTimestamp(seven, UuidLayout.SQL_SERVER));
            // Read straight from SQL order matches converting back first.
            assertEquals(
                    UuidGenerator.v7UnixMillis(UuidGenerator.v7FromSqlOrder(seven)),
                    UuidGenerator.v7UnixMillis(seven, UuidLayout.SQL_SERVER));
            // The raw form takes the bytes SQL Server stores: the SQL-ordered UUID's bytes.
            assertEquals(7, UuidGenerator.version(RfcBytes.toRfcBytes(seven), UuidLayout.SQL_SERVER));
        }
    }

    // Fixed vectors only: RFC-ordered bytes can genuinely carry the SQL layout's 8 and variant
    // (octet 8 already carries the RFC variant, octet 7 is random), so one random v4 in 16
    // would read as a SQL-ordered v7. That is correct; the layout is the caller's to know.
    @Test
    void nonSqlValuesHaveNoSqlVersion() {
        UUID[] ids = {
            UuidGenerator.NIL,
            UuidGenerator.MAX,
            UUID.fromString("919108f7-52d1-4320-9bac-f847db4148a8"), // v4, RFC 9562 A.3
            UUID.fromString("017f22e2-79b0-7cc3-98c4-dc0c0c07398f"), // v7, RFC 9562 A.6
            UUID.fromString("1ec9414c-232a-6b00-b3c8-9f6bdeced846"), // v6, RFC 9562 A.5
        };
        for (UUID id : ids) {
            assertEquals(0, UuidGenerator.version(id, UuidLayout.SQL_SERVER), id.toString());
            assertEquals(Optional.empty(), UuidGenerator.getTimestamp(id, UuidLayout.SQL_SERVER), id.toString());
        }
        // RFC 9562 A.4's v5 is one of the one in 16: octet 7 is 0x8b and octet 8 0x95, so read
        // as SQL order it carries the layout's 8 and variant.
        UUID v5 = UUID.fromString("2ed6657d-e927-568b-95e1-2665a8aea6a2");
        assertEquals(7, UuidGenerator.version(v5, UuidLayout.SQL_SERVER));
    }

    // getTimestamp requires the RFC variant in both layouts: the nibble alone is not a version.
    @Test
    void getTimestampRequiresTheRfcVariant() {
        UUID v7 = UuidGenerator.newV7(MS);
        for (long variantBits : new long[] {0x0L, 0x6L, 0x7L}) { // NCS 0xx, Microsoft 110, future 111
            UUID broken = new UUID(
                    v7.getMostSignificantBits(), (v7.getLeastSignificantBits() & ~(0x7L << 61)) | (variantBits << 61));
            assertEquals(7, UuidGenerator.version(broken), broken.toString());
            assertEquals(Optional.empty(), UuidGenerator.getTimestamp(broken), broken.toString());
            assertEquals(Optional.empty(), UuidGenerator.getTimestamp(broken, UuidLayout.RFC_9562));
        }
        assertEquals(Optional.of(Instant.ofEpochMilli(MS)), UuidGenerator.getTimestamp(v7));
    }

    // Only version 7 timestamps are read: a version 6 from another generator has none, in
    // either layout, though its version nibble still reads 6.
    @Test
    void aV6HasNoTimestamp() {
        UUID v6 = UUID.fromString("1ec9414c-232a-6b00-b3c8-9f6bdeced846");
        assertEquals(6, UuidGenerator.version(v6));
        assertTrue(UuidGenerator.isRfc(v6, 6));
        assertEquals(Optional.empty(), UuidGenerator.getTimestamp(v6));
        assertEquals(Optional.empty(), UuidGenerator.getTimestamp(v6, UuidLayout.RFC_9562));
        assertEquals(Optional.empty(), UuidGenerator.getTimestamp(v6, UuidLayout.SQL_SERVER));
    }

    @Test
    void anUnspecifiedOrNullLayoutThrows() {
        UUID id = UuidGenerator.newV7(MS);
        byte[] raw = new byte[16];
        Executable[] doors = {
            () -> UuidGenerator.version(id, UuidLayout.UNSPECIFIED),
            () -> UuidGenerator.isRfc(id, 7, UuidLayout.UNSPECIFIED),
            () -> UuidGenerator.v7UnixMillis(id, UuidLayout.UNSPECIFIED),
            () -> UuidGenerator.v7Timestamp(id, UuidLayout.UNSPECIFIED),
            () -> UuidGenerator.getTimestamp(id, UuidLayout.UNSPECIFIED),
            () -> UuidGenerator.version(raw, UuidLayout.UNSPECIFIED),
            () -> UuidGenerator.isRfc(raw, 7, UuidLayout.UNSPECIFIED),
        };
        for (Executable door : doors) {
            IllegalArgumentException thrown = assertThrows(IllegalArgumentException.class, door);
            assertTrue(thrown.getMessage().contains("layout"), thrown.getMessage());
        }
        Executable[] nullDoors = {
            () -> UuidGenerator.version(id, null),
            () -> UuidGenerator.isRfc(id, 7, null),
            () -> UuidGenerator.v7UnixMillis(id, null),
            () -> UuidGenerator.v7Timestamp(id, null),
            () -> UuidGenerator.getTimestamp(id, null),
            () -> UuidGenerator.version(raw, null),
            () -> UuidGenerator.isRfc(raw, 7, null),
        };
        for (Executable door : nullDoors) {
            assertEquals(
                    "layout", assertThrows(NullPointerException.class, door).getMessage());
        }
    }

    @Test
    void theRawByteFormsRequireExactly16Bytes() {
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.version(new byte[15]));
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.version(new byte[17], UuidLayout.SQL_SERVER));
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.variant(new byte[17]));
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.isRfc(new byte[0], 7));
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.isRfc(new byte[15], 7, UuidLayout.SQL_SERVER));
    }

    // The v7 batch limit: the counter space, enforced before anything is allocated or handed
    // to the core. The arrays here are the caller's, sized just past the limit, and must come
    // back untouched.
    @Test
    void aV7BatchPastTheCounterSpaceIsRefusedOnEveryDoor() {
        int tooMany = UuidGenerator.MAX_V7_BATCH + 1;
        String limit = Integer.toString(UuidGenerator.MAX_V7_BATCH);
        Executable[] doors = {
            () -> UuidGenerator.newV7Batch(tooMany, MS),
            () -> UuidGenerator.newV7Batch(tooMany),
            () -> UuidGenerator.newV7Batch(Integer.MAX_VALUE / 16, MS),
        };
        for (Executable door : doors) {
            assertTrue(assertThrows(IllegalArgumentException.class, door)
                    .getMessage()
                    .contains(limit));
        }

        UUID[] uuids = new UUID[tooMany];
        assertTrue(assertThrows(IllegalArgumentException.class, () -> UuidGenerator.fillV7(uuids, MS))
                .getMessage()
                .contains(limit));
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.fillV7(uuids));
        assertTrue(Arrays.stream(uuids, 0, 64).allMatch(u -> u == null));

        byte[] bytes = new byte[tooMany * 16];
        assertTrue(assertThrows(IllegalArgumentException.class, () -> UuidGenerator.fillV7(bytes, MS))
                .getMessage()
                .contains(limit));
        assertThrows(IllegalArgumentException.class, () -> UuidGenerator.fillV7(bytes));
        assertArrayEquals(new byte[64], Arrays.copyOf(bytes, 64));

        assertEquals(1 << 26, UuidGenerator.MAX_V7_BATCH);
    }

    // The core's own return codes map to argument errors, not to the random source: code 4
    // is the limit above, code 3 a batch too large to address (a 32-bit host only).
    @Test
    void batchFailureCodesMapToTheirMeaning() {
        assertTrue(UuidGenerator.batchFailure(4, "uuid_new_v7_batch", 7, "ts") instanceof IllegalArgumentException);
        assertTrue(UuidGenerator.batchFailure(4, "uuid_new_v7_batch", 7, "ts")
                .getMessage()
                .contains(Integer.toString(UuidGenerator.MAX_V7_BATCH)));
        assertTrue(UuidGenerator.batchFailure(3, "uuid_new_v7_batch", 7, "ts") instanceof IllegalArgumentException);
        assertEquals(
                "ts",
                UuidGenerator.batchFailure(2, "uuid_new_v7_batch", 7, "ts").getMessage());
        assertTrue(UuidGenerator.batchFailure(1, "uuid_new_v7_batch", 7, "ts") instanceof IllegalStateException);
    }

    // Exactly the counter space: 1 GiB of bytes, in strictly increasing order end to end. Minted
    // at the present, where the stream is: at a fixed old timestamp the call is out of stream and
    // unordered. A batch this size takes 2^20 steps: on an advance it fits at the supplied
    // millisecond, on a clamp it splits to the stream's next one, and the core refuses a split
    // more than 1000 ms ahead of the supplied timestamp, so the last item is within 1000 ms.
    @Test
    void aV7BatchOfExactlyTheCounterSpaceIsStrictlyIncreasing() {
        long now = System.currentTimeMillis();
        byte[] bytes = new byte[UuidGenerator.MAX_V7_BATCH * 16];
        UuidGenerator.fillV7(bytes, now);
        for (int i = 1; i < UuidGenerator.MAX_V7_BATCH; i++) {
            int at = i * 16;
            if (Arrays.compareUnsigned(bytes, at - 16, at, bytes, at, at + 16) >= 0) {
                throw new AssertionError("item " + i + " does not sort after item " + (i - 1));
            }
        }
        long first = UuidGenerator.v7UnixMillis(RfcBytes.fromRfcBytes(bytes, 0));
        assertTrue(first >= now && first <= now + 11_000, "first item at " + first + ", supplied " + now);
        long last = UuidGenerator.v7UnixMillis(RfcBytes.fromRfcBytes(bytes, bytes.length - 16));
        assertTrue(last >= now && last <= now + 1_000, "last item at " + last + ", supplied " + now);
    }
}
