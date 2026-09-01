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
/// One compiled assembly covers every platform including <c>browser-wasm</c> (Blazor) — no
/// separate build. Every native entry point is declared twice, unconditionally: once against
/// <c>"hyperuuid"</c> (resolved via <c>dlopen</c> on every real native platform), once against
/// <c>"*"</c> (resolves against the current module — the only thing that works for a
/// statically-linked WASM native, which has no separate module to dlopen), sharing the same
/// <see cref="LibraryImportAttribute.EntryPoint"/> so both point at the identical native
/// symbol. <see cref="OperatingSystem.IsBrowser"/> picks the right one at the call site — a
/// real runtime check, not just documentation, but one the .NET linker specifically knows how
/// to constant-fold per publish target (the same mechanism the BCL itself uses for
/// platform-conditional code), so a trimmed/published build still only ships the branch that
/// platform can actually reach, same as the old two-build split did — see
/// <c>HyperUuid.csproj</c>'s packaging targets for exactly how the single build lands in the
/// NuGet package. Proven working end-to-end in a real headless-browser session — see this
/// package's own README's WebAssembly (Blazor) section, including a real, currently-open
/// upstream blocker (dotnet/runtime#132858).
/// </remarks>
public static partial class UuidGenerator
{
    [LibraryImport("hyperuuid", EntryPoint = "uuid_new_v4")]
    private static unsafe partial int uuid_new_v4_native(byte* outPtr);
    [LibraryImport("*", EntryPoint = "uuid_new_v4")]
    private static unsafe partial int uuid_new_v4_browser(byte* outPtr);
    private static unsafe int uuid_new_v4(byte* outPtr) =>
        OperatingSystem.IsBrowser() ? uuid_new_v4_browser(outPtr) : uuid_new_v4_native(outPtr);

    [LibraryImport("hyperuuid", EntryPoint = "uuid_new_v5")]
    private static unsafe partial int uuid_new_v5_native(byte* nsPtr, byte* namePtr, uint nameLen, byte* outPtr);
    [LibraryImport("*", EntryPoint = "uuid_new_v5")]
    private static unsafe partial int uuid_new_v5_browser(byte* nsPtr, byte* namePtr, uint nameLen, byte* outPtr);
    private static unsafe int uuid_new_v5(byte* nsPtr, byte* namePtr, uint nameLen, byte* outPtr) =>
        OperatingSystem.IsBrowser()
            ? uuid_new_v5_browser(nsPtr, namePtr, nameLen, outPtr)
            : uuid_new_v5_native(nsPtr, namePtr, nameLen, outPtr);

    [LibraryImport("hyperuuid", EntryPoint = "uuid_new_v6")]
    private static unsafe partial int uuid_new_v6_native(long unixMillis, byte* outPtr);
    [LibraryImport("*", EntryPoint = "uuid_new_v6")]
    private static unsafe partial int uuid_new_v6_browser(long unixMillis, byte* outPtr);
    private static unsafe int uuid_new_v6(long unixMillis, byte* outPtr) =>
        OperatingSystem.IsBrowser() ? uuid_new_v6_browser(unixMillis, outPtr) : uuid_new_v6_native(unixMillis, outPtr);

    [LibraryImport("hyperuuid", EntryPoint = "uuid_v6_unix_millis")]
    private static unsafe partial ulong uuid_v6_unix_millis_native(byte* uuidPtr);
    [LibraryImport("*", EntryPoint = "uuid_v6_unix_millis")]
    private static unsafe partial ulong uuid_v6_unix_millis_browser(byte* uuidPtr);
    private static unsafe ulong uuid_v6_unix_millis(byte* uuidPtr) =>
        OperatingSystem.IsBrowser() ? uuid_v6_unix_millis_browser(uuidPtr) : uuid_v6_unix_millis_native(uuidPtr);

    [LibraryImport("hyperuuid", EntryPoint = "uuid_new_v6_batch")]
    private static unsafe partial int uuid_new_v6_batch_native(long unixMillis, uint count, byte* outPtr);
    [LibraryImport("*", EntryPoint = "uuid_new_v6_batch")]
    private static unsafe partial int uuid_new_v6_batch_browser(long unixMillis, uint count, byte* outPtr);
    private static unsafe int uuid_new_v6_batch(long unixMillis, uint count, byte* outPtr) =>
        OperatingSystem.IsBrowser()
            ? uuid_new_v6_batch_browser(unixMillis, count, outPtr)
            : uuid_new_v6_batch_native(unixMillis, count, outPtr);

