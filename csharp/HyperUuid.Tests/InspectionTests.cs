using Shouldly;

namespace HyperUuid.Tests;

/// <summary>
/// Version/variant inspection and the layout-aware doors against values minted at run time,
/// by this library and by the BCL; <see cref="CorpusTests"/> pins the fixed vectors.
/// </summary>
public sealed class InspectionTests
{
	const long Ms = 1_645_557_742_000L;

	// RFC 9562 Appendix A.5's version 6, minted elsewhere: a version, but no timestamp here.
	static readonly Guid RfcV6 = new("1ec9414c-232a-6b00-b3c8-9f6bdeced846");

	[Fact]
	void Bcl_and_library_values_report_their_version_and_the_rfc_variant()
	{
		foreach (var (id, version) in new[]
		{
			(Guid.CreateVersion7(), 7), (Guid.NewGuid(), 4), (UuidGenerator.NewV4(), 4),
			(UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "x"), 5),
			(RfcV6, 6), (UuidGenerator.NewV7(Ms), 7),
		})
		{
			UuidGenerator.Version(id).ShouldBe(version);
			UuidGenerator.Version(id).ShouldBe(id.Version); // the BCL agrees
			UuidGenerator.Variant(id).ShouldBe(UuidVariant.Rfc9562);
			UuidGenerator.IsRfc(id, version).ShouldBeTrue();
			UuidGenerator.IsRfc(id, version == 7 ? 6 : 7).ShouldBeFalse();
		}
	}

	[Fact]
	void Nil_and_max_are_classified_as_the_rfc_says()
	{
		UuidGenerator.Version(UuidGenerator.Nil).ShouldBe(0);
		UuidGenerator.Variant(UuidGenerator.Nil).ShouldBe(UuidVariant.Ncs);
		UuidGenerator.Version(UuidGenerator.Max).ShouldBe(15);
		UuidGenerator.Variant(UuidGenerator.Max).ShouldBe(UuidVariant.Future);
		UuidGenerator.IsRfc(UuidGenerator.Nil, 0).ShouldBeFalse();
		UuidGenerator.IsRfc(UuidGenerator.Max, 15).ShouldBeFalse();
	}

	[Fact]
	void A_version_outside_the_nibble_never_matches()
	{
		var id = UuidGenerator.NewV7(Ms);
		UuidGenerator.IsRfc(id, 7 + 256).ShouldBeFalse();
		UuidGenerator.IsRfc(id, -1).ShouldBeFalse();
	}

	// The conversion writes a version 8 whose timestamp, counter and random bits are all
	// around the 8 and the variant. Enough draws that a misread would surface.
	[Fact]
	void Sql_ordered_v7_is_validated_and_read_in_place()
	{
		for (var i = 0; i < 2048; i++)
		{
			var seven = UuidGenerator.V7ToSqlOrder(UuidGenerator.NewV7(Ms));

			UuidGenerator.Version(seven, UuidLayout.SqlServer).ShouldBe(7);
			UuidGenerator.IsRfc(seven, 7, UuidLayout.SqlServer).ShouldBeTrue();
			UuidGenerator.IsRfc(seven, 6, UuidLayout.SqlServer).ShouldBeFalse();
			UuidGenerator.IsRfc(seven, 8, UuidLayout.SqlServer).ShouldBeFalse();

			UuidGenerator.V7UnixMillis(seven, UuidLayout.SqlServer).ShouldBe(Ms);
			UuidGenerator.GetTimestamp(seven, UuidLayout.SqlServer).ShouldBe(DateTimeOffset.FromUnixTimeMilliseconds(Ms));
			// Read straight from SQL order matches permuting back first.
			UuidGenerator.V7UnixMillis(seven, UuidLayout.SqlServer)
				.ShouldBe(UuidGenerator.V7UnixMillis(UuidGenerator.V7FromSqlOrder(seven)));
			// The raw form takes the bytes SQL Server stores, which is Guid.ToByteArray().
			UuidGenerator.Version(seven.ToByteArray(), UuidLayout.SqlServer).ShouldBe(7);
		}
	}

	// What the overloads without a layout make of a SQL-ordered Guid. The conversion writes an
	// RFC 9562 version 8, its 8 and variant exactly where a Guid (and SQL Server's text form)
	// holds them, so to every reader of the Guid it is a version 8, which has no timestamp. Only
	// UuidLayout.SqlServer recovers the version 7 it came from, and its timestamp.
	[Fact]
	void A_sql_ordered_value_is_a_version_8_without_a_timestamp()
	{
		for (var i = 0; i < 2048; i++)
		{
			var sql = UuidGenerator.V7ToSqlOrder(UuidGenerator.NewV7(Ms));
			UuidGenerator.Version(sql).ShouldBe(8);
			sql.Version.ShouldBe(8); // the BCL sees it too
			sql.ToString()[14].ShouldBe('8'); // and so does the text SQL Server shows
			UuidGenerator.Variant(sql).ShouldBe(UuidVariant.Rfc9562);
			UuidGenerator.IsRfc(sql, 8).ShouldBeTrue();
			UuidGenerator.IsRfc(sql, 7).ShouldBeFalse();
			UuidGenerator.GetTimestamp(sql).ShouldBeNull();
			UuidGenerator.Version(sql, UuidLayout.SqlServer).ShouldBe(7);

			// The raw wire bytes hold the 8 at octet 7, the Guid position; an RFC read of
			// raw bytes looks at octet 6, which holds key bits.
			var raw = sql.ToByteArray();
			UuidGenerator.Version(raw).ShouldBe(raw[6] >> 4);
			UuidGenerator.IsRfc(raw, 8).ShouldBe(raw[6] >> 4 == 8);
			// Read as RFC order, a SQL-ordered v7's "timestamp" is key bits, not its timestamp.
			UuidGenerator.V7UnixMillis(sql).ShouldNotBe(Ms);
		}
	}

	// The layout is the caller's to know: an RFC-ordered version 8 Guid minted elsewhere carries
	// the 8 and the variant where SQL order puts them, so SqlServer reads it as a converted value.
	// An RFC-ordered Guid of version 1-7 never does: its own version nibble sits there.
	[Fact]
	void Only_an_rfc_ordered_version_8_guid_can_pass_for_a_sql_ordered_one()
	{
		Span<byte> v8 = stackalloc byte[16];
		for (var i = 0; i < 256; i++)
		{
			Random.Shared.NextBytes(v8);
			v8[6] = (byte)(0x80 | (v8[6] & 0x0F));
			v8[8] = (byte)(0x80 | (v8[8] & 0x3F));
			var id = new Guid(v8, bigEndian: true);
			UuidGenerator.Version(id, UuidLayout.SqlServer).ShouldBe(7);

			foreach (var other in new[] { UuidGenerator.NewV4(), RfcV6, UuidGenerator.NewV7(Ms), Guid.CreateVersion7() })
				UuidGenerator.Version(other, UuidLayout.SqlServer).ShouldBe(0);
		}
	}

	[Fact]
	void Non_sql_values_have_no_sql_version()
	{
		foreach (var id in new[] { Guid.NewGuid(), UuidGenerator.Nil, UuidGenerator.Max, UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "x") })
		{
			UuidGenerator.Version(id, UuidLayout.SqlServer).ShouldBe(0);
			UuidGenerator.GetTimestamp(id, UuidLayout.SqlServer).ShouldBeNull();
		}
	}

	[Fact]
	void An_unspecified_or_undefined_layout_throws()
	{
		var id = UuidGenerator.NewV7(Ms);
		foreach (var layout in new[] { UuidLayout.Unspecified, (UuidLayout)3 })
		{
			Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.Version(id, layout)).ParamName.ShouldBe("layout");
			Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.IsRfc(id, 7, layout)).ParamName.ShouldBe("layout");
			Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.V7UnixMillis(id, layout)).ParamName.ShouldBe("layout");
			Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.V7Timestamp(id, layout)).ParamName.ShouldBe("layout");
			Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.GetTimestamp(id, layout)).ParamName.ShouldBe("layout");
			Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.Version(new byte[16], layout)).ParamName.ShouldBe("layout");
			Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.IsRfc(new byte[16], 7, layout)).ParamName.ShouldBe("layout");
		}
	}

	[Fact]
	void The_raw_byte_forms_require_exactly_16_bytes()
	{
		Should.Throw<ArgumentException>(() => UuidGenerator.Version(new byte[15])).ParamName.ShouldBe("uuid");
		Should.Throw<ArgumentException>(() => UuidGenerator.Variant(new byte[17])).ParamName.ShouldBe("uuid");
		Should.Throw<ArgumentException>(() => UuidGenerator.IsRfc(new byte[0], 7)).ParamName.ShouldBe("uuid");
	}

	// The v7 batch limit: the counter space, enforced before any scratch buffer is rented. The
	// span past the limit is never touched, so it can claim a length nothing allocated.
	[Fact]
	void A_v7_batch_past_the_counter_space_is_refused_on_every_door()
	{
		const int tooMany = UuidGenerator.MaxV7Batch + 1;
		var one = Guid.Empty;
		var guids = System.Runtime.InteropServices.MemoryMarshal.CreateSpan(ref one, tooMany);
		var thrown = Should.Throw<ArgumentOutOfRangeException>(() =>
		{
			var one = Guid.Empty;
			UuidGenerator.FillV7(System.Runtime.InteropServices.MemoryMarshal.CreateSpan(ref one, tooMany), Ms);
		});
		thrown.ParamName.ShouldBe("destination");
		thrown.Message.ShouldContain(UuidGenerator.MaxV7Batch.ToString(System.Globalization.CultureInfo.InvariantCulture));
		UuidGenerator.TryFillV7(guids, Ms).ShouldBeFalse();
		one.ShouldBe(Guid.Empty);

		Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.NewV7Batch(tooMany, Ms)).ParamName.ShouldBe("count");
		Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.NewV7Batch(tooMany)).ParamName.ShouldBe("count");

		// The byte door hands the core the count; the core refuses it (code 4) before writing.
		var bytes = new byte[(long)tooMany * 16];
		Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.FillV7(bytes, Ms)).ParamName.ShouldBe("destination");
		UuidGenerator.TryFillV7(bytes, Ms).ShouldBeFalse();
		bytes.AsSpan(0, 64).ToArray().ShouldAllBe(b => b == 0);
	}

	// Exactly the counter space: 1 GiB of bytes, in strictly increasing order end to end, and
	// stamped with the supplied timestamp exactly: Ms is far behind the stream, so the batch is minted
	// out of stream and never splits.
	[Fact]
	void A_v7_batch_of_exactly_the_counter_space_is_strictly_increasing()
	{
		var bytes = new byte[(long)UuidGenerator.MaxV7Batch * 16];
		UuidGenerator.FillV7(bytes, Ms);
		var previous = bytes.AsSpan(0, 16);
		for (var i = 1; i < UuidGenerator.MaxV7Batch; i++)
		{
			var current = bytes.AsSpan(i * 16, 16);
			current.SequenceCompareTo(previous).ShouldBeGreaterThan(0);
			previous = current;
		}
		var last = new Guid(previous, bigEndian: true);
		UuidGenerator.V7UnixMillis(last).ShouldBe(Ms);
	}
}
