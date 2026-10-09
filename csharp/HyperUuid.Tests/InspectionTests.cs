using Shouldly;

namespace HyperUuid.Tests;

/// <summary>
/// Version/variant inspection and the layout-aware doors against values minted at run time,
/// by this library and by the BCL; <see cref="CorpusTests"/> pins the fixed vectors.
/// </summary>
public sealed class InspectionTests
{
	const long Ms = 1_645_557_742_000L;

	[Fact]
	void Bcl_and_library_values_report_their_version_and_the_rfc_variant()
	{
		foreach (var (id, version) in new[]
		{
			(Guid.CreateVersion7(), 7), (Guid.NewGuid(), 4), (UuidGenerator.NewV4(), 4),
			(UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "x"), 5),
			(UuidGenerator.NewV6(Ms), 6), (UuidGenerator.NewV7(Ms), 7),
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

	// In SQL order a v6's random clock_seq sits where a v7's version nibble does and reads as 7
	// one time in 16; enough draws that a confusion would surface.
	[Fact]
	void Sql_ordered_v6_and_v7_are_validated_and_read_in_place_without_confusion()
	{
		for (var i = 0; i < 2048; i++)
		{
			var six = UuidGenerator.V6ToSqlOrder(UuidGenerator.NewV6(Ms));
			var seven = UuidGenerator.V7ToSqlOrder(UuidGenerator.NewV7(Ms));

			UuidGenerator.Version(six, UuidLayout.SqlServer).ShouldBe(6);
			UuidGenerator.Version(seven, UuidLayout.SqlServer).ShouldBe(7);
			UuidGenerator.IsRfc(six, 6, UuidLayout.SqlServer).ShouldBeTrue();
			UuidGenerator.IsRfc(six, 7, UuidLayout.SqlServer).ShouldBeFalse();
			UuidGenerator.IsRfc(seven, 7, UuidLayout.SqlServer).ShouldBeTrue();
			UuidGenerator.IsRfc(seven, 6, UuidLayout.SqlServer).ShouldBeFalse();

			UuidGenerator.V6UnixMillis(six, UuidLayout.SqlServer).ShouldBe(Ms);
			UuidGenerator.V7UnixMillis(seven, UuidLayout.SqlServer).ShouldBe(Ms);
			UuidGenerator.GetTimestamp(six, UuidLayout.SqlServer).ShouldBe(DateTimeOffset.FromUnixTimeMilliseconds(Ms));
			UuidGenerator.GetTimestamp(seven, UuidLayout.SqlServer).ShouldBe(DateTimeOffset.FromUnixTimeMilliseconds(Ms));
			// Read straight from SQL order matches permuting back first.
			UuidGenerator.V7UnixMillis(seven, UuidLayout.SqlServer)
				.ShouldBe(UuidGenerator.V7UnixMillis(UuidGenerator.V7FromSqlOrder(seven)));
			// The raw form takes the bytes SQL Server stores, which is Guid.ToByteArray().
			UuidGenerator.Version(seven.ToByteArray(), UuidLayout.SqlServer).ShouldBe(7);
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
			Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.V6UnixMillis(id, layout)).ParamName.ShouldBe("layout");
			Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.V7Timestamp(id, layout)).ParamName.ShouldBe("layout");
			Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.V6Timestamp(id, layout)).ParamName.ShouldBe("layout");
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
	// at most a millisecond past the supplied timestamp (the roll-forward over the wrap).
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
		UuidGenerator.V7UnixMillis(last).ShouldBeInRange(Ms, Ms + 1);
	}
}
