// Proves the binding under Native AOT for real: this program publishes with PublishAot and
// crosses every native entry point the binding declares — the twelve uuid_* functions and
// hyperuuid_version — against the real native library, through every call shape the public
// surface offers (throwing, Try*, Guid, raw bytes, batch, extension property). Exit code 0 only
// if every check lands as expected.

using HyperUuid;

const long FixedMs = 1_645_557_742_000L;

// The probe must answer before any generator is the thing that finds out, and it must name the
// core it actually loaded — under AOT too, where a stale runtimes/ asset is the classic slip.
Console.WriteLine($"native: available={UuidGenerator.IsAvailable} version={UuidGenerator.NativeVersion}");
if (!UuidGenerator.IsAvailable || UuidGenerator.NativeVersion is null)
{
	Console.WriteLine("FAIL: the native library did not answer the version probe");
	return 1;
}

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
// The other two name shapes reach the same native call by different routes: UTF-16 text
// without a string, and raw bytes with no encode step at all. The empty name is the one input
// that crosses as a null pointer.
if (UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "www.example.com".AsSpan()) != v5
	|| UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "www.example.com"u8) != v5)
{
	Console.WriteLine("FAIL: the v5 char-span or byte overload disagrees with the string overload");
	return 1;
}
if (UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "") != new Guid("4ebd0208-8328-5d69-8c44-ec50939c0967"))
{
	Console.WriteLine("FAIL: v5 of an empty name mismatch");
	return 1;
}

var v7 = UuidGenerator.NewV7(FixedMs);
Span<byte> rfc = stackalloc byte[16];
v7.TryWriteBytes(rfc, bigEndian: true, out _);
var ms = ((long)rfc[0] << 40) | ((long)rfc[1] << 32) | ((long)rfc[2] << 24) |
		  ((long)rfc[3] << 16) | ((long)rfc[4] << 8) | rfc[5];
if (ms != FixedMs)
{
	Console.WriteLine($"FAIL: v7 timestamp mismatch, got {ms}");
	return 1;
}

// The C# 14 extension property. It lowers to an ordinary static call, which is precisely why
// it belongs here: "lowers to something trim-safe" is the claim, and this is where claims about
// the published surface get checked instead of asserted. Both arms matter — the value case and
// the null case are different code paths through the version nibble.
if (v7.Timestamp != DateTimeOffset.FromUnixTimeMilliseconds(FixedMs))
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
if (!UuidGenerator.TryNewV7(FixedMs, out var tryV7) || tryV7 == Guid.Empty)
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

// ...and back: the inverse transform is its own native entry point, in both shapes.
UuidGenerator.V7FromSqlOrder(sqlBytes);
if (!sqlBytes.SequenceEqual(rfc) || UuidGenerator.V7FromSqlOrder(UuidGenerator.V7ToSqlOrder(v7)) != v7)
{
	Console.WriteLine("FAIL: V7FromSqlOrder did not invert V7ToSqlOrder");
	return 1;
}

// Destination-buffer batch fill, raw bytes: one native call, zero managed per-element work.
Span<byte> batch = stackalloc byte[4 * 16];
UuidGenerator.FillV7(batch, FixedMs);
for (var i = 0; i < 4; i++)
{
	var item = new Guid(batch.Slice(i * 16, 16), bigEndian: true);
	if (UuidGenerator.V7UnixMillis(item) != FixedMs)
	{
		Console.WriteLine($"FAIL: batch item {i} carried the wrong timestamp");
		return 1;
	}
}

// The Guid-shaped batch forms: an allocated array past the stack scratch buffer (so the
// pooled path runs), strictly increasing within the millisecond, and the Try fill.
var v7Batch = UuidGenerator.NewV7Batch(100, FixedMs);
for (var i = 1; i < v7Batch.Length; i++)
{
	if (v7Batch[i].CompareTo(v7Batch[i - 1]) <= 0 || UuidGenerator.V7UnixMillis(v7Batch[i]) != FixedMs)
	{
		Console.WriteLine($"FAIL: v7 batch item {i} is out of order or carries the wrong timestamp");
		return 1;
	}
}
Span<Guid> filled = stackalloc Guid[4];
if (!UuidGenerator.TryFillV7(filled, FixedMs) || UuidGenerator.TryFillV7(filled, 1L << 48))
{
	Console.WriteLine("FAIL: TryFillV7 misreported a valid or an out-of-range timestamp");
	return 1;
}

// Version 6, end to end: generation, timestamp recovery, the Try path, both SQL-order
// directions in both shapes, and both batch shapes.
var v6 = UuidGenerator.NewV6(FixedMs);
if (UuidGenerator.V6UnixMillis(v6) != FixedMs || v6.Timestamp != DateTimeOffset.FromUnixTimeMilliseconds(FixedMs))
{
	Console.WriteLine($"FAIL: v6 timestamp mismatch for {v6}");
	return 1;
}
if (!UuidGenerator.TryNewV6(FixedMs, out var tryV6) || tryV6 == Guid.Empty
	|| UuidGenerator.TryNewV6(-1, out var negative) || negative != Guid.Empty)
{
	Console.WriteLine("FAIL: TryNewV6 misreported a valid or an out-of-range timestamp");
	return 1;
}
Span<byte> v6Bytes = stackalloc byte[16];
v6.TryWriteBytes(v6Bytes, bigEndian: true, out _);
UuidGenerator.V6ToSqlOrder(v6Bytes);
if (!v6Bytes.SequenceEqual(UuidGenerator.V6ToSqlOrder(v6).ToByteArray()))
{
	Console.WriteLine("FAIL: V6ToSqlOrder byte overload disagrees with the Guid overload");
	return 1;
}
UuidGenerator.V6FromSqlOrder(v6Bytes);
if (new Guid(v6Bytes, bigEndian: true) != v6 || UuidGenerator.V6FromSqlOrder(UuidGenerator.V6ToSqlOrder(v6)) != v6)
{
	Console.WriteLine("FAIL: V6FromSqlOrder did not invert V6ToSqlOrder");
	return 1;
}
var v6Batch = UuidGenerator.NewV6Batch(100, FixedMs);
Span<byte> v6Raw = stackalloc byte[4 * 16];
UuidGenerator.FillV6(v6Raw, FixedMs);
if (v6Batch.Distinct().Count() != 100
	|| v6Batch.Any(id => UuidGenerator.V6UnixMillis(id) != FixedMs)
	|| UuidGenerator.V6UnixMillis(new Guid(v6Raw.Slice(3 * 16, 16), bigEndian: true)) != FixedMs)
{
	Console.WriteLine("FAIL: a v6 batch is not distinct ids at the requested timestamp");
	return 1;
}

Console.WriteLine($"v4: {a} {b}");
Console.WriteLine($"v5: {v5} matches RFC 9562 Appendix A.4 vector");
Console.WriteLine($"v6: {v6} embeds timestamp {UuidGenerator.V6UnixMillis(v6)}");
Console.WriteLine($"v7: {v7} embeds timestamp {ms}");
Console.WriteLine();
Console.WriteLine("every native entry point crossed: try/span/batch/SQL-order/extension surface verified under Native AOT");
Console.WriteLine("ALL NATIVE AOT CHECKS PASSED");
return 0;
