using Shouldly;

namespace HyperUuid.Tests;

public sealed class GuidExtensionsTests
{
	// RFC 9562 Appendix A.6's timestamp, the same fixed input UuidGeneratorTests uses.
	private const long RfcTestVectorMs = 1645557742000;

	[Fact]
	public void Timestamp_RecoversTheEmbeddedV7Timestamp()
	{
		var id = UuidGenerator.NewV7(RfcTestVectorMs);
		id.Timestamp.ShouldBe(DateTimeOffset.FromUnixTimeMilliseconds(RfcTestVectorMs));
	}

	// RFC 9562 Appendix A.5's version 6: it has a timestamp field, but this library reads
	// timestamps from version 7 only, so it answers null like any other non-v7 value.
	static readonly Guid RfcV6 = new("1ec9414c-232a-6b00-b3c8-9f6bdeced846");

	[Fact]
	public void Timestamp_IsNullForAVersion6()
	{
		UuidGenerator.Version(RfcV6).ShouldBe(6);
		RfcV6.Timestamp.ShouldBeNull();
	}

	[Fact]
	public void Timestamp_IsNullForVersionsThatCarryNoTimestamp()
	{
		UuidGenerator.NewV4().Timestamp.ShouldBeNull();
		UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "test").Timestamp.ShouldBeNull();
		UuidGenerator.Nil.Timestamp.ShouldBeNull();
		UuidGenerator.Max.Timestamp.ShouldBeNull();
	}

	// The extension member must stay a re-spelling of the static method, never a second
	// implementation that can drift from it — the same cross-path agreement rule the Ruby
	// binding pins between its Magnus and Fiddle backends.
	[Fact]
	public void Timestamp_AgreesWithGetTimestampOnEveryVersion()
	{
		Guid[] ids =
		[
			UuidGenerator.NewV4(),
			UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "www.example.com"),
			RfcV6,
			UuidGenerator.NewV7(RfcTestVectorMs),
			UuidGenerator.NewV7(0),
			UuidGenerator.Nil,
			UuidGenerator.Max,
		];

		foreach (var id in ids)
		{
			id.Timestamp.ShouldBe(UuidGenerator.GetTimestamp(id));
		}
	}

	// Reads on a plain BCL-produced Guid too — the extension is on Guid itself, not on
	// anything this package constructs, so it must not assume provenance.
	[Fact]
	public void Timestamp_IsNullForABclGuid()
	{
		Guid.NewGuid().Timestamp.ShouldBeNull();
		Guid.Empty.Timestamp.ShouldBeNull();
	}

	// Guid.CreateVersion7 is the BCL's own v7, produced with no involvement from this
	// package: the timestamp must still come back, since both write RFC 9562 §6.2 layout.
	[Fact]
	public void Timestamp_ReadsABclCreateVersion7Guid()
	{
		var created = DateTimeOffset.FromUnixTimeMilliseconds(RfcTestVectorMs);
		Guid.CreateVersion7(created).Timestamp.ShouldBe(created);
	}

	[Fact]
	public void Timestamp_ComposesWithNullHandling()
	{
		var fallback = DateTimeOffset.UnixEpoch;
		UuidGenerator.NewV4().Timestamp.ShouldBeNull();
		(UuidGenerator.NewV4().Timestamp ?? fallback).ShouldBe(fallback);

		var id = UuidGenerator.NewV7(RfcTestVectorMs);
		id.Timestamp.ShouldNotBeNull();
		(id.Timestamp is { } recovered).ShouldBeTrue();
	}
}
