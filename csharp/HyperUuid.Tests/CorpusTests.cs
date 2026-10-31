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
/// <c>sql_order.json</c> a change to data already persisted in SQL Server;
/// <c>sql_order_refused.json</c> pins what the conversions refuse, and how.
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
			UuidGenerator.V7ToSqlOrder(rfc).ShouldBe(sql, vector.ToString());
			UuidGenerator.V7FromSqlOrder(sql).ShouldBe(rfc, vector.ToString());
			// Every SQL-ordered value is a version 8 to the Guid; the layout names its source.
			sql.Version.ShouldBe(8, vector.ToString());
			UuidGenerator.Version(sql, UuidLayout.SqlServer).ShouldBe(7, vector.ToString());
			// What reaches SQL Server is Guid.ToByteArray(); it must be the corpus's wire bytes.
			UuidGenerator.V7ToSqlOrder(rfc).ToByteArray().ShouldBe(sqlBytes, vector.ToString());

			// The raw-byte doors, in place.
			Span<byte> bytes = [.. rfcBytes];
			UuidGenerator.V7ToSqlOrder(bytes);
			bytes.ToArray().ShouldBe(sqlBytes, vector.ToString());
			UuidGenerator.V7FromSqlOrder(bytes);
			bytes.ToArray().ShouldBe(rfcBytes, vector.ToString());
		}
	}

	delegate void InPlace(Span<byte> uuid);

	// Each refused value fails its conversion with the binding's wrong-version error, an
	// ArgumentException naming the parameter. The value is fed the way a caller holds it in the input's layout
	// (RFC order into a To conversion, SQL order into a From one), and the in-place byte form
	// must leave the buffer untouched.
	[Fact]
	void Sql_order_refused_corpus()
	{
		foreach (var vector in Corpus("sql_order_refused.json"))
		{
			var bytes = Hex(vector, "uuid");
			var (toSql, convert, inPlace) = vector.GetProperty("direction").GetString() switch
			{
				"to_sql" => (true, (Func<Guid, Guid>)UuidGenerator.V7ToSqlOrder, (InPlace)UuidGenerator.V7ToSqlOrder),
				"from_sql" => (false, (Func<Guid, Guid>)UuidGenerator.V7FromSqlOrder, (InPlace)UuidGenerator.V7FromSqlOrder),
				var other => throw new InvalidDataException($"unknown direction {other}"),
			};
			const string message = "the value is not a UUID of the version this conversion takes";
			var id = GuidOf(bytes, toSql ? UuidLayout.Rfc9562 : UuidLayout.SqlServer);

			foreach (var thrown in new[]
			{
				Should.Throw<Exception>(() => convert(id), vector.ToString()),
				Should.Throw<Exception>(() => inPlace(bytes.AsSpan()), vector.ToString()),
			})
			{
				thrown.GetType().ShouldBe(typeof(ArgumentException), vector.ToString());
				thrown.Message.ShouldStartWith(message, Case.Sensitive, vector.ToString());
				((ArgumentException)thrown).ParamName.ShouldBe("uuid", vector.ToString());
			}
			bytes.ShouldBe(Hex(vector, "uuid"), vector.ToString());
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

			// DateTimeOffset's ceiling is the generation cap, so a v7 another generator dated past
			// it has no timestamp at the version-agnostic doors (as a version 6 has none), while
			// the per-version millisecond door reads the field and V7Timestamp, which builds a
			// DateTimeOffset, throws for it as documented.
			if (millis > DateTimeOffset.MaxValue.ToUnixTimeMilliseconds())
			{
				UuidGenerator.V7UnixMillis(id, layout).ShouldBe(millis.Value, vector.ToString());
				UuidGenerator.GetTimestamp(id, layout).ShouldBeNull(vector.ToString());
				if (layout == UuidLayout.Rfc9562)
				{
					UuidGenerator.GetTimestamp(id).ShouldBeNull(vector.ToString());
					id.Timestamp.ShouldBeNull(vector.ToString());
				}
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
			// Only a version 7 carries a timestamp; a version 6 (or 1) row has none.
			if (millis is not { } value)
				continue;
			version.ShouldBe(7, vector.ToString());
			UuidGenerator.V7UnixMillis(id, layout).ShouldBe(value, vector.ToString());
			UuidGenerator.V7Timestamp(id, layout).ShouldBe(expected!.Value, vector.ToString());
			if (layout == UuidLayout.Rfc9562)
				UuidGenerator.V7UnixMillis(id).ShouldBe(value, vector.ToString());
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