    [LibraryImport("hyperuuid", EntryPoint = "uuid_new_v7")]
    private static unsafe partial int uuid_new_v7_native(long unixMillis, byte* outPtr);
    [LibraryImport("*", EntryPoint = "uuid_new_v7")]
    private static unsafe partial int uuid_new_v7_browser(long unixMillis, byte* outPtr);
    private static unsafe int uuid_new_v7(long unixMillis, byte* outPtr) =>
        OperatingSystem.IsBrowser() ? uuid_new_v7_browser(unixMillis, outPtr) : uuid_new_v7_native(unixMillis, outPtr);

    [LibraryImport("hyperuuid", EntryPoint = "uuid_v7_unix_millis")]
    private static unsafe partial ulong uuid_v7_unix_millis_native(byte* uuidPtr);
    [LibraryImport("*", EntryPoint = "uuid_v7_unix_millis")]
    private static unsafe partial ulong uuid_v7_unix_millis_browser(byte* uuidPtr);
    private static unsafe ulong uuid_v7_unix_millis(byte* uuidPtr) =>
        OperatingSystem.IsBrowser() ? uuid_v7_unix_millis_browser(uuidPtr) : uuid_v7_unix_millis_native(uuidPtr);

    [LibraryImport("hyperuuid", EntryPoint = "uuid_new_v7_batch")]
    private static unsafe partial int uuid_new_v7_batch_native(long unixMillis, uint count, byte* outPtr);
    [LibraryImport("*", EntryPoint = "uuid_new_v7_batch")]
    private static unsafe partial int uuid_new_v7_batch_browser(long unixMillis, uint count, byte* outPtr);
    private static unsafe int uuid_new_v7_batch(long unixMillis, uint count, byte* outPtr) =>
        OperatingSystem.IsBrowser()
            ? uuid_new_v7_batch_browser(unixMillis, count, outPtr)
            : uuid_new_v7_batch_native(unixMillis, count, outPtr);

    [LibraryImport("hyperuuid", EntryPoint = "uuid_v7_to_sql_order")]
    private static unsafe partial void uuid_v7_to_sql_order_native(byte* uuidPtr);
    [LibraryImport("*", EntryPoint = "uuid_v7_to_sql_order")]
    private static unsafe partial void uuid_v7_to_sql_order_browser(byte* uuidPtr);
    private static unsafe void uuid_v7_to_sql_order(byte* uuidPtr)
    {
        if (OperatingSystem.IsBrowser()) uuid_v7_to_sql_order_browser(uuidPtr);
        else uuid_v7_to_sql_order_native(uuidPtr);
    }

    [LibraryImport("hyperuuid", EntryPoint = "uuid_v7_to_rfc_order")]
    private static unsafe partial void uuid_v7_to_rfc_order_native(byte* uuidPtr);
    [LibraryImport("*", EntryPoint = "uuid_v7_to_rfc_order")]
    private static unsafe partial void uuid_v7_to_rfc_order_browser(byte* uuidPtr);
    private static unsafe void uuid_v7_to_rfc_order(byte* uuidPtr)
    {
        if (OperatingSystem.IsBrowser()) uuid_v7_to_rfc_order_browser(uuidPtr);
        else uuid_v7_to_rfc_order_native(uuidPtr);
    }

    [LibraryImport("hyperuuid", EntryPoint = "uuid_v6_to_sql_order")]
    private static unsafe partial void uuid_v6_to_sql_order_native(byte* uuidPtr);
    [LibraryImport("*", EntryPoint = "uuid_v6_to_sql_order")]
    private static unsafe partial void uuid_v6_to_sql_order_browser(byte* uuidPtr);
    private static unsafe void uuid_v6_to_sql_order(byte* uuidPtr)
    {
        if (OperatingSystem.IsBrowser()) uuid_v6_to_sql_order_browser(uuidPtr);
        else uuid_v6_to_sql_order_native(uuidPtr);
    }

