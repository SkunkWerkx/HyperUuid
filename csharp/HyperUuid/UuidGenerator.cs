using System.Buffers;
using System.Runtime.InteropServices;

namespace HyperUuid;

/// <summary>
/// RFC 9562 UUID generation (v4 random, v5 deterministic, v6/v7 time-sortable) calling directly
/// into the native <c>libhyperuuid</c> shared library via source-generated P/Invoke.
/// </summary>
/// <remarks>
/// No allocation beyond the fixed 16-byte stack buffers here — the underlying Rust core never
/// allocates for these calls either — confirmed empirically with BenchmarkDotNet's
/// <c>[MemoryDiagnoser]</c> in <c>HyperUuid.Benchmarks</c> (0 B for <c>NewV4</c>/<c>NewV5</c>/
/// <c>NewV6</c>/<c>NewV7</c>, including the <c>string</c>-based <see cref="NewV5(Guid, string)"/>
/// overload, which UTF-8-encodes into a 256-byte stack buffer with an <see cref="ArrayPool{T}"/>
/// fallback for longer names — the same pattern the batch methods already use, ported from this
/// project's own <c>SequentialGuid</c> library). AOT/trimming friendly: <see cref="LibraryImportAttribute"/>
/// is source-generated (no runtime reflection), so this type publishes cleanly under
/// <c>PublishAot</c>. Needs a platform-specific native binary — this build ships
/// <c>linux-arm64</c> only; every other native platform needs its own build.
/// <c>browser-wasm</c> (Blazor) is NOT one of those platforms this <c>[LibraryImport("hyperuuid")]</c>
/// surface works for as-is: a statically-linked WASM native has no separate <c>"hyperuuid"</c>
/// module to dlopen, so it needs <c>[LibraryImport("*")]</c> instead (resolve against the
/// current module) — proven working via a hand-written WASM-specific P/Invoke surface in
/// <c>HyperUuid.WasmSmokeTest/NativeWasm.cs</c>, not this type. See this package's own
/// README's WebAssembly (Blazor) section for the full story, including a real,
/// currently-open upstream blocker (dotnet/runtime#132858).
/// </remarks>
public static partial class UuidGenerator
{
    [LibraryImport("hyperuuid")]
    private static unsafe partial int uuid_new_v4(byte* outPtr);

    [LibraryImport("hyperuuid")]
    private static unsafe partial int uuid_new_v5(byte* nsPtr, byte* namePtr, uint nameLen, byte* outPtr);

    [LibraryImport("hyperuuid")]
    private static unsafe partial int uuid_new_v6(long unixMillis, byte* outPtr);

    [LibraryImport("hyperuuid")]
    private static unsafe partial ulong uuid_v6_unix_millis(byte* uuidPtr);

    [LibraryImport("hyperuuid")]
    private static unsafe partial int uuid_new_v6_batch(long unixMillis, uint count, byte* outPtr);

    [LibraryImport("hyperuuid")]
    private static unsafe partial int uuid_new_v7(long unixMillis, byte* outPtr);

    [LibraryImport("hyperuuid")]
    private static unsafe partial ulong uuid_v7_unix_millis(byte* uuidPtr);

    [LibraryImport("hyperuuid")]
    private static unsafe partial int uuid_new_v7_batch(long unixMillis, uint count, byte* outPtr);

    [LibraryImport("hyperuuid")]
    private static unsafe partial void uuid_v7_to_sql_order(byte* uuidPtr);

    [LibraryImport("hyperuuid")]
    private static unsafe partial void uuid_v7_to_rfc_order(byte* uuidPtr);

    [LibraryImport("hyperuuid")]
    private static unsafe partial void uuid_v6_to_sql_order(byte* uuidPtr);

    [LibraryImport("hyperuuid")]
    private static unsafe partial void uuid_v6_to_rfc_order(byte* uuidPtr);

