package io.github.buvinghausen.hyperuuid

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNotEquals
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import java.time.Instant
import java.util.UUID

class UuidGeneratorTest {

    @Test
    fun `v4 has version and variant bits set`() {
        val id = UuidGenerator.newV4()
        assertEquals(4, id.version())
        assertEquals(2, id.variant())
    }

    @Test
    fun `v4 is non-deterministic`() {
        val results = (0 until 100).map { UuidGenerator.newV4() }.toSet()
        assertEquals(100, results.size)
    }

    // RFC 9562 Appendix A.4 official test vector.
    @Test
    fun `v5 matches rfc test vector`() {
        val id = UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "www.example.com")
        assertEquals(UUID.fromString("2ed6657d-e927-568b-95e1-2665a8aea6a2"), id)
    }

    // Python's `uuid` standard library documentation test vector.
    @Test
    fun `v5 matches python docs vector`() {
        val id = UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "python.org")
        assertEquals(UUID.fromString("886313e1-3b8a-5372-9b90-0c9aee199e5d"), id)
    }

    @Test
    fun `v5 is deterministic`() {
        val a = UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "same-name")
        val b = UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "same-name")
        assertEquals(a, b)
    }

    @Test
    fun `v5 different namespaces differ`() {
        val dns = UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "test")
        val url = UuidGenerator.newV5(UuidGenerator.Namespaces.URL, "test")
        assertNotEquals(dns, url)
    }

    // RFC 9562 Appendix A.6: 2022-02-22T19:22:22Z = 1645557742000 ms since epoch.
    private val rfcTestVectorMs = 1_645_557_742_000L

    @Test
    fun `v7 embeds the timestamp`() {
        val id = UuidGenerator.newV7(rfcTestVectorMs)
        val embeddedMs = (id.mostSignificantBits ushr 16) and 0xFFFF_FFFF_FFFFL
        assertEquals(rfcTestVectorMs, embeddedMs)
    }

    @Test
    fun `v7 has version and variant bits set`() {
        val id = UuidGenerator.newV7(rfcTestVectorMs)
        assertEquals(7, id.version())
        assertEquals(2, id.variant())
    }

    @Test
    fun `v7 overflow timestamp throws`() {
        assertThrows(IllegalArgumentException::class.java) {
            UuidGenerator.newV7(0x0001_0000_0000_0000L)
        }
    }

    @Test
    fun `v7 same millisecond batch is monotonically ordered`() {
        val ids = (0 until 100).map { UuidGenerator.newV7(rfcTestVectorMs) }
        assertEquals(ids, ids.sorted())
    }

    @Test
    fun `v7 current timestamp is embedded`() {
        val before = System.currentTimeMillis()
        val id = UuidGenerator.newV7()
        val after = System.currentTimeMillis()

        val embeddedMs = (id.mostSignificantBits ushr 16) and 0xFFFF_FFFF_FFFFL
        assertTrue(embeddedMs in before..after)
    }

    @Test
    fun `v7Timestamp recovers the exact millisecond`() {
        val id = UuidGenerator.newV7(rfcTestVectorMs)
        assertEquals(Instant.ofEpochMilli(rfcTestVectorMs), id.v7Timestamp())
    }

    @Test
    fun `v7Timestamp round-trips zero and the RFC 48-bit max`() {
        assertEquals(Instant.ofEpochMilli(0), UuidGenerator.newV7(0).v7Timestamp())

        val maxMs = 0x0000_FFFF_FFFF_FFFFL
        assertEquals(Instant.ofEpochMilli(maxMs), UuidGenerator.newV7(maxMs).v7Timestamp())
    }
}
