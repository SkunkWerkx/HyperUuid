using System.Runtime.InteropServices;

namespace HyperUuid;

/// <summary>
/// RFC 9562 UUID generation (v4 random, v5 deterministic, v6/v7 time-sortable) calling directly
/// into the native <c>libhyperuuid</c> shared library via source-generated P/Invoke.
/// </summary>
/// <remarks>
/// No allocation beyond the fixed 16-byte stack buffers here — the underlying Rust core never
/// allocates for these calls either. AOT/trimming friendly: <see cref="LibraryImportAttribute"/>
/// is source-generated (no runtime reflection), so this type publishes cleanly under
/// <c>PublishAot</c>. Needs a platform-specific native binary — this build ships
/// <c>linux-arm64</c> only; every other platform (including <c>browser-wasm</c> for Blazor,
/// which uses this exact same P/Invoke surface statically linked into <c>dotnet.wasm</c> via
/// Emscripten) needs its own build.
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
    private static unsafe partial int uuid_new_v7(long unixMillis, byte* outPtr);

    [LibraryImport("hyperuuid")]
    private static unsafe partial ulong uuid_v7_unix_millis(byte* uuidPtr);

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
    public static Guid NewV5(Guid namespaceId, string name) =>
        NewV5(namespaceId, System.Text.Encoding.UTF8.GetBytes(name));

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
}