    [LibraryImport("hyperuuid", EntryPoint = "uuid_v6_to_rfc_order")]
    private static unsafe partial void uuid_v6_to_rfc_order_native(byte* uuidPtr);
    [LibraryImport("*", EntryPoint = "uuid_v6_to_rfc_order")]
    private static unsafe partial void uuid_v6_to_rfc_order_browser(byte* uuidPtr);
    private static unsafe void uuid_v6_to_rfc_order(byte* uuidPtr)
    {
        if (OperatingSystem.IsBrowser()) uuid_v6_to_rfc_order_browser(uuidPtr);
        else uuid_v6_to_rfc_order_native(uuidPtr);
    }

    // Batch calls marshal through a byte scratch buffer rather than Span<Guid> directly —
    // Guid's in-memory field layout isn't RFC-byte-order (it's mixed-endian and not
    // guaranteed stable across runtimes), so each 16-byte chunk still needs the same
    // `new Guid(chunk, bigEndian: true)` conversion the single-item calls already do.
    // Fixed-size stackalloc + ArrayPool-above-threshold mirrors GuidV7.Fill in the
    // SequentialGuid library this crate is ported from.
    const int BatchStackThresholdBytes = 256;

    static void RequireWholeUuids(Span<byte> destination, string paramName)
    {
        if (destination.Length % 16 != 0)
            throw new ArgumentException(
                $"Destination length must be a multiple of 16 (one whole UUID per 16 bytes); got {destination.Length}.",
                paramName);
    }

    static void ThrowOnBatchFailure(int rc, string entryPoint, string outOfRangeMessage)
    {
        if (rc == 0)
            return;
        throw rc switch
        {
            2 => new ArgumentOutOfRangeException("unixMilliseconds", outOfRangeMessage),
            _ => new InvalidOperationException($"{entryPoint} failed with code {rc} (random source failure)."),
        };
    }

    /// <summary>Well-known namespace UUIDs defined in RFC 9562 Section 6.6.</summary>
    public static class Namespaces
    {
        /// <summary>The DNS namespace UUID.</summary>
        public static readonly Guid Dns = new("6ba7b810-9dad-11d1-80b4-00c04fd430c8");
        /// <summary>The URL namespace UUID.</summary>
        public static readonly Guid Url = new("6ba7b811-9dad-11d1-80b4-00c04fd430c8");
        /// <summary>The ISO OID namespace UUID.</summary>
        public static readonly Guid Oid = new("6ba7b812-9dad-11d1-80b4-00c04fd430c8");
        /// <summary>The X.500 DN namespace UUID.</summary>
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
    public static Guid NewV4()
    {
        var rc = CoreNewV4(out var result);
        if (rc != 0)
            throw new InvalidOperationException($"uuid_new_v4 failed with code {rc} (random source failure).");
        return result;
    }

    /// <summary>
    /// Non-throwing counterpart to <see cref="NewV4"/> — returns <see langword="false"/> and
    /// leaves <paramref name="result"/> as <see cref="Guid.Empty"/> if the native random source
    /// fails, instead of throwing.
    /// </summary>
    /// <remarks>
    /// No exception ever crosses the P/Invoke boundary in either form — the native layer signals
    /// failure with an <c>int</c> return code (0 success, 1 random-source failure, 2 timestamp out
    /// of range; see <c>rust/src/ffi.rs</c>) and it's the managed wrapper that decides whether to
    /// translate that code into a <see langword="throw"/>. This overload simply doesn't, which is
    /// what a <c>Result</c>-shaped call site wants: a failure it can branch on without paying for
    /// a managed exception or wrapping every call in a <c>try</c>/<c>catch</c>.
    /// </remarks>
    public static bool TryNewV4(out Guid result) => CoreNewV4(out result) == 0;

