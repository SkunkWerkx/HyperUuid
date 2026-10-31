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

	// An empty name is a legitimate v5 input (the namespace hashed with zero name bytes), and
	// the one input that crosses the FFI as a null pointer with length 0. Pinned to
	// uuid.uuid5(uuid.NAMESPACE_DNS, "") on every overload, since each reaches the native call
	// by a different route.
	[Fact]
	public void V5_EmptyNameIsValidOnEveryOverload()
	{
		var expected = new Guid("4ebd0208-8328-5d69-8c44-ec50939c0967");
		UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "").ShouldBe(expected);
		UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, ReadOnlySpan<char>.Empty).ShouldBe(expected);
		UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, ReadOnlySpan<byte>.Empty).ShouldBe(expected);
		UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, Array.Empty<byte>()).ShouldBe(expected);
	}

	[Fact]
	public void V5_NullNameThrowsArgumentNull()
	{
		var thrown = Should.Throw<ArgumentNullException>(() => UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, (string)null!));
		thrown.ParamName.ShouldBe("name");
	}

	[Fact]
	public void V5_ByteOverloadMatchesRfcTestVector()
	{
		UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "www.example.com"u8)
			.ShouldBe(new Guid("2ed6657d-e927-568b-95e1-2665a8aea6a2"));
	}

	// The byte overload hashes what it is given — these four bytes are not valid UTF-8, and no
	// text overload could produce them. Expected value from SHA-1 over namespace + bytes.
	[Fact]
	public void V5_ByteOverloadHashesBytesThatAreNotText()
	{
		UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, [0xFF, 0xFE, 0x00, 0x80])
			.ShouldBe(new Guid("3d0e33b8-b10f-561a-808e-37c1addebe0b"));
	}

	[Fact]
	public void V5_CharSpanOverloadMatchesTheStringOverload()
	{
		// A slice of a larger buffer — the case the overload exists for: no string to build.
		var line = "host=www.example.com;port=443".AsSpan();
		var name = line.Slice(5, 15);
		UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, name)
			.ShouldBe(new Guid("2ed6657d-e927-568b-95e1-2665a8aea6a2"));
		UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, name)
			.ShouldBe(UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, name.ToString()));
		UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "www.example.com".ToCharArray())
			.ShouldBe(new Guid("2ed6657d-e927-568b-95e1-2665a8aea6a2"));
	}

	// The text overloads encode into a 256-byte stack buffer sized by the encoder's worst case
	// (3 bytes per char, plus 3), so 84 chars is the last name on the stack and 85 the first
	// from the pool. Both sides of that edge, and well past it, pinned to independently
	// computed vectors: the pooled path must hash the same bytes the stack path does.
	[Theory]
	[InlineData(84, "16eb10f5-0101-57cc-a4d8-f65d128b656f")]
	[InlineData(85, "3cb7eff9-b47a-51bb-9b0a-fc8402d5f907")]
	[InlineData(300, "39aaf90c-2a5a-5781-93dc-08baea928201")]
	public void V5_NamesAcrossTheStackBufferThresholdMatch(int length, string expected)
	{
		var name = new string('a', length);
		UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, name).ShouldBe(new Guid(expected));
		UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, name.AsSpan()).ShouldBe(new Guid(expected));
		UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, System.Text.Encoding.UTF8.GetBytes(name))
			.ShouldBe(new Guid(expected));
	}

	[Fact]
	public void V5_MultiByteNamePastTheStackBufferMatches()
	{
		// 100 chars, 300 UTF-8 bytes: past the threshold by encoded length as well as by the
		// worst-case estimate.
		var name = new string('€', 100);
		var expected = new Guid("86724f88-7cba-5f66-a02a-23dffedea996");
		UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, name).ShouldBe(expected);
		UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, System.Text.Encoding.UTF8.GetBytes(name)).ShouldBe(expected);
	}

	[Fact]
	public void V5_LoneSurrogateHashesAsTheReplacementCharacter()
	{
		UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "a\uD800b")
			.ShouldBe(new Guid("7ddcff48-f6da-51a8-84aa-6fabda0e46f1"));
		UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "a\uD800b")
			.ShouldBe(UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "a�b"));
	}

	const long RfcTestVectorMs = 1_645_557_742_000;

	// The present, for ordering tests: the v7 stream is process-wide and sits at the present, so
	// a fixed old timestamp is minted outside it and orders against nothing.
	static long Now() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();

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
		var now = Now();
		var ids = Enumerable.Range(0, 100).Select(_ => UuidGenerator.NewV7(now)).ToArray();
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

		// The stream may stamp a later ms that an earlier test moved it to.
		ms.ShouldBeGreaterThanOrEqualTo(before);
		ms.ShouldBeLessThanOrEqualTo(after + 1_000);
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

		// Inside DateTimeOffset's last year; the generation cap is its last millisecond (the
		// test below pins that boundary).
		var largeMs = new DateTimeOffset(9999, 1, 1, 0, 0, 0, TimeSpan.Zero).ToUnixTimeMilliseconds();
		UuidGenerator.V7Timestamp(UuidGenerator.NewV7(largeMs)).ToUnixTimeMilliseconds().ShouldBe(largeMs);
	}

	[Fact]
	public void V7Timestamps_StopAtTheLastMillisecondOf9999InBothDirections()
	{
		// The cap is DateTimeOffset.MaxValue's millisecond, so the whole accepted range has a
		// DateTimeOffset. One millisecond more is the out-of-range exception; a version 7
		// another generator dated past the cap has no timestamp at GetTimestamp, as a version 6
		// has none, while V7UnixMillis trusts the caller and reads the 48-bit field as it is, and
		// V7Timestamp, which builds a DateTimeOffset, throws for it as documented.
		DateTimeOffset.MaxValue.ToUnixTimeMilliseconds().ShouldBe(253_402_300_799_999L);
		var last = DateTimeOffset.FromUnixTimeMilliseconds(253_402_300_799_999L);
		UuidGenerator.GetTimestamp(UuidGenerator.NewV7(DateTimeOffset.MaxValue)).ShouldBe(last);
		UuidGenerator.GetTimestamp(UuidGenerator.NewV7(253_402_300_799_999L)).ShouldBe(last);
		var thrown = Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.NewV7(253_402_300_800_000L));
		thrown.Message.ShouldContain("at most 253402300799999 (9999-12-31T23:59:59.999Z)");
		UuidGenerator.TryNewV7(253_402_300_800_000L, out _).ShouldBeFalse();

		var past = Guid.Parse("ffffffff-ffff-7000-8000-010203040506");
		UuidGenerator.IsRfc(past, 7).ShouldBeTrue();
		UuidGenerator.GetTimestamp(past).ShouldBeNull();
		past.Timestamp.ShouldBeNull();
		UuidGenerator.V7UnixMillis(past).ShouldBe(0x0000_FFFF_FFFF_FFFFL);
		Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.V7Timestamp(past));
	}

	[Fact]
	public void GetTimestamp_ReturnsNullForNonTimeBasedVersions()
	{
		UuidGenerator.GetTimestamp(UuidGenerator.NewV4()).ShouldBeNull();
		UuidGenerator.GetTimestamp(UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "test")).ShouldBeNull();
	}

	// RFC 9562 Appendix A.5's version 6, minted elsewhere. It has a timestamp field, but only a
	// version 7's timestamp is read, in either layout.
	static readonly Guid RfcV6 = new("1ec9414c-232a-6b00-b3c8-9f6bdeced846");

	[Fact]
	public void GetTimestamp_ReturnsNullForAVersion6()
	{
		UuidGenerator.Version(RfcV6).ShouldBe(6);
		UuidGenerator.IsRfc(RfcV6, 6).ShouldBeTrue();
		UuidGenerator.GetTimestamp(RfcV6).ShouldBeNull();
		UuidGenerator.GetTimestamp(RfcV6, UuidLayout.SqlServer).ShouldBeNull();
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
		var now = Now();
		var before = UuidGenerator.NewV7(now);
		var batch = UuidGenerator.NewV7Batch(10, now);
		var after = UuidGenerator.NewV7(now);

		var ids = new[] { before }.Concat(batch).Append(after).ToArray();
		var sorted = ids.OrderBy(x => x).ToArray();
		sorted.ShouldBe(ids, ignoreOrder: false);
	}

	[Fact]
	public async Task FillV7_SurvivesAnotherThreadWritingTheSameBuffer()
	{
		// A race on the buffer's contents is the caller's to have, and must cost them the bytes,
		// not the process. Windows' ProcessPrng fail-fasts the process when another thread writes
		// the memory it is filling, so the core's batch fill does not hand it the caller's memory.
		var buffer = new byte[16 * 100_000];
		using var done = new ManualResetEventSlim();
		var writers = Enumerable.Range(0, 2).Select(_ => Task.Run(() =>
		{
			while (!done.IsSet)
			{
				Array.Clear(buffer);
			}
		}, TestContext.Current.CancellationToken)).ToArray();
		for (var i = 0; i < 500; i++)
		{
			UuidGenerator.FillV7(buffer.AsSpan(), RfcTestVectorMs);
		}
		done.Set();
		await Task.WhenAll(writers);
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
	public void V7ToSqlOrder_WritesAVersion8()
	{
		var sqlOrdered = UuidGenerator.V7ToSqlOrder(UuidGenerator.NewV7(RfcTestVectorMs));
		var bytes = sqlOrdered.ToByteArray();
		(bytes[7] & 0xF0).ShouldBe(0x80);
		(bytes[8] & 0xC0).ShouldBe(0x80);
		sqlOrdered.Version.ShouldBe(8);
	}

	[Fact]
	public void V7ToSqlOrder_SortsByCreationOrderUnderRealSqlGuidComparison()
	{
		// The correctness oracle here is the real System.Data.SqlTypes.SqlGuid — the same
		// type T-SQL's own ORDER BY on a uniqueidentifier column matches — not a hand-rolled
		// stand-in for it, unlike the Rust core's own version of this test.
		// Current-time timestamps: only calls near the stream's millisecond are ordered, and a
		// fixed 2022 timestamp is minted outside it. No offset exceeds now + 500 ms (see TestStream).
		var now = Now();
		var ids = new List<Guid>();
		for (long i = 0; i < 200; i++)
		{
			ids.Add(UuidGenerator.NewV7(now + i));
		}
		// Same-millisecond run, so the counter (not just the timestamp) has to sort correctly too.
		for (var i = 0; i < 200; i++)
		{
			ids.Add(UuidGenerator.NewV7(now + 500));
		}

		var sqlOrdered = ids.Select(UuidGenerator.V7ToSqlOrder).ToList();
		var sorted = sqlOrdered.OrderBy(g => new SqlGuid(g)).ToList();

		sorted.ShouldBe(sqlOrdered);
	}

	// ---- SQL-order refusals -------------------------------------------------------------

	const string WrongVersion = "the value is not a UUID of the version this conversion takes";

	// Exactly TException, not a subclass: ArgumentOutOfRangeException is an ArgumentException.
	static TException Refused<TException>(Action convert, string message) where TException : Exception
	{
		var thrown = Should.Throw<Exception>(convert);
		thrown.GetType().ShouldBe(typeof(TException));
		thrown.Message.ShouldStartWith(message);
		((ArgumentException)thrown).ParamName.ShouldBe("uuid");
		return (TException)thrown;
	}

	// Guid form, then the in-place byte form, which must leave the buffer as it was.
	static void RefusedEverywhere<TException>(Func<Guid, Guid> convert, SpanAction byteConvert, Guid value, byte[] bytes, string message)
		where TException : Exception
	{
		Refused<TException>(() => convert(value), message);
		var copy = bytes.ToArray();
		Refused<TException>(() => byteConvert(copy), message);
		copy.ShouldBe(bytes);
	}

	delegate void SpanAction(Span<byte> uuid);

	static byte[] RfcBytes(Guid id) { var b = new byte[16]; id.TryWriteBytes(b, bigEndian: true, out _); return b; }

	[Fact]
	public void V7ToSqlOrder_RefusesAnythingButAVersion7()
	{
		foreach (var id in new[] { UuidGenerator.NewV4(), RfcV6, UuidGenerator.Nil, UuidGenerator.Max,
			UuidGenerator.V7ToSqlOrder(UuidGenerator.NewV7(RfcTestVectorMs)) })
		{
			RefusedEverywhere<ArgumentException>(UuidGenerator.V7ToSqlOrder, UuidGenerator.V7ToSqlOrder, id, RfcBytes(id), WrongVersion);
		}
	}

	[Fact]
	public void V7ToSqlOrder_ConvertsEveryTimestamp()
	{
		// The layout holds all 122 of a v7's free bits, so every timestamp NewV7 takes converts,
		// the generation cap included.
		foreach (var ms in new[] { 0L, (1L << 47) - 1, 1L << 47, 253_402_300_799_999L })
		{
			var id = UuidGenerator.NewV7(ms);
			var sql = UuidGenerator.V7ToSqlOrder(id);
			UuidGenerator.V7FromSqlOrder(sql).ShouldBe(id);
			UuidGenerator.V7UnixMillis(sql, UuidLayout.SqlServer).ShouldBe(ms);
		}
	}

	[Fact]
	public void FromSqlOrder_RefusesAnythingButASqlOrderedValue()
	{
		// RFC 9562 Appendix A.6's v7, fixed: its raw RFC bytes put rand_a's 0xc3 at octet 7.
		// A freshly minted v7's raw bytes would carry a random octet 7, whose top nibble is the
		// SQL layout's 8 one time in 16, and those bytes are then accepted in the byte form.
		var v7 = new Guid("017f22e2-79b0-7cc3-98c4-dc0c0c07398f");
		// 0.8.0 wrote this layout with a 7 where the 8 now is; such a value is refused.
		var old = UuidGenerator.V7ToSqlOrder(v7).ToByteArray();
		old[7] = (byte)(0x70 | (old[7] & 0x0F));
		// A SQL-ordered Guid's bytes are its ToByteArray(); an RFC-ordered one's are big-endian.
		foreach (var (id, bytes) in new[] { (v7, RfcBytes(v7)), (RfcV6, RfcBytes(RfcV6)), (new Guid(old), old), (UuidGenerator.Nil, new byte[16]) })
			RefusedEverywhere<ArgumentException>(UuidGenerator.V7FromSqlOrder, UuidGenerator.V7FromSqlOrder, id, bytes, WrongVersion);
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
	public void TryNewV7_ReturnsFalseWhereTheThrowingOverloadThrows()
	{
		// One past the generation cap — the native layer reports rc 2, which NewV7 turns into
		// an exception and TryNewV7 reports as a plain false.
		const long outOfRange = 253_402_300_800_000L;
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
		// The equivalence that lets a byte-level oracle validate this without modelling Guid's
		// mixed-endian field layout: the Guid overload's result, viewed as bytes, IS the byte
		// overload's result.
		var id = UuidGenerator.NewV7(RfcTestVectorMs);

		Span<byte> rfc = stackalloc byte[16];
		id.TryWriteBytes(rfc, bigEndian: true, out _);
		UuidGenerator.V7ToSqlOrder(rfc);

		UuidGenerator.V7ToSqlOrder(id).ToByteArray().ShouldBe(rfc.ToArray());
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
	public void SqlOrder_ByteOverloadRejectsAWrongSizedBuffer()
	{
		Should.Throw<ArgumentException>(() => UuidGenerator.V7ToSqlOrder(new byte[15]));
		Should.Throw<ArgumentException>(() => UuidGenerator.V7FromSqlOrder(new byte[17]));
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
		const long outOfRange = 253_402_300_800_000L;
		var buf = new Guid[8];
		Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.FillV7(buf, outOfRange));
		UuidGenerator.TryFillV7(buf, outOfRange).ShouldBeFalse();
		// "Left untouched" is part of the Guid overload's contract: it fills through a scratch
		// buffer, so a failed call never writes a partial batch.
		buf.ShouldAllBe(id => id == Guid.Empty);
		UuidGenerator.TryFillV7(buf, RfcTestVectorMs).ShouldBeTrue();
		buf.ShouldAllBe(id => id != Guid.Empty);

		var bytes = new byte[32];
		Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.FillV7(bytes, outOfRange));
		UuidGenerator.TryFillV7(bytes, outOfRange).ShouldBeFalse();
		UuidGenerator.TryFillV7(bytes, RfcTestVectorMs).ShouldBeTrue();
	}

	[Fact]
	public void FillV7_NowOverloadEmbedsTheCurrentTimestamp()
	{
		var buf = new Guid[4];
		var before = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
		UuidGenerator.FillV7(buf);
		var after = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();

		foreach (var id in buf)
			// The stream may stamp a later ms that an earlier test moved it to.
			UuidGenerator.V7UnixMillis(id).ShouldBeInRange(before, after + 1_000);
	}

	[Theory]
	[InlineData(1)]
	[InlineData(16)] // exactly the 256-byte stack scratch buffer
	[InlineData(17)] // first size that rents from the pool
	[InlineData(200)]
	public void FillV7_FillsEveryElementWithAnIncreasingV7SharingTheTimestamp(int count)
	{
		var buf = new Guid[count];
		UuidGenerator.FillV7(buf, RfcTestVectorMs);

		foreach (var id in buf)
		{
			id.ToString()[14].ShouldBe('7');
			// Out of stream (the vector is years behind it), so the timestamp is carried exactly.
			UuidGenerator.V7UnixMillis(id).ShouldBe(RfcTestVectorMs);
		}
		buf.OrderBy(x => x).ShouldBe(buf, ignoreOrder: false);
		buf.ToHashSet().Count.ShouldBe(count);
	}

	[Fact]
	public void FillV7_EmptyDestinationIsANoOp()
	{
		UuidGenerator.FillV7(Span<Guid>.Empty, RfcTestVectorMs);
		UuidGenerator.FillV7(Span<byte>.Empty, RfcTestVectorMs);
		UuidGenerator.TryFillV7(Span<Guid>.Empty, RfcTestVectorMs).ShouldBeTrue();
		UuidGenerator.TryFillV7(Span<byte>.Empty, RfcTestVectorMs).ShouldBeTrue();
	}

	// ---- Caller-error edges -------------------------------------------------------------

	[Fact]
	public void Batch_NegativeCountThrowsArgumentOutOfRange()
	{
		Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.NewV7Batch(-1, RfcTestVectorMs)).ParamName.ShouldBe("count");
		Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.NewV7Batch(-1));
	}

	[Fact]
	public void V7_NegativeTimestampIsRejectedNotEncoded()
	{
		Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.NewV7(-1)).ParamName.ShouldBe("unixMilliseconds");
		Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.NewV7(DateTimeOffset.UnixEpoch.AddMilliseconds(-1)));
		UuidGenerator.TryNewV7(-1, out var id).ShouldBeFalse();
		id.ShouldBe(Guid.Empty);
	}

	[Fact]
	public void DateTimeOffsetOverloads_EmbedTheGivenInstant()
	{
		var instant = DateTimeOffset.FromUnixTimeMilliseconds(RfcTestVectorMs);
		UuidGenerator.V7Timestamp(UuidGenerator.NewV7(instant)).ShouldBe(instant);
		// An offset is the same instant, not a different one.
		UuidGenerator.V7Timestamp(UuidGenerator.NewV7(instant.ToOffset(TimeSpan.FromHours(5)))).ShouldBe(instant);
	}

	// ---- Native library probe -----------------------------------------------------------

	[Fact]
	public void NativeLibrary_IsProbedOnceAndReportsItsVersion()
	{
		UuidGenerator.IsAvailable.ShouldBeTrue();
		UuidGenerator.NativeVersion.ShouldNotBeNull();
		// The binding and the core share one version — the probe exists to catch the mismatch.
		var binding = typeof(UuidGenerator).Assembly.GetName().Version!;
		UuidGenerator.NativeVersion.ShouldBe(new Version(binding.Major, binding.Minor, binding.Build));
		// Cached: the same instance every time, not a fresh native call.
		UuidGenerator.NativeVersion.ShouldBeSameAs(UuidGenerator.NativeVersion);
	}
}
