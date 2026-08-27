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

Console.WriteLine($"v4: {a} {b}");
Console.WriteLine($"v5: {v5} matches RFC 9562 Appendix A.4 vector");
Console.WriteLine($"v7: {v7} embeds timestamp {ms}");
Console.WriteLine();
Console.WriteLine("ALL NATIVE AOT CHECKS PASSED");
return 0;
