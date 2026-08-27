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
    public void V6_EmbedsTheTimestamp()
    {
        var id = UuidGenerator.NewV6(RfcTestVectorMs);
        UuidGenerator.V6Timestamp(id).ShouldBe(DateTimeOffset.FromUnixTimeMilliseconds(RfcTestVectorMs));
    }

    [Fact]
    public void V6_HasVersionAndVariantBitsSet()
    {
        var id = UuidGenerator.NewV6(RfcTestVectorMs);
        Span<byte> rfc = stackalloc byte[16];
        id.TryWriteBytes(rfc, bigEndian: true, out _);
        (rfc[6] >> 4).ShouldBe((byte)6);
        (rfc[8] & 0xC0).ShouldBe((byte)0x80);
    }

    [Fact]
    public void V6_SetsTheNodeIdMulticastBit()
    {
        var id = UuidGenerator.NewV6(RfcTestVectorMs);
        Span<byte> rfc = stackalloc byte[16];
        id.TryWriteBytes(rfc, bigEndian: true, out _);
        (rfc[10] & 0x01).ShouldBe(0x01);
    }

    [Fact]
    public void V6_IsNonDeterministicWithinTheSameMillisecond()
    {
        var results = Enumerable.Range(0, 100).Select(_ => UuidGenerator.NewV6(RfcTestVectorMs)).ToHashSet();
        results.Count.ShouldBe(100);
    }

    [Fact]
    public void V6Timestamp_RoundTripsZero()
    {
        UuidGenerator.V6Timestamp(UuidGenerator.NewV6(0)).ShouldBe(DateTimeOffset.FromUnixTimeMilliseconds(0));
    }

    [Fact]
    public void V6Batch_ReturnsCountUuidsSharingTheTimestamp()
    {
        var ids = UuidGenerator.NewV6Batch(10, RfcTestVectorMs);
        ids.Length.ShouldBe(10);
        foreach (var id in ids)
        {
            UuidGenerator.V6Timestamp(id).ShouldBe(DateTimeOffset.FromUnixTimeMilliseconds(RfcTestVectorMs));
        }
    }

    [Fact]
    public void V6Batch_ProducesPairwiseDistinctUuids()
    {
        var ids = UuidGenerator.NewV6Batch(100, RfcTestVectorMs);
        ids.ToHashSet().Count.ShouldBe(100);
    }

    [Fact]
    public void V6Batch_CountZeroReturnsEmptyArray()
    {
        UuidGenerator.NewV6Batch(0, RfcTestVectorMs).ShouldBeEmpty();
    }

    [Fact]
    public void V6Batch_OverflowTimestampThrows()
    {
        Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.NewV6Batch(1, long.MaxValue));
    }

    [Fact]
    public void Nil_IsAllZeroBytes()
    {
        UuidGenerator.Nil.ShouldBe(Guid.Empty);
        UuidGenerator.Nil.ToString().ShouldBe("00000000-0000-0000-0000-000000000000");
    }

    [Fact]
    public void Max_IsAllOneBytes()
    {
        UuidGenerator.Max.ToString().ShouldBe("ffffffff-ffff-ffff-ffff-ffffffffffff");
    }

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

    [Fact]
    public void V7Timestamp_RecoversTheExactMillisecond()
    {
        var id = UuidGenerator.NewV7(RfcTestVectorMs);
        UuidGenerator.V7Timestamp(id).ShouldBe(DateTimeOffset.FromUnixTimeMilliseconds(RfcTestVectorMs));
    }

    [Fact]
    public void V7Timestamp_RoundTripsZeroAndALargeTimestamp()
    {
        UuidGenerator.V7Timestamp(UuidGenerator.NewV7(0)).ShouldBe(DateTimeOffset.FromUnixTimeMilliseconds(0));

        // Largest ms value DateTimeOffset (year <= 9999) can represent, not the RFC's own
        // 48-bit max (valid to year 10889) — see test below for that boundary.
        var largeMs = new DateTimeOffset(9999, 1, 1, 0, 0, 0, TimeSpan.Zero).ToUnixTimeMilliseconds();
        UuidGenerator.V7Timestamp(UuidGenerator.NewV7(largeMs)).ToUnixTimeMilliseconds().ShouldBe(largeMs);
    }

    [Fact]
    public void V7Timestamp_ThrowsPastDateTimeOffsetYearRange()
    {
        // A legitimate RFC 9562 v7 UUID can embed a timestamp DateTimeOffset can't hold.
        var id = UuidGenerator.NewV7(0x0000_FFFF_FFFF_FFFFL);
        Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.V7Timestamp(id));
    }

    [Fact]
    public void V7Batch_ReturnsCountUuidsSortedAndSharingTheTimestamp()
    {
        var ids = UuidGenerator.NewV7Batch(1000, RfcTestVectorMs);
        ids.Length.ShouldBe(1000);
        var sorted = ids.OrderBy(x => x).ToArray();
        sorted.ShouldBe(ids, ignoreOrder: false);
        foreach (var id in ids)
        {
            UuidGenerator.V7Timestamp(id).ShouldBe(DateTimeOffset.FromUnixTimeMilliseconds(RfcTestVectorMs));
        }
    }

    [Fact]
    public void V7Batch_ContinuesTheSameCounterSequenceAsIndividualCalls()
    {
        var before = UuidGenerator.NewV7(RfcTestVectorMs);
        var batch = UuidGenerator.NewV7Batch(10, RfcTestVectorMs);
        var after = UuidGenerator.NewV7(RfcTestVectorMs);

        var ids = new[] { before }.Concat(batch).Append(after).ToArray();
        var sorted = ids.OrderBy(x => x).ToArray();
        sorted.ShouldBe(ids, ignoreOrder: false);
    }

    [Fact]
    public void V7Batch_CountZeroReturnsEmptyArray()
    {
        UuidGenerator.NewV7Batch(0, RfcTestVectorMs).ShouldBeEmpty();
    }

    [Fact]
    public void V7Batch_OverflowTimestampThrows()
    {
        Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.NewV7Batch(1, 0x0001_0000_0000_0000L));
    }

    [Fact]
    public void V7Batch_AboveStackThresholdUsesArrayPool()
    {
        // BatchStackThresholdBytes is 256 (16 items); this exercises the ArrayPool fallback path.
        var ids = UuidGenerator.NewV7Batch(50, RfcTestVectorMs);
        ids.Length.ShouldBe(50);
        ids.ToHashSet().Count.ShouldBe(50);
    }
}
