package io.github.buvinghausen.hyperuuid

import java.lang.foreign.Arena
import java.lang.foreign.FunctionDescriptor
import java.lang.foreign.Linker
import java.lang.foreign.MemorySegment
import java.lang.foreign.SymbolLookup
import java.lang.foreign.ValueLayout
import java.lang.invoke.MethodHandle
import java.nio.charset.Charset
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import java.util.UUID

/**
 * RFC 9562 UUID generation (v4 random, v5 deterministic, v7 time-sortable) calling directly
 * into the native `libhyperuuid` shared library via the Java Foreign Function & Memory API
 * (stable since JDK 22 / JEP 454) — no runtime bridge.
 *
 * No allocation beyond the confined [Arena] scratch segments here — the underlying Rust core
 * never allocates for these calls either. This is JVM-only: `java.lang.foreign` doesn't exist
 * outside the JVM, so this does not extend to a browser target the way the C#/Blazor P/Invoke
 * path does — Kotlin's actual browser story (Kotlin/Wasm, the `wasmJs` target) is a different
 * compiler backend entirely and would need its own JS-interop-based binding. This jar bundles
 * a native build for every platform (see [NativePlatform]) and picks the right one at runtime.
 */
object UuidGenerator {

    /** Well-known namespace UUIDs defined in RFC 9562 Section 6.6. */
    object Namespaces {
        val DNS: UUID = UUID.fromString("6ba7b810-9dad-11d1-80b4-00c04fd430c8")
        val URL: UUID = UUID.fromString("6ba7b811-9dad-11d1-80b4-00c04fd430c8")
        val OID: UUID = UUID.fromString("6ba7b812-9dad-11d1-80b4-00c04fd430c8")
        val X500: UUID = UUID.fromString("6ba7b814-9dad-11d1-80b4-00c04fd430c8")
    }

    private val linker = Linker.nativeLinker()

    private val lookup: SymbolLookup = run {
        // The library must outlive every downcall made through it, so it's loaded into the
        // JDK-provided arena that lives for the process's lifetime rather than one we'd have
        // to remember to keep a reference to.
        val resourcePath = NativePlatform.resourcePath
        val resource = requireNotNull(UuidGenerator::class.java.getResourceAsStream(resourcePath)) {
            "$resourcePath classpath resource not found (unsupported platform, or this jar was " +
                "built without a native library for it)"
        }
        val extension = "." + NativePlatform.current.libraryFileName.substringAfterLast('.')
        val tmp = Files.createTempFile("hyperuuid", extension)
        tmp.toFile().deleteOnExit()
        resource.use { input -> Files.copy(input, tmp, StandardCopyOption.REPLACE_EXISTING) }
        SymbolLookup.libraryLookup(tmp, Arena.global())
    }

    private val uuidNewV4: MethodHandle = linker.downcallHandle(
        lookup.find("uuid_new_v4").get(),
        FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS),
    )
    private val uuidNewV5: MethodHandle = linker.downcallHandle(
        lookup.find("uuid_new_v5").get(),
        FunctionDescriptor.of(
            ValueLayout.JAVA_INT,
            ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_INT, ValueLayout.ADDRESS,
        ),
    )
    private val uuidNewV7: MethodHandle = linker.downcallHandle(
        lookup.find("uuid_new_v7").get(),
        FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS),
    )
    private val uuidV7UnixMillis: MethodHandle = linker.downcallHandle(
        lookup.find("uuid_v7_unix_millis").get(),
        FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS),
    )

    /** Creates a random UUID version 4 (RFC 9562 §5.4). */
    @JvmStatic
    fun newV4(): UUID = Arena.ofConfined().use { local ->
        val out = local.allocate(16)
        val rc = uuidNewV4.invokeExact(out) as Int
        check(rc == 0) { "uuid_new_v4 failed with code $rc (random source failure)" }
        readUuid(out)
    }

    /**
     * Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a name. The
     * same (namespace, name) pair always produces the same UUID.
     */
    @JvmStatic
    @JvmOverloads
    fun newV5(namespace: UUID, name: String, charset: Charset = Charsets.UTF_8): UUID =
        newV5(namespace, name.toByteArray(charset))

    /**
     * Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and raw name
     * bytes. The same (namespace, name) pair always produces the same UUID.
     */
    @JvmStatic
    fun newV5(namespace: UUID, name: ByteArray): UUID = Arena.ofConfined().use { local ->
        val nsSeg = local.allocate(16)
        MemorySegment.copy(namespace.toRfcBytes(), 0, nsSeg, ValueLayout.JAVA_BYTE, 0, 16)

        val nameSeg = if (name.isEmpty()) {
            MemorySegment.NULL
        } else {
            local.allocate(name.size.toLong()).also {
                MemorySegment.copy(name, 0, it, ValueLayout.JAVA_BYTE, 0, name.size)
            }
        }

        val out = local.allocate(16)
        val rc = uuidNewV5.invokeExact(nsSeg, nameSeg, name.size, out) as Int
        check(rc == 0) { "uuid_new_v5 failed with code $rc" }
        readUuid(out)
    }

    /** Creates a time-sortable UUID version 7 (RFC 9562 §6.2) using the current time. */
    @JvmStatic
    @JvmOverloads
    fun newV7(unixMillis: Long = System.currentTimeMillis()): UUID = Arena.ofConfined().use { local ->
        val out = local.allocate(16)
        when (val rc = uuidNewV7.invokeExact(unixMillis, out) as Int) {
            0 -> readUuid(out)
            2 -> throw IllegalArgumentException("unixMillis must be non-negative and fit within 48 bits")
            else -> error("uuid_new_v7 failed with code $rc (random source failure)")
        }
    }

    /**
     * Recovers the Unix-epoch millisecond timestamp embedded in a version 7 UUID's
     * `unix_ts_ms` field. Only meaningful when [uuid]'s version nibble is 7 — the RFC 9562
     * bit layout doesn't distinguish "not a v7 UUID" from "v7 UUID with a very early
     * timestamp", so the caller is responsible for checking that first if it matters.
     */
    @JvmStatic
    fun v7UnixMillis(uuid: UUID): Long = Arena.ofConfined().use { local ->
        val seg = local.allocate(16)
        MemorySegment.copy(uuid.toRfcBytes(), 0, seg, ValueLayout.JAVA_BYTE, 0, 16)
        uuidV7UnixMillis.invokeExact(seg) as Long
    }

    private fun readUuid(segment: MemorySegment): UUID = uuidFromRfcBytes(segment.toArray(ValueLayout.JAVA_BYTE))
}

/**
 * Recovers the UTC timestamp embedded in a version 7 UUID as an [java.time.Instant]. Throws
 * [java.time.DateTimeException] for a (spec-valid) embedded timestamp past year 999,999,999 —
 * far beyond the RFC's own 48-bit ceiling (year 10889), so this is unreachable in practice
 * for any genuine version 7 UUID, unlike the corresponding Python/C# bindings.
 */
fun UUID.v7Timestamp(): java.time.Instant = java.time.Instant.ofEpochMilli(UuidGenerator.v7UnixMillis(this))
