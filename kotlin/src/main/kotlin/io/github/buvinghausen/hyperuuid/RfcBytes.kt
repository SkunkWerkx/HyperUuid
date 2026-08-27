package io.github.buvinghausen.hyperuuid

import java.util.UUID

/**
 * Converts 16 RFC 9562 network-byte-order bytes into a [UUID]. `java.util.UUID`'s
 * `mostSignificantBits`/`leastSignificantBits` already decompose into this exact byte order,
 * so this is just two shift loops — no swapping, unlike .NET's `Guid`.
 */
internal fun uuidFromRfcBytes(bytes: ByteArray): UUID {
    var msb = 0L
    for (i in 0 until 8) msb = (msb shl 8) or (bytes[i].toLong() and 0xFF)
    var lsb = 0L
    for (i in 8 until 16) lsb = (lsb shl 8) or (bytes[i].toLong() and 0xFF)
    return UUID(msb, lsb)
}

/** Converts a [UUID] into its 16 RFC 9562 network-byte-order bytes. */
internal fun UUID.toRfcBytes(): ByteArray {
    val bytes = ByteArray(16)
    var msb = mostSignificantBits
    for (i in 7 downTo 0) {
        bytes[i] = (msb and 0xFF).toByte()
        msb = msb ushr 8
    }
    var lsb = leastSignificantBits
    for (i in 15 downTo 8) {
        bytes[i] = (lsb and 0xFF).toByte()
        lsb = lsb ushr 8
    }
    return bytes
}