    // Batch calls marshal through a byte scratch buffer rather than Span<Guid> directly —
    // Guid's in-memory field layout isn't RFC-byte-order (it's mixed-endian and not
    // guaranteed stable across runtimes), so each 16-byte chunk still needs the same
    // `new Guid(chunk, bigEndian: true)` conversion the single-item calls already do.
    // Fixed-size stackalloc + ArrayPool-above-threshold mirrors GuidV7.Fill in the
    // SequentialGuid library this crate is ported from.
    const int BatchStackThresholdBytes = 256;

    /// <summary>Well-known namespace UUIDs defined in RFC 9562 Section 6.6.</summary>
    public static class Namespaces
    {
        public static readonly Guid Dns = new("6ba7b810-9dad-11d1-80b4-00c04fd430c8");
        public static readonly Guid Url = new("6ba7b811-9dad-11d1-80b4-00c04fd430c8");
        public static readonly Guid Oid = new("6ba7b812-9dad-11d1-80b4-00c04fd430c8");
        public static readonly Guid X500 = new("6ba7b814-9dad-11d1-80b4-00c04fd430c8");
    }

    /// <summary>The RFC 9562 §5.9 Nil UUID — all 128 bits zero. Equivalent to <see cref="Guid.Empty"/>.</summary>
    public static readonly Guid Nil = Guid.Empty;

    /// <summary>The RFC 9562 §5.10 Max UUID — all 128 bits one.</summary>
    public static readonly Guid Max = new(new byte[]
    {
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    });

    /// <summary>Creates a random UUID version 4 (RFC 9562 §5.4).</summary>
    public static unsafe Guid NewV4()
    {
        Span<byte> buf = stackalloc byte[16];
        int rc;
        fixed (byte* p = buf)
        {
            rc = uuid_new_v4(p);
        }
        if (rc != 0)
            throw new InvalidOperationException($"uuid_new_v4 failed with code {rc} (random source failure).");
        return new Guid(buf, bigEndian: true);
    }

    /// <summary>Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a UTF-8 name.</summary>
    public static unsafe Guid NewV5(Guid namespaceId, string name)
    {
        var maxByteCount = System.Text.Encoding.UTF8.GetMaxByteCount(name.Length);
        Span<byte> stackBuf = stackalloc byte[BatchStackThresholdBytes];
        byte[]? rented = null;
        var buffer = maxByteCount <= BatchStackThresholdBytes
            ? stackBuf[..maxByteCount]
            : (rented = ArrayPool<byte>.Shared.Rent(maxByteCount)).AsSpan(0, maxByteCount);
        try
        {
            var len = System.Text.Encoding.UTF8.GetBytes(name, buffer);
            return NewV5(namespaceId, buffer[..len]);
        }
        finally
        {
            if (rented is not null) ArrayPool<byte>.Shared.Return(rented);
        }
    }

    /// <summary>Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and raw name bytes.</summary>
    public static unsafe Guid NewV5(Guid namespaceId, ReadOnlySpan<byte> name)
    {
        Span<byte> ns = stackalloc byte[16];
        namespaceId.TryWriteBytes(ns, bigEndian: true, out _);
        Span<byte> outBuf = stackalloc byte[16];

        int rc;
        fixed (byte* nsPtr = ns)
        fixed (byte* namePtr = name)
        fixed (byte* outPtr = outBuf)
        {
            rc = uuid_new_v5(nsPtr, name.IsEmpty ? null : namePtr, (uint)name.Length, outPtr);
        }
        if (rc != 0)
            throw new InvalidOperationException($"uuid_new_v5 failed with code {rc}.");
        return new Guid(outBuf, bigEndian: true);
    }

    /// <summary>
    /// Creates a time-sortable UUID version 6 (RFC 9562 §5.6), a field-compatible reordering
    /// of version 1 for better sort/index locality, using the current UTC time.
    /// </summary>
    public static Guid NewV6() => NewV6(DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());