    static unsafe int CoreNewV4(out Guid result)
    {
        Span<byte> buf = stackalloc byte[16];
        int rc;
        fixed (byte* p = buf)
        {
            rc = uuid_new_v4(p);
        }
        result = rc == 0 ? new Guid(buf, bigEndian: true) : default;
        return rc;
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
    public static Guid NewV6(long unixMilliseconds)
    {
        var rc = CoreNewV6(unixMilliseconds, out var result);
        if (rc != 0)
        {
            throw rc switch
            {
                2 => new ArgumentOutOfRangeException(nameof(unixMilliseconds),
                    "Unix millisecond timestamp does not fit the 60-bit v6 timestamp field."),
                _ => new InvalidOperationException($"uuid_new_v6 failed with code {rc} (random source failure)."),
            };
        }
        return result;
    }

    /// <summary>
    /// Non-throwing counterpart to <see cref="NewV6(long)"/> — returns <see langword="false"/> for
    /// both failure modes the native layer reports (random-source failure, and a
    /// <paramref name="unixMilliseconds"/> that doesn't fit the 60-bit v6 timestamp field) rather
    /// than throwing. See <see cref="TryNewV4"/> for why this is the cheaper shape at a
    /// <c>Result</c>-style call site.
    /// </summary>
    public static bool TryNewV6(long unixMilliseconds, out Guid result) =>
        CoreNewV6(unixMilliseconds, out result) == 0;

    /// <summary>
    /// Non-throwing counterpart to <see cref="NewV6()"/>, using the current UTC time.
    /// </summary>
    public static bool TryNewV6(out Guid result) =>
        TryNewV6(DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(), out result);

    static unsafe int CoreNewV6(long unixMilliseconds, out Guid result)
    {
        Span<byte> buf = stackalloc byte[16];
        int rc;
        fixed (byte* p = buf)
        {
            rc = uuid_new_v6(unixMilliseconds, p);
        }
        result = rc == 0 ? new Guid(buf, bigEndian: true) : default;
        return rc;
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
    public static void FillV6(Span<Guid> destination, long unixMilliseconds) =>
        ThrowOnBatchFailure(CoreFillV6(destination, unixMilliseconds), "uuid_new_v6_batch",
            "Unix millisecond timestamp does not fit the 60-bit v6 timestamp field.");

    /// <summary>
    /// Non-throwing counterpart to <see cref="FillV6(Span{Guid}, long)"/> — returns
    /// <see langword="false"/> instead of throwing when the native call reports a random-source
    /// failure or an out-of-range <paramref name="unixMilliseconds"/>. On failure
    /// <paramref name="destination"/> is left untouched. See <see cref="TryNewV4"/> for why.
    /// </summary>
    public static bool TryFillV6(Span<Guid> destination, long unixMilliseconds) =>
        CoreFillV6(destination, unixMilliseconds) == 0;

    /// <summary>
    /// Fills <paramref name="destination"/> with raw RFC 9562-ordered version 6 UUID bytes —
    /// 16 per UUID, contiguous, no <see cref="Guid"/> anywhere on the path.
    /// </summary>
    /// <remarks>
    /// This is the allocation-free, conversion-free form of
    /// <see cref="FillV6(Span{Guid}, long)"/>: the native core already writes the batch as one
    /// contiguous block of RFC-ordered bytes, so handing it the caller's own buffer means one
    /// native call and <em>zero</em> managed per-element work. The <see cref="Guid"/> overload has
    /// to marshal through a scratch buffer and run a
    /// <c>new Guid(chunk, bigEndian: true)</c> conversion per element, because
    /// <see cref="Guid"/>'s in-memory field layout is mixed-endian and isn't the RFC byte order.
    /// Prefer this overload when the destination is a wire buffer, a database parameter, or
    /// anything else that wants RFC bytes rather than <see cref="Guid"/> values.
    /// <para>
    /// <paramref name="destination"/>'s length must be an exact multiple of 16; anything else is a
    /// caller error and throws.
    /// </para>
    /// </remarks>
    /// <exception cref="ArgumentException">
    /// <paramref name="destination"/>'s length is not a multiple of 16.
    /// </exception>
    public static void FillV6(Span<byte> destination, long unixMilliseconds)
    {
        RequireWholeUuids(destination, nameof(destination));
        ThrowOnBatchFailure(CoreFillV6Bytes(destination, unixMilliseconds), "uuid_new_v6_batch",
            "Unix millisecond timestamp does not fit the 60-bit v6 timestamp field.");
    }

    /// <summary>
    /// Non-throwing counterpart to <see cref="FillV6(Span{byte}, long)"/>. Returns
    /// <see langword="false"/> — rather than throwing — for a native failure <em>and</em> for a
    /// <paramref name="destination"/> whose length isn't a multiple of 16, matching the BCL's own
    /// <c>Try</c> convention (<see cref="Guid.TryWriteBytes(Span{byte})"/> likewise returns
    /// <see langword="false"/> for a badly sized destination rather than throwing).
    /// </summary>
    public static bool TryFillV6(Span<byte> destination, long unixMilliseconds) =>
        destination.Length % 16 == 0 && CoreFillV6Bytes(destination, unixMilliseconds) == 0;

    static unsafe int CoreFillV6(Span<Guid> destination, long unixMilliseconds)
    {
        if (destination.IsEmpty)
            return 0;

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
                return rc;
            for (int i = 0; i < destination.Length; i++)
            {
                destination[i] = new Guid(buf.Slice(i * 16, 16), bigEndian: true);
            }
            return 0;
        }
        finally
        {
            if (rented is not null)
                ArrayPool<byte>.Shared.Return(rented);
        }
    }

    static unsafe int CoreFillV6Bytes(Span<byte> destination, long unixMilliseconds)
    {
        if (destination.IsEmpty)
            return 0;
        fixed (byte* p = destination)
        {
            return uuid_new_v6_batch(unixMilliseconds, (uint)(destination.Length / 16), p);
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
    public static Guid NewV7(long unixMilliseconds)
    {
        var rc = CoreNewV7(unixMilliseconds, out var result);
        if (rc != 0)
        {
            throw rc switch
            {
                2 => new ArgumentOutOfRangeException(nameof(unixMilliseconds),
                    "Unix millisecond timestamp must be non-negative and fit within 48 bits."),
                _ => new InvalidOperationException($"uuid_new_v7 failed with code {rc} (random source failure)."),
            };
        }
        return result;
    }

    /// <summary>
    /// Non-throwing counterpart to <see cref="NewV7(long)"/> — returns <see langword="false"/> for
    /// both failure modes the native layer reports (random-source failure, and a
    /// <paramref name="unixMilliseconds"/> that is negative or doesn't fit 48 bits) rather than
    /// throwing. See <see cref="TryNewV4"/> for why this is the cheaper shape at a
    /// <c>Result</c>-style call site.
    /// </summary>
    public static bool TryNewV7(long unixMilliseconds, out Guid result) =>
        CoreNewV7(unixMilliseconds, out result) == 0;

    /// <summary>
    /// Non-throwing counterpart to <see cref="NewV7()"/>, using the current UTC time.
    /// </summary>
    public static bool TryNewV7(out Guid result) =>
        TryNewV7(DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(), out result);

    static unsafe int CoreNewV7(long unixMilliseconds, out Guid result)
    {
        Span<byte> buf = stackalloc byte[16];
        int rc;
        fixed (byte* p = buf)
        {
            rc = uuid_new_v7(unixMilliseconds, p);
        }
        result = rc == 0 ? new Guid(buf, bigEndian: true) : default;
        return rc;
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
    /// Recovers the UTC timestamp embedded in <paramref name="uuid"/>, or <see langword="null"/>
    /// if it isn't a version 6 or 7 UUID. Unlike <see cref="V6Timestamp"/>/<see cref="V7Timestamp"/>,
    /// this reads the version nibble itself first, so a caller doesn't need to already know (or
    /// separately check) which version <paramref name="uuid"/> is before asking — delegates
    /// straight to whichever of those two methods applies, no bit-layout logic duplicated here.
    /// </summary>
    public static unsafe DateTimeOffset? GetTimestamp(Guid uuid)
    {
        Span<byte> bytes = stackalloc byte[16];
        uuid.TryWriteBytes(bytes, bigEndian: true, out _);
        return (bytes[6] >> 4) switch
        {
            6 => V6Timestamp(uuid),
            7 => V7Timestamp(uuid),
            _ => null,
        };
    }

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
    /// Meaningful only for a genuine version 7 UUID; see <see cref="V6ToSqlOrder(Guid)"/> for v6.
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
    /// Inverse of <see cref="V7ToSqlOrder(Guid)"/> — converts a SQL-Server-ordered version 7
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
    /// <see cref="V7ToSqlOrder(Guid)"/>, applied to v6's very different field layout. v6 has no
    /// monotonic counter the way v7 does; the only field that determines its creation order is
    /// the 60-bit timestamp itself, so this moves that whole timestamp — most significant
    /// chunk first — into the comparison's most significant bytes, and relocates
    /// <c>clock_seq</c>/<c>node</c> (no ordering value — randomly generated per call, not a
    /// counter) into the remaining bytes. Version and variant end up at different byte offsets
    /// than <see cref="V7ToSqlOrder(Guid)"/>'s result (octet 8's top nibble and octet 6's top two
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
    /// Inverse of <see cref="V6ToSqlOrder(Guid)"/> — converts a SQL-Server-ordered version 6
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

    // ---- Raw-byte SQL-order transforms -------------------------------------------------
    //
    // The four overloads below are the same native permutations as the Guid-taking methods
    // above, but operating directly on a caller's 16-byte buffer. They exist because the Guid
    // round trip is the one genuinely subtle part of this file: the Guid form reads its input
    // with `bigEndian: true` (RFC order) and then constructs its result with plain
    // `new Guid(bytes)` — no byte-order conversion — precisely so that a later
    // Guid.ToByteArray() reproduces the SQL-order bytes. That asymmetry is correct and
    // load-bearing, but it is only explicable in prose.
    //
    // On these overloads there is no asymmetry to explain, because there is no Guid: RFC-ordered
    // bytes go in, SQL-ordered bytes come out, in place. That makes them the form a byte-level
    // correctness oracle can be pointed at directly — notably this project's own
    // SequentialGuidBytes tests in Svartalfheim, which compare raw 16-byte permutations and
    // would otherwise have to model Guid's mixed-endian field layout just to compare results.
    // It also makes them the right call when the value is headed for a wire format or a database
    // parameter that wants bytes anyway, since the Guid detour is pure overhead there.

    /// <summary>
    /// In-place raw-byte form of <see cref="V7ToSqlOrder(Guid)"/> — rewrites the 16 RFC
    /// 9562-ordered version 7 bytes in <paramref name="uuid"/> into SQL Server
    /// <c>uniqueidentifier</c> sort order.
    /// </summary>
    /// <exception cref="ArgumentException"><paramref name="uuid"/> is not exactly 16 bytes.</exception>
    public static unsafe void V7ToSqlOrder(Span<byte> uuid)
    {
        RequireSingleUuid(uuid, nameof(uuid));
        fixed (byte* p = uuid) { uuid_v7_to_sql_order(p); }
    }

    /// <summary>
    /// In-place raw-byte form of <see cref="V7FromSqlOrder(Guid)"/> — rewrites the 16
    /// SQL-Server-ordered version 7 bytes in <paramref name="uuid"/> back into RFC 9562 order.
    /// </summary>
    /// <exception cref="ArgumentException"><paramref name="uuid"/> is not exactly 16 bytes.</exception>
    public static unsafe void V7FromSqlOrder(Span<byte> uuid)
    {
        RequireSingleUuid(uuid, nameof(uuid));
        fixed (byte* p = uuid) { uuid_v7_to_rfc_order(p); }
    }

    /// <summary>
    /// In-place raw-byte form of <see cref="V6ToSqlOrder(Guid)"/> — rewrites the 16 RFC
    /// 9562-ordered version 6 bytes in <paramref name="uuid"/> into SQL Server
    /// <c>uniqueidentifier</c> sort order.
    /// </summary>
    /// <exception cref="ArgumentException"><paramref name="uuid"/> is not exactly 16 bytes.</exception>
    public static unsafe void V6ToSqlOrder(Span<byte> uuid)
    {
        RequireSingleUuid(uuid, nameof(uuid));
        fixed (byte* p = uuid) { uuid_v6_to_sql_order(p); }
    }

    /// <summary>
    /// In-place raw-byte form of <see cref="V6FromSqlOrder(Guid)"/> — rewrites the 16
    /// SQL-Server-ordered version 6 bytes in <paramref name="uuid"/> back into RFC 9562 order.
    /// </summary>
    /// <exception cref="ArgumentException"><paramref name="uuid"/> is not exactly 16 bytes.</exception>
    public static unsafe void V6FromSqlOrder(Span<byte> uuid)
    {
        RequireSingleUuid(uuid, nameof(uuid));
        fixed (byte* p = uuid) { uuid_v6_to_rfc_order(p); }
    }

    static void RequireSingleUuid(Span<byte> uuid, string paramName)
    {
        if (uuid.Length != 16)
            throw new ArgumentException($"A UUID is exactly 16 bytes; got {uuid.Length}.", paramName);
    }

    /// <summary>
    /// Fills <paramref name="destination"/> with time-sortable version 7 UUIDs sharing one
    /// timestamp capture and one contiguous block of the monotonic counter — one native call
    /// and one random-bytes fetch instead of <paramref name="destination"/>'s length worth of
    /// each.
    /// </summary>
    public static void FillV7(Span<Guid> destination, long unixMilliseconds) =>
        ThrowOnBatchFailure(CoreFillV7(destination, unixMilliseconds), "uuid_new_v7_batch",
            "Unix millisecond timestamp must be non-negative and fit within 48 bits.");

    /// <summary>
    /// Non-throwing counterpart to <see cref="FillV7(Span{Guid}, long)"/> — returns
    /// <see langword="false"/> instead of throwing when the native call reports a random-source
    /// failure or an out-of-range <paramref name="unixMilliseconds"/>. On failure
    /// <paramref name="destination"/> is left untouched. See <see cref="TryNewV4"/> for why.
    /// </summary>
    public static bool TryFillV7(Span<Guid> destination, long unixMilliseconds) =>
        CoreFillV7(destination, unixMilliseconds) == 0;

    /// <summary>
    /// Fills <paramref name="destination"/> with raw RFC 9562-ordered version 7 UUID bytes —
    /// 16 per UUID, contiguous, no <see cref="Guid"/> anywhere on the path.
    /// </summary>
    /// <remarks>
    /// This is the allocation-free, conversion-free form of
    /// <see cref="FillV7(Span{Guid}, long)"/>: the native core already writes the batch as one
    /// contiguous block of RFC-ordered bytes, so handing it the caller's own buffer means one
    /// native call and <em>zero</em> managed per-element work. The <see cref="Guid"/> overload has
    /// to marshal through a scratch buffer and run a
    /// <c>new Guid(chunk, bigEndian: true)</c> conversion per element, because
    /// <see cref="Guid"/>'s in-memory field layout is mixed-endian and isn't the RFC byte order.
    /// Prefer this overload when the destination is a wire buffer, a database parameter, or
    /// anything else that wants RFC bytes rather than <see cref="Guid"/> values.
    /// <para>
    /// <paramref name="destination"/>'s length must be an exact multiple of 16; anything else is a
    /// caller error and throws.
    /// </para>
    /// </remarks>
    /// <exception cref="ArgumentException">
    /// <paramref name="destination"/>'s length is not a multiple of 16.
    /// </exception>
    public static void FillV7(Span<byte> destination, long unixMilliseconds)
    {
        RequireWholeUuids(destination, nameof(destination));
        ThrowOnBatchFailure(CoreFillV7Bytes(destination, unixMilliseconds), "uuid_new_v7_batch",
            "Unix millisecond timestamp must be non-negative and fit within 48 bits.");
    }

    /// <summary>
    /// Non-throwing counterpart to <see cref="FillV7(Span{byte}, long)"/>. Returns
    /// <see langword="false"/> — rather than throwing — for a native failure <em>and</em> for a
    /// <paramref name="destination"/> whose length isn't a multiple of 16, matching the BCL's own
    /// <c>Try</c> convention (<see cref="Guid.TryWriteBytes(Span{byte})"/> likewise returns
    /// <see langword="false"/> for a badly sized destination rather than throwing).
    /// </summary>
    public static bool TryFillV7(Span<byte> destination, long unixMilliseconds) =>
        destination.Length % 16 == 0 && CoreFillV7Bytes(destination, unixMilliseconds) == 0;

    static unsafe int CoreFillV7(Span<Guid> destination, long unixMilliseconds)
    {
        if (destination.IsEmpty)
            return 0;

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
                return rc;
            for (int i = 0; i < destination.Length; i++)
            {
                destination[i] = new Guid(buf.Slice(i * 16, 16), bigEndian: true);
            }
            return 0;
        }
        finally
        {
            if (rented is not null)
                ArrayPool<byte>.Shared.Return(rented);
        }
    }

    static unsafe int CoreFillV7Bytes(Span<byte> destination, long unixMilliseconds)
    {
        if (destination.IsEmpty)
            return 0;
        fixed (byte* p = destination)
        {
            return uuid_new_v7_batch(unixMilliseconds, (uint)(destination.Length / 16), p);
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
