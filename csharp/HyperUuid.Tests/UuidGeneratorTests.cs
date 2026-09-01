using System.Data.SqlTypes;
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
    public void GetTimestamp_ReturnsNullForNonTimeBasedVersions()
    {
        UuidGenerator.GetTimestamp(UuidGenerator.NewV4()).ShouldBeNull();
        UuidGenerator.GetTimestamp(UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "test")).ShouldBeNull();
    }

    [Fact]
    public void GetTimestamp_MatchesV6Timestamp()
    {
        var id = UuidGenerator.NewV6(RfcTestVectorMs);
        UuidGenerator.GetTimestamp(id).ShouldBe(UuidGenerator.V6Timestamp(id));
    }

    [Fact]
    public void GetTimestamp_MatchesV7Timestamp()
    {
        var id = UuidGenerator.NewV7(RfcTestVectorMs);
        UuidGenerator.GetTimestamp(id).ShouldBe(UuidGenerator.V7Timestamp(id));
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

    [Fact]
    public void V7ToSqlOrder_RoundTripsThroughV7FromSqlOrder()
    {
        var id = UuidGenerator.NewV7(RfcTestVectorMs);
        var sqlOrdered = UuidGenerator.V7ToSqlOrder(id);
        sqlOrdered.ShouldNotBe(id);
        UuidGenerator.V7FromSqlOrder(sqlOrdered).ShouldBe(id);
    }

    [Fact]
    public void V7ToSqlOrder_PreservesVersionAndVariant()
    {
        var sqlOrdered = UuidGenerator.V7ToSqlOrder(UuidGenerator.NewV7(RfcTestVectorMs));
        var bytes = sqlOrdered.ToByteArray();
        (bytes[7] & 0xF0).ShouldBe(0x70);
        (bytes[8] & 0xC0).ShouldBe(0x80);
    }

    [Fact]
    public void V7ToSqlOrder_SortsByCreationOrderUnderRealSqlGuidComparison()
    {
        // The correctness oracle here is the real System.Data.SqlTypes.SqlGuid — the same
        // type T-SQL's own ORDER BY on a uniqueidentifier column matches — not a hand-rolled
        // stand-in for it, unlike the Rust core's own version of this test.
        var ids = new List<Guid>();
        for (long i = 0; i < 200; i++)
        {
            ids.Add(UuidGenerator.NewV7(RfcTestVectorMs + i));
        }
        // Same-millisecond run, so the counter (not just the timestamp) has to sort correctly too.
        for (var i = 0; i < 200; i++)
        {
            ids.Add(UuidGenerator.NewV7(RfcTestVectorMs + 1_000_000));
        }

        var sqlOrdered = ids.Select(UuidGenerator.V7ToSqlOrder).ToList();
        var sorted = sqlOrdered.OrderBy(g => new SqlGuid(g)).ToList();

        sorted.ShouldBe(sqlOrdered);
    }

    [Fact]
    public void V6ToSqlOrder_RoundTripsThroughV6FromSqlOrder()
    {
        var id = UuidGenerator.NewV6(RfcTestVectorMs);
        var sqlOrdered = UuidGenerator.V6ToSqlOrder(id);
        sqlOrdered.ShouldNotBe(id);
        UuidGenerator.V6FromSqlOrder(sqlOrdered).ShouldBe(id);
    }

    [Fact]
    public void V6ToSqlOrder_PreservesVersionAndVariant()
    {
        // Different offsets than v7's sql order — see V6ToSqlOrder's doc comment for why.
        var sqlOrdered = UuidGenerator.V6ToSqlOrder(UuidGenerator.NewV6(RfcTestVectorMs));
        var bytes = sqlOrdered.ToByteArray();
        (bytes[8] & 0xF0).ShouldBe(0x60);
        (bytes[6] & 0xC0).ShouldBe(0x80);
    }

    [Fact]
    public void V6ToSqlOrder_SortsByCreationOrderUnderRealSqlGuidComparisonForDistinctTimestamps()
    {
        // Unlike v7, v6 has no counter — two UUIDs at the same millisecond aren't guaranteed
        // to sort in creation order even in plain RFC order, so this only exercises strictly
        // increasing timestamps, where the timestamp alone determines order with no tie to break.
        var ids = new List<Guid>();
        for (long i = 0; i < 300; i++)
        {
            ids.Add(UuidGenerator.NewV6(RfcTestVectorMs + i));
        }

        var sqlOrdered = ids.Select(UuidGenerator.V6ToSqlOrder).ToList();
        var sorted = sqlOrdered.OrderBy(g => new SqlGuid(g)).ToList();

        sorted.ShouldBe(sqlOrdered);
    }

    // ---- Non-throwing construction path -------------------------------------------------

    [Fact]
    public void TryNewV4_SucceedsAndMatchesThrowingOverloadShape()
    {
        UuidGenerator.TryNewV4(out var id).ShouldBeTrue();
        id.ShouldNotBe(Guid.Empty);
        id.ToString()[14].ShouldBe('4');
    }

    [Fact]
    public void TryNewV7_SucceedsForAValidTimestamp()
    {
        UuidGenerator.TryNewV7(RfcTestVectorMs, out var id).ShouldBeTrue();
        UuidGenerator.V7UnixMillis(id).ShouldBe(RfcTestVectorMs);
    }

    [Fact]
    public void TryNewV6_SucceedsForAValidTimestamp()
    {
        UuidGenerator.TryNewV6(RfcTestVectorMs, out var id).ShouldBeTrue();
        UuidGenerator.V6UnixMillis(id).ShouldBe(RfcTestVectorMs);
    }

    [Fact]
    public void TryNewV7_ReturnsFalseWhereTheThrowingOverloadThrows()
    {
        // 2^48 ms doesn't fit v7's 48-bit unix_ts_ms field — the native layer reports rc 2,
        // which NewV7 turns into an exception and TryNewV7 reports as a plain false.
        const long outOfRange = 1L << 48;
        Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.NewV7(outOfRange));
        UuidGenerator.TryNewV7(outOfRange, out var id).ShouldBeFalse();
        id.ShouldBe(Guid.Empty);
    }

    [Fact]
    public void TryNewV7_NowOverloadSucceeds()
    {
        UuidGenerator.TryNewV7(out var id).ShouldBeTrue();
        id.ShouldNotBe(Guid.Empty);
    }

    // ---- Raw-byte SQL-order transforms --------------------------------------------------

    [Fact]
    public void V7ToSqlOrder_ByteOverloadAgreesWithTheGuidOverload()
    {
        // The equivalence that lets a byte-level oracle (Svartalfheim's SequentialGuidBytes)
        // validate this without modelling Guid's mixed-endian field layout: the Guid overload's
        // result, viewed as bytes, IS the byte overload's result.
        var id = UuidGenerator.NewV7(RfcTestVectorMs);

        Span<byte> rfc = stackalloc byte[16];
        id.TryWriteBytes(rfc, bigEndian: true, out _);
        UuidGenerator.V7ToSqlOrder(rfc);

        UuidGenerator.V7ToSqlOrder(id).ToByteArray().ShouldBe(rfc.ToArray());
    }

    [Fact]
    public void V6ToSqlOrder_ByteOverloadAgreesWithTheGuidOverload()
    {
        var id = UuidGenerator.NewV6(RfcTestVectorMs);

        Span<byte> rfc = stackalloc byte[16];
        id.TryWriteBytes(rfc, bigEndian: true, out _);
        UuidGenerator.V6ToSqlOrder(rfc);

        UuidGenerator.V6ToSqlOrder(id).ToByteArray().ShouldBe(rfc.ToArray());
    }

    [Fact]
    public void V7SqlOrder_ByteOverloadsRoundTrip()
    {
        var id = UuidGenerator.NewV7(RfcTestVectorMs);
        var rfc = new byte[16];
        id.TryWriteBytes(rfc, bigEndian: true, out _);
        var original = rfc.ToArray();

        UuidGenerator.V7ToSqlOrder(rfc);
        rfc.ShouldNotBe(original);
        UuidGenerator.V7FromSqlOrder(rfc);
        rfc.ShouldBe(original);
    }

    [Fact]
    public void V6SqlOrder_ByteOverloadsRoundTrip()
    {
        var id = UuidGenerator.NewV6(RfcTestVectorMs);
        var rfc = new byte[16];
        id.TryWriteBytes(rfc, bigEndian: true, out _);
        var original = rfc.ToArray();

        UuidGenerator.V6ToSqlOrder(rfc);
        rfc.ShouldNotBe(original);
        UuidGenerator.V6FromSqlOrder(rfc);
        rfc.ShouldBe(original);
    }

    [Fact]
    public void SqlOrder_ByteOverloadRejectsAWrongSizedBuffer()
    {
        Should.Throw<ArgumentException>(() => UuidGenerator.V7ToSqlOrder(new byte[15]));
        Should.Throw<ArgumentException>(() => UuidGenerator.V6FromSqlOrder(new byte[17]));
    }

    // ---- Raw-byte batch fill ------------------------------------------------------------

    [Fact]
    public void FillV7_ByteOverloadMatchesTheGuidOverloadElementForElement()
    {
        const int count = 64;
        var asGuids = new Guid[count];
        UuidGenerator.FillV7(asGuids, RfcTestVectorMs);

        var asBytes = new byte[count * 16];
        UuidGenerator.FillV7(asBytes, RfcTestVectorMs);

        // Not value-equal (each batch draws its own entropy), but structurally identical:
        // every 16-byte chunk must decode to a v7 carrying the same timestamp.
        for (var i = 0; i < count; i++)
        {
            var fromBytes = new Guid(asBytes.AsSpan(i * 16, 16), bigEndian: true);
            UuidGenerator.V7UnixMillis(fromBytes).ShouldBe(RfcTestVectorMs);
            UuidGenerator.V7UnixMillis(asGuids[i]).ShouldBe(RfcTestVectorMs);
            fromBytes.ToString()[14].ShouldBe('7');
        }
        asBytes.Length.ShouldBe(asGuids.Length * 16);
    }

    [Fact]
    public void FillV7_ByteOverloadRejectsANonMultipleOf16()
    {
        Should.Throw<ArgumentException>(() => UuidGenerator.FillV7(new byte[17], RfcTestVectorMs));
        UuidGenerator.TryFillV7(new byte[17], RfcTestVectorMs).ShouldBeFalse();
    }

    [Fact]
    public void TryFillV7_ReturnsFalseWhereFillV7Throws()
    {
        const long outOfRange = 1L << 48;
        var buf = new Guid[8];
        Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.FillV7(buf, outOfRange));
        UuidGenerator.TryFillV7(buf, outOfRange).ShouldBeFalse();
        UuidGenerator.TryFillV7(buf, RfcTestVectorMs).ShouldBeTrue();
    }
}
