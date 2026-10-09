using Shouldly;
using System.Text.Json;

namespace HyperUuid.Tests;

/// <summary>
/// Replays the shared conformance corpus (<c>corpus/*.json</c> at the repository root),
/// the same files the Rust core's own suite replays, through this binding's public API.
/// Every value crosses as a <see cref="Guid"/> the way a caller holds one, so this pins the
/// <see cref="Guid"/> byte-order handling as well as the core: an RFC-ordered hex value
/// becomes a <see cref="Guid"/> through <c>bigEndian: true</c>, a SQL-ordered one through
/// the plain constructor, exactly as <see cref="UuidGenerator.V7ToSqlOrder(Guid)"/> returns
/// it. A vector that fails here is a break in the cross-language contract, and for
/// <c>sql_order.json</c> a change to data already persisted in SQL Server.
/// </summary>
public sealed class CorpusTests
{
	static readonly string _corpusDirectory = FindCorpusDirectory();

	static string FindCorpusDirectory()
	{
		for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir is not null; dir = dir.Parent)
		{
			var corpus = Path.Combine(dir.FullName, "corpus");
			if (Directory.Exists(corpus))
				return corpus;
		}
		throw new DirectoryNotFoundException($"corpus directory not found above {AppContext.BaseDirectory}");
	}

	static JsonElement[] Corpus(string name)
	{
		using var document = JsonDocument.Parse(File.ReadAllText(Path.Combine(_corpusDirectory, name)));
		return [.. document.RootElement.EnumerateArray().Select(vector => vector.Clone())];
	}

	static byte[] Hex(JsonElement vector, string field) => Convert.FromHexString(vector.GetProperty(field).GetString()!);

	static UuidLayout LayoutOf(JsonElement vector) => vector.GetProperty("layout").GetString() switch
	{
		"rfc9562" => UuidLayout.Rfc9562,
		"sql_server" => UuidLayout.SqlServer,
		var other => throw new InvalidDataException($"unknown layout {other}"),
	};

	// The Guid a caller holds for these bytes in this layout; see the class remarks.
	static Guid GuidOf(byte[] bytes, UuidLayout layout) =>
		layout == UuidLayout.SqlServer ? new Guid(bytes) : new Guid(bytes, bigEndian: true);

	[Fact]
	void V5_corpus()
	{
		foreach (var vector in Corpus("v5.json"))
		{
			var ns = vector.GetProperty("namespace").GetString() switch
			{
				"dns" => UuidGenerator.Namespaces.Dns,
				"url" => UuidGenerator.Namespaces.Url,
				"oid" => UuidGenerator.Namespaces.Oid,
				"x500" => UuidGenerator.Namespaces.X500,
				var other => throw new InvalidDataException($"unknown namespace {other}"),
			};
			var expected = new Guid(Hex(vector, "expect"), bigEndian: true);
			UuidGenerator.NewV5(ns, Hex(vector, "name_hex")).ShouldBe(expected, vector.ToString());
			if (vector.TryGetProperty("name", out var name))
				UuidGenerator.NewV5(ns, name.GetString()!).ShouldBe(expected, vector.ToString());
		}
	}

	[Fact]
	void Sql_order_corpus()
	{
		foreach (var vector in Corpus("sql_order.json"))
		{
			var rfcBytes = Hex(vector, "rfc");
			var sqlBytes = Hex(vector, "sql");
			var rfc = GuidOf(rfcBytes, UuidLayout.Rfc9562);
			var sql = GuidOf(sqlBytes, UuidLayout.SqlServer);
			var (toSql, toRfc) = vector.GetProperty("version").GetInt32() switch
			{
				6 => ((Func<Guid, Guid>)UuidGenerator.V6ToSqlOrder, (Func<Guid, Guid>)UuidGenerator.V6FromSqlOrder),
				7 => (UuidGenerator.V7ToSqlOrder, UuidGenerator.V7FromSqlOrder),
				var other => throw new InvalidDataException($"no SQL order for version {other}"),
			};
			toSql(rfc).ShouldBe(sql, vector.ToString());
			toRfc(sql).ShouldBe(rfc, vector.ToString());
			// What reaches SQL Server is Guid.ToByteArray(); it must be the corpus's wire bytes.
			toSql(rfc).ToByteArray().ShouldBe(sqlBytes, vector.ToString());

			// The raw-byte doors, in place.
			Span<byte> bytes = [.. rfcBytes];
			if (vector.GetProperty("version").GetInt32() == 6) UuidGenerator.V6ToSqlOrder(bytes); else UuidGenerator.V7ToSqlOrder(bytes);
			bytes.ToArray().ShouldBe(sqlBytes, vector.ToString());
			if (vector.GetProperty("version").GetInt32() == 6) UuidGenerator.V6FromSqlOrder(bytes); else UuidGenerator.V7FromSqlOrder(bytes);
			bytes.ToArray().ShouldBe(rfcBytes, vector.ToString());
		}
	}

	[Fact]
	void Timestamp_corpus()
	{
		foreach (var vector in Corpus("timestamp.json"))
		{
			var layout = LayoutOf(vector);
			var id = GuidOf(Hex(vector, "uuid"), layout);
			var version = vector.GetProperty("version").GetInt32();
			var millis = vector.GetProperty("unix_millis") is { ValueKind: JsonValueKind.Number } m ? m.GetInt64() : (long?)null;
			UuidGenerator.Version(id, layout).ShouldBe(version, vector.ToString());

			// A spec-valid v7 timestamp past year 9999 has no DateTimeOffset; the timestamp
			// doors throw for it (documented on V7Timestamp) while the millisecond doors read it.
			if (millis > DateTimeOffset.MaxValue.ToUnixTimeMilliseconds())
			{
				UuidGenerator.V7UnixMillis(id, layout).ShouldBe(millis.Value, vector.ToString());
				Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.GetTimestamp(id, layout));
				Should.Throw<ArgumentOutOfRangeException>(() => UuidGenerator.V7Timestamp(id, layout));
				continue;
			}
			var expected = millis is { } ms ? DateTimeOffset.FromUnixTimeMilliseconds(ms) : (DateTimeOffset?)null;

			UuidGenerator.GetTimestamp(id, layout).ShouldBe(expected, vector.ToString());
			if (layout == UuidLayout.Rfc9562)
			{
				UuidGenerator.GetTimestamp(id).ShouldBe(expected, vector.ToString());
				id.Timestamp.ShouldBe(expected, vector.ToString());
			}
			if (millis is not { } value)
				continue;
			switch (version)
			{
				case 6:
					UuidGenerator.V6UnixMillis(id, layout).ShouldBe(value, vector.ToString());
					UuidGenerator.V6Timestamp(id, layout).ShouldBe(expected!.Value, vector.ToString());
					if (layout == UuidLayout.Rfc9562)
						UuidGenerator.V6UnixMillis(id).ShouldBe(value, vector.ToString());
					break;
				case 7:
					UuidGenerator.V7UnixMillis(id, layout).ShouldBe(value, vector.ToString());
					UuidGenerator.V7Timestamp(id, layout).ShouldBe(expected!.Value, vector.ToString());
					if (layout == UuidLayout.Rfc9562)
						UuidGenerator.V7UnixMillis(id).ShouldBe(value, vector.ToString());
					break;
			}
		}
	}

	[Fact]
	void Inspect_corpus()
	{
		foreach (var vector in Corpus("inspect.json"))
		{
			var layout = LayoutOf(vector);
			var bytes = Hex(vector, "uuid");
			var id = GuidOf(bytes, layout);
			var version = vector.GetProperty("version").GetInt32();
			var isRfc = vector.GetProperty("is_rfc").GetBoolean();

			UuidGenerator.Version(id, layout).ShouldBe(version, vector.ToString());
			UuidGenerator.Version(bytes, layout).ShouldBe(version, vector.ToString());
			UuidGenerator.IsRfc(id, version, layout).ShouldBe(isRfc, vector.ToString());
			UuidGenerator.IsRfc(bytes, version, layout).ShouldBe(isRfc, vector.ToString());
			for (var other = 0; other <= 15; other++)
			{
				if (other != version)
					UuidGenerator.IsRfc(id, other, layout).ShouldBeFalse($"{vector} IsRfc({other})");
			}

			if (!vector.TryGetProperty("variant", out var variant))
				continue;
			var expected = variant.GetString() switch
			{
				"ncs" => UuidVariant.Ncs,
				"rfc9562" => UuidVariant.Rfc9562,
				"microsoft" => UuidVariant.Microsoft,
				"future" => UuidVariant.Future,
				var other => throw new InvalidDataException($"unknown variant {other}"),
			};
			UuidGenerator.Variant(id).ShouldBe(expected, vector.ToString());
			UuidGenerator.Variant(bytes).ShouldBe(expected, vector.ToString());
			UuidGenerator.Version(id).ShouldBe(version, vector.ToString());
			UuidGenerator.IsRfc(id, version).ShouldBe(isRfc, vector.ToString());
		}
	}
}