    /// <summary>Creates a time-sortable UUID version 6 (RFC 9562 §5.6) from a <see cref="DateTimeOffset"/>.</summary>
    public static Guid NewV6(DateTimeOffset timestamp) => NewV6(timestamp.ToUnixTimeMilliseconds());

    /// <summary>
    /// Creates a time-sortable UUID version 6 (RFC 9562 §5.6) from a Unix-epoch millisecond
    /// timestamp. <c>clock_seq</c> and <c>node</c> are randomly generated on every call —
    /// unlike version 7, there is no monotonic counter, so calls within the same millisecond
    /// are not guaranteed to sort in creation order.
    /// </summary>
    public static unsafe Guid NewV6(long unixMilliseconds)
    {
        Span<byte> buf = stackalloc byte[16];
        int rc;
        fixed (byte* p = buf)
        {
            rc = uuid_new_v6(unixMilliseconds, p);
        }
        if (rc != 0)
        {
            throw rc switch
            {
                2 => new ArgumentOutOfRangeException(nameof(unixMilliseconds),
                    "Unix millisecond timestamp does not fit the 60-bit v6 timestamp field."),
                _ => new InvalidOperationException($"uuid_new_v6 failed with code {rc} (random source failure)."),
            };
        }
        return new Guid(buf, bigEndian: true);
    }

    /// <summary>
    /// Recovers the Unix-epoch millisecond timestamp embedded in a version 6 UUID's timestamp
    /// field. Only meaningful when <paramref name="uuid"/>'s version nibble is 6 — the RFC
    /// 9562 bit layout doesn't distinguish "not a v6 UUID" from "v6 UUID with a very early
    /// timestamp", so the caller is responsible for checking that first if it matters.
    /// </summary>
    public static unsafe long V6UnixMillis(Guid uuid)
    {
        Span<byte> bytes = stackalloc byte[16];
        uuid.TryWriteBytes(bytes, bigEndian: true, out _);
        fixed (byte* p = bytes)
        {
            return (long)uuid_v6_unix_millis(p);
        }
    }

    /// <summary>
    /// Recovers the UTC timestamp embedded in a version 6 UUID as a <see cref="DateTimeOffset"/>.
    /// Unlike <see cref="V7Timestamp"/>, this can't throw <see cref="ArgumentOutOfRangeException"/>:
    /// v6's 60-bit tick count, offset from the 1582 UUID epoch rather than 1970, tops out
    /// around the year 5236 — well short of <see cref="DateTimeOffset"/>'s own year-9999 ceiling.
    /// </summary>
    public static DateTimeOffset V6Timestamp(Guid uuid) =>
        DateTimeOffset.FromUnixTimeMilliseconds(V6UnixMillis(uuid));

    /// <summary>
    /// Fills <paramref name="destination"/> with time-sortable version 6 UUIDs sharing one
    /// timestamp capture — one native call and one random-bytes fetch instead of
    /// <paramref name="destination"/>'s length worth of each.
    /// </summary>
    public static unsafe void FillV6(Span<Guid> destination, long unixMilliseconds)
    {
        if (destination.IsEmpty)
            return;

        int totalBytes = destination.Length * 16;
        Span<byte> stackBuf = stackalloc byte[BatchStackThresholdBytes];
        byte[]? rented = null;
        Span<byte> buf = totalBytes <= BatchStackThresholdBytes
            ? stackBuf[..totalBytes]
            : (rented = ArrayPool<byte>.Shared.Rent(totalBytes)).AsSpan(0, totalBytes);
        try
        {
            int rc;
            fixed (byte* p = buf)
            {
                rc = uuid_new_v6_batch(unixMilliseconds, (uint)destination.Length, p);
            }
            if (rc != 0)
            {
                throw rc switch
                {
                    2 => new ArgumentOutOfRangeException(nameof(unixMilliseconds),
                        "Unix millisecond timestamp does not fit the 60-bit v6 timestamp field."),
                    _ => new InvalidOperationException(
                        $"uuid_new_v6_batch failed with code {rc} (random source failure)."),
                };
            }
            for (int i = 0; i < destination.Length; i++)
            {
                destination[i] = new Guid(buf.Slice(i * 16, 16), bigEndian: true);
            }
        }
        finally
        {
            if (rented is not null)
                ArrayPool<byte>.Shared.Return(rented);
        }
    }

