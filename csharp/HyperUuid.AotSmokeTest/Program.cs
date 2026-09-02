using HyperUuid;

var a = UuidGenerator.NewV4();
var b = UuidGenerator.NewV4();
if (a == b)
{
    Console.WriteLine("FAIL: two v4 calls produced identical output");
    return 1;
}

var v5 = UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "www.example.com");
if (v5 != new Guid("2ed6657d-e927-568b-95e1-2665a8aea6a2"))
{
    Console.WriteLine($"FAIL: v5 mismatch, got {v5}");
    return 1;
}

var v7 = UuidGenerator.NewV7(1_645_557_742_000L);
Span<byte> rfc = stackalloc byte[16];
v7.TryWriteBytes(rfc, bigEndian: true, out _);
var ms = ((long)rfc[0] << 40) | ((long)rfc[1] << 32) | ((long)rfc[2] << 24) |
          ((long)rfc[3] << 16) | ((long)rfc[4] << 8) | rfc[5];
if (ms != 1_645_557_742_000L)
{
    Console.WriteLine($"FAIL: v7 timestamp mismatch, got {ms}");
    return 1;
}

// The C# 14 extension property. It lowers to an ordinary static call, which is precisely why
// it belongs here: "lowers to something trim-safe" is the claim, and this is where claims about
// the published surface get checked instead of asserted. Both arms matter — the value case and
// the null case are different code paths through the version nibble.
if (v7.Timestamp != DateTimeOffset.FromUnixTimeMilliseconds(1_645_557_742_000L))
{
    Console.WriteLine($"FAIL: Guid.Timestamp mismatch, got {v7.Timestamp}");
    return 1;
}
if (a.Timestamp is not null)
{
    Console.WriteLine($"FAIL: Guid.Timestamp returned {a.Timestamp} for a v4 UUID");
    return 1;
}

// Non-throwing construction path — must be AOT-clean too, and must actually report failure
// rather than throw (2^48 ms overflows v7's 48-bit unix_ts_ms field).
if (!UuidGenerator.TryNewV7(1_645_557_742_000L, out var tryV7) || tryV7 == Guid.Empty)
{
    Console.WriteLine("FAIL: TryNewV7 rejected a valid timestamp");
    return 1;
}
if (UuidGenerator.TryNewV7(1L << 48, out var overflowed) || overflowed != Guid.Empty)
{
    Console.WriteLine("FAIL: TryNewV7 accepted an out-of-range timestamp");
    return 1;
}

// Raw-byte SQL-order transform — the byte overload must agree with the Guid overload.
Span<byte> sqlBytes = stackalloc byte[16];
v7.TryWriteBytes(sqlBytes, bigEndian: true, out _);
UuidGenerator.V7ToSqlOrder(sqlBytes);
if (!sqlBytes.SequenceEqual(UuidGenerator.V7ToSqlOrder(v7).ToByteArray()))
{
    Console.WriteLine("FAIL: V7ToSqlOrder byte overload disagrees with the Guid overload");
    return 1;
}

// Destination-buffer batch fill, raw bytes: one native call, zero managed per-element work.
Span<byte> batch = stackalloc byte[4 * 16];
UuidGenerator.FillV7(batch, 1_645_557_742_000L);
for (var i = 0; i < 4; i++)
{
    var item = new Guid(batch.Slice(i * 16, 16), bigEndian: true);
    if (UuidGenerator.V7UnixMillis(item) != 1_645_557_742_000L)
    {
        Console.WriteLine($"FAIL: batch item {i} carried the wrong timestamp");
        return 1;
    }
}

Console.WriteLine($"v4: {a} {b}");
Console.WriteLine($"v5: {v5} matches RFC 9562 Appendix A.4 vector");
Console.WriteLine($"v7: {v7} embeds timestamp {ms}");
Console.WriteLine();
Console.WriteLine("try/span/batch/extension surface verified under Native AOT");
Console.WriteLine("ALL NATIVE AOT CHECKS PASSED");
return 0;
