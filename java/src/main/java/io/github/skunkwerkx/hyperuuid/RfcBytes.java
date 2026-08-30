package io.github.skunkwerkx.hyperuuid;

import java.util.UUID;

/**
 * Converts between {@link UUID} and its 16 RFC 9562 network-byte-order bytes.
 * {@code UUID}'s {@code getMostSignificantBits}/{@code getLeastSignificantBits} already
 * decompose into this exact byte order, so this is just two shift loops — no swapping,
 * unlike .NET's {@code Guid}.
 */
final class RfcBytes {
    private RfcBytes() {}

    static UUID fromRfcBytes(byte[] bytes) {
        long msb = 0;
        for (int i = 0; i < 8; i++) {
            msb = (msb << 8) | (bytes[i] & 0xFF);
        }
        long lsb = 0;
        for (int i = 8; i < 16; i++) {
            lsb = (lsb << 8) | (bytes[i] & 0xFF);
        }
        return new UUID(msb, lsb);
    }

    static byte[] toRfcBytes(UUID uuid) {
        byte[] bytes = new byte[16];
        long msb = uuid.getMostSignificantBits();
        for (int i = 7; i >= 0; i--) {
            bytes[i] = (byte) (msb & 0xFF);
            msb >>>= 8;
        }
        long lsb = uuid.getLeastSignificantBits();
        for (int i = 15; i >= 8; i--) {
            bytes[i] = (byte) (lsb & 0xFF);
            lsb >>>= 8;
        }
        return bytes;
    }
}