    /// <summary>
    /// Fills <paramref name="destination"/> with time-sortable version 6 UUIDs using the
    /// current UTC time.
    /// </summary>
    public static void FillV6(Span<Guid> destination) =>
        FillV6(destination, DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());

    /// <summary>
    /// Creates an array of <paramref name="count"/> time-sortable version 6 UUIDs sharing one
    /// timestamp capture. <c>clock_seq</c> and <c>node</c> are randomly generated per item —
    /// unlike version 7, there is no monotonic counter, so items are not guaranteed to sort
    /// in creation order.
    /// </summary>
    public static Guid[] NewV6Batch(int count, long unixMilliseconds)
    {
        var result = new Guid[count];
        FillV6(result, unixMilliseconds);
        return result;
    }

    /// <summary>
    /// Creates an array of <paramref name="count"/> time-sortable version 6 UUIDs using the
    /// current UTC time.
    /// </summary>
    public static Guid[] NewV6Batch(int count) =>
        NewV6Batch(count, DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());

    /// <summary>Creates a time-sortable UUID version 7 (RFC 9562 §6.2) using the current UTC time.</summary>
    public static Guid NewV7() => NewV7(DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());

    /// <summary>Creates a time-sortable UUID version 7 (RFC 9562 §6.2) from a <see cref="DateTimeOffset"/>.</summary>
    public static Guid NewV7(DateTimeOffset timestamp) => NewV7(timestamp.ToUnixTimeMilliseconds());

    /// <summary>Creates a time-sortable UUID version 7 (RFC 9562 §6.2) from a Unix-epoch millisecond timestamp.</summary>
    public static unsafe Guid NewV7(long unixMilliseconds)
    {
        Span<byte> buf = stackalloc byte[16];
        int rc;
        fixed (byte* p = buf)
        {
            rc = uuid_new_v7(unixMilliseconds, p);
        }
        if (rc != 0)
        {
            throw rc switch
            {
                2 => new ArgumentOutOfRangeException(nameof(unixMilliseconds),
                    "Unix millisecond timestamp must be non-negative and fit within 48 bits."),
                _ => new InvalidOperationException($"uuid_new_v7 failed with code {rc} (random source failure)."),
            };
        }
        return new Guid(buf, bigEndian: true);
    }

    /// <summary>
    /// Recovers the Unix-epoch millisecond timestamp embedded in a version 7 UUID's
    /// <c>unix_ts_ms</c> field. Only meaningful when <paramref name="uuid"/>'s version nibble
    /// is 7 — the RFC 9562 bit layout doesn't distinguish "not a v7 UUID" from "v7 UUID with a
    /// very early timestamp", so the caller is responsible for checking that first if it matters.
    /// </summary>
    public static unsafe long V7UnixMillis(Guid uuid)
    {
        Span<byte> bytes = stackalloc byte[16];
        uuid.TryWriteBytes(bytes, bigEndian: true, out _);
        fixed (byte* p = bytes)
        {
            return (long)uuid_v7_unix_millis(p);
        }
    }

    /// <summary>
    /// Recovers the UTC timestamp embedded in a version 7 UUID as a <see cref="DateTimeOffset"/>.
    /// </summary>
    /// <exception cref="ArgumentOutOfRangeException">
    /// Thrown for a (spec-valid) embedded timestamp past year 9999 — the RFC's 48-bit
    /// millisecond field holds values up to the year 10889, but <see cref="DateTimeOffset"/>
    /// cannot represent a year beyond 9999.
    /// </exception>
    public static DateTimeOffset V7Timestamp(Guid uuid) =>
        DateTimeOffset.FromUnixTimeMilliseconds(V7UnixMillis(uuid));

