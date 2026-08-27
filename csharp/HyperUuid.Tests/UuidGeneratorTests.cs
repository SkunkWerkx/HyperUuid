using Shouldly;

namespace HyperUuid.Tests;

public sealed class UuidGeneratorTests
{
    [Fact]
    public void V4_HasVersionAndVariantBitsSet()
    {
        var id = UuidGenerator.NewV4();
        id.ToString()[14].ShouldBe('4');
        (id.ToByteArray()[8] & 0xC0).ShouldBe(0x80);
    }

    [Fact]
    public void V4_IsNonDeterministic()
    {
        var results = Enumerable.Range(0, 100).Select(_ => UuidGenerator.NewV4()).ToHashSet();
        results.Count.ShouldBe(100);
    }

    // RFC 9562 Appendix A.4 official test vector.
    [Fact]
    public void V5_MatchesRfcTestVector()
    {
        var id = UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "www.example.com");
        id.ShouldBe(new Guid("2ed6657d-e927-568b-95e1-2665a8aea6a2"));
    }

    // Python's `uuid` standard library documentation test vector.
    [Fact]
    public void V5_MatchesPythonDocsVector()
    {
        var id = UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "python.org");
        id.ShouldBe(new Guid("886313e1-3b8a-5372-9b90-0c9aee199e5d"));
    }

    [Fact]
    public void V5_IsDeterministic()
    {
        var a = UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "same-name");
        var b = UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "same-name");
        a.ShouldBe(b);
    }

    [Fact]
    public void V5_DifferentNamespacesDiffer()
    {
        var dns = UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "test");
        var url = UuidGenerator.NewV5(UuidGenerator.Namespaces.Url, "test");
        dns.ShouldNotBe(url);
    }

    const long RfcTestVectorMs = 1_645_557_742_000;

    [Fact]
    public void V7_EmbedsTheTimestamp()
    {
        var id = UuidGenerator.NewV7(RfcTestVectorMs);
        Span<byte> rfc = stackalloc byte[16];
        id.TryWriteBytes(rfc, bigEndian: true, out _);
        var ms = ((long)rfc[0] << 40) | ((long)rfc[1] << 32) | ((long)rfc[2] << 24) |
                  ((long)rfc[3] << 16) | ((long)rfc[4] << 8) | rfc[5];
        ms.ShouldBe(RfcTestVectorMs);
    }

    [Fact]
    public void V7_HasVersionAndVariantBitsSet()
    {
        var id = UuidGenerator.NewV7(RfcTestVectorMs);
        Span<byte> rfc = stackalloc byte[16];
        id.TryWriteBytes(rfc, bigEndian: true, out _);
        (rfc[6] >> 4).ShouldBe((byte)7);
        (rfc[8] & 0xC0).ShouldBe((byte)0x80);
    }

    [Fact]
    public void V7_OverflowTimestampThrows()
    {
        Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.NewV7(0x0001_0000_0000_0000L));
    }

    [Fact]
    public void V7_SameMillisecondBatchIsMonotonicallyOrdered()
    {
        var ids = Enumerable.Range(0, 100).Select(_ => UuidGenerator.NewV7(RfcTestVectorMs)).ToArray();
        var sorted = ids.OrderBy(x => x).ToArray();
        sorted.ShouldBe(ids, ignoreOrder: false);
    }

    [Fact]
    public void V7_CurrentTimestampIsEmbedded()
    {
        var before = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        var id = UuidGenerator.NewV7();
        var after = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();

        Span<byte> rfc = stackalloc byte[16];
        id.TryWriteBytes(rfc, bigEndian: true, out _);
        var ms = ((long)rfc[0] << 40) | ((long)rfc[1] << 32) | ((long)rfc[2] << 24) |
                  ((long)rfc[3] << 16) | ((long)rfc[4] << 8) | rfc[5];

        ms.ShouldBeGreaterThanOrEqualTo(before);
        ms.ShouldBeLessThanOrEqualTo(after);
    }
}