    /// <summary>
    /// Converts an RFC 9562-ordered version 7 <paramref name="uuid"/> to the byte order SQL
    /// Server's <c>uniqueidentifier</c> needs on the wire to sort by creation order.
    /// </summary>
    /// <remarks>
    /// <see cref="System.Data.SqlTypes.SqlGuid"/> comparison — and therefore T-SQL
    /// <c>ORDER BY</c> on a <c>uniqueidentifier</c> column — doesn't compare a GUID's 16 bytes
    /// left to right; it uses a fixed, non-sequential byte significance order. This moves the
    /// timestamp and counter (the two fields that determine creation order) into that
    /// comparison's most-significant bytes, and moves the trailing entropy, which carries no
    /// ordering information, into the least-significant ones as one intact block. The result
    /// is exactly what <see cref="Guid.ToByteArray()"/> on the returned value needs to produce
    /// to sort correctly once written to SQL Server — pass the result straight through
    /// ADO.NET as you would any other <see cref="Guid"/> parameter. Same permutation this
    /// project's own <see href="https://github.com/NorseArchitecture/Svartalfheim">Svartalfheim</see>
    /// implements, ported here from the native Rust core instead of reimplemented in C#, so
    /// every binding in this repo (not just this one) gets it from one verified source.
    /// Meaningful only for a genuine version 7 UUID; see <see cref="V6ToSqlOrder"/> for v6.
    /// </remarks>
    public static unsafe Guid V7ToSqlOrder(Guid uuid)
    {
        Span<byte> bytes = stackalloc byte[16];
        uuid.TryWriteBytes(bytes, bigEndian: true, out _);
        fixed (byte* p = bytes)
        {
            uuid_v7_to_sql_order(p);
        }
        // Not bigEndian: true — the native call already rewrote these bytes into the exact
        // layout Guid.ToByteArray() needs to reproduce for SQL Server, so the default
        // constructor (no further byte-order conversion) is the correct one here.
        return new Guid(bytes);
    }

    /// <summary>
    /// Inverse of <see cref="V7ToSqlOrder"/> — converts a SQL-Server-ordered version 7
    /// <paramref name="uuid"/> (as read back via <see cref="Guid.ToByteArray()"/>) back to
    /// RFC 9562 order.
    /// </summary>
    public static unsafe Guid V7FromSqlOrder(Guid uuid)
    {
        Span<byte> bytes = stackalloc byte[16];
        // Not bigEndian: true — read back the same native layout V7ToSqlOrder wrote.
        uuid.TryWriteBytes(bytes);
        fixed (byte* p = bytes)
        {
            uuid_v7_to_rfc_order(p);
        }
        return new Guid(bytes, bigEndian: true);
    }

    /// <summary>
    /// Converts an RFC 9562-ordered version 6 <paramref name="uuid"/> to the byte order SQL
    /// Server's <c>uniqueidentifier</c> needs on the wire to sort by creation order.
    /// </summary>
    /// <remarks>
    /// Same <see cref="System.Data.SqlTypes.SqlGuid"/> significance order as
    /// <see cref="V7ToSqlOrder"/>, applied to v6's very different field layout. v6 has no
    /// monotonic counter the way v7 does; the only field that determines its creation order is
    /// the 60-bit timestamp itself, so this moves that whole timestamp — most significant
    /// chunk first — into the comparison's most significant bytes, and relocates
    /// <c>clock_seq</c>/<c>node</c> (no ordering value — randomly generated per call, not a
    /// counter) into the remaining bytes. Version and variant end up at different byte offsets
    /// than <see cref="V7ToSqlOrder"/>'s result (octet 8's top nibble and octet 6's top two
    /// bits here, not 7/8) — fine, since the two versions are separate methods and a caller
    /// always knows which one it's calling.
    /// <para>
    /// Unlike v7, two version 6 UUIDs minted at the same millisecond have identical timestamp
    /// bits — <c>clock_seq</c>/<c>node</c> are independently random, not a counter — so this
    /// doesn't (and can't) make same-millisecond v6 UUIDs sort in creation order any more than
    /// plain RFC order already does. Distinct timestamps sort correctly; same-timestamp ties
    /// don't, by the RFC's own v6 design, not a limitation introduced here.
    /// </para>
    /// Meaningful only for a genuine version 6 UUID.
    /// </remarks>
    public static unsafe Guid V6ToSqlOrder(Guid uuid)
    {
        Span<byte> bytes = stackalloc byte[16];
        uuid.TryWriteBytes(bytes, bigEndian: true, out _);
        fixed (byte* p = bytes)
        {
            uuid_v6_to_sql_order(p);
        }
        return new Guid(bytes);
    }

    /// <summary>
    /// Inverse of <see cref="V6ToSqlOrder"/> — converts a SQL-Server-ordered version 6
    /// <paramref name="uuid"/> (as read back via <see cref="Guid.ToByteArray()"/>) back to
    /// RFC 9562 order.
    /// </summary>
    public static unsafe Guid V6FromSqlOrder(Guid uuid)
    {
        Span<byte> bytes = stackalloc byte[16];
        uuid.TryWriteBytes(bytes);
        fixed (byte* p = bytes)
        {
            uuid_v6_to_rfc_order(p);
        }
        return new Guid(bytes, bigEndian: true);
    }

    /// <summary>
    /// Fills <paramref name="destination"/> with time-sortable version 7 UUIDs sharing one
    /// timestamp capture and one contiguous block of the monotonic counter — one native call
    /// and one random-bytes fetch instead of <paramref name="destination"/>'s length worth of
    /// each.
    /// </summary>
    public static unsafe void FillV7(Span<Guid> destination, long unixMilliseconds)
    {
        if (destination.IsEmpty)
            return;

        int totalBytes = destination.Length * 16;
        Span<byte> stackBuf = stackalloc byte[BatchStackThresholdBytes];
        byte[]? rented = null;
        Span<byte> buf = totalBytes <= BatchStackThresholdBytes
            ? stackBuf[..totalBytes]
            : (rented = ArrayPool<byte>.Shared.Rent(totalBytes)).AsSpan(0, totalBytes);
        try
        {
            int rc;
            fixed (byte* p = buf)
            {
                rc = uuid_new_v7_batch(unixMilliseconds, (uint)destination.Length, p);
            }
            if (rc != 0)
            {
                throw rc switch
                {
                    2 => new ArgumentOutOfRangeException(nameof(unixMilliseconds),
                        "Unix millisecond timestamp must be non-negative and fit within 48 bits."),
                    _ => new InvalidOperationException(
                        $"uuid_new_v7_batch failed with code {rc} (random source failure)."),
                };
            }
            for (int i = 0; i < destination.Length; i++)
            {
                destination[i] = new Guid(buf.Slice(i * 16, 16), bigEndian: true);
            }
        }
        finally
        {
            if (rented is not null)
                ArrayPool<byte>.Shared.Return(rented);
        }
    }

    /// <summary>
    /// Fills <paramref name="destination"/> with time-sortable version 7 UUIDs using the
    /// current UTC time.
    /// </summary>
    public static void FillV7(Span<Guid> destination) =>
        FillV7(destination, DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());

    /// <summary>
    /// Creates an array of <paramref name="count"/> time-sortable version 7 UUIDs sharing one
    /// timestamp capture.
    /// </summary>
    public static Guid[] NewV7Batch(int count, long unixMilliseconds)
    {
        var result = new Guid[count];
        FillV7(result, unixMilliseconds);
        return result;
    }

    /// <summary>
    /// Creates an array of <paramref name="count"/> time-sortable version 7 UUIDs using the
    /// current UTC time.
    /// </summary>
    public static Guid[] NewV7Batch(int count) =>
        NewV7Batch(count, DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
}
