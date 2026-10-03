require "hyperuuid"
require "open3"

RFC_TEST_VECTOR_MS = 1_645_557_742_000

# Replicates System.Data.SqlTypes.SqlGuid.CompareTo's fixed byte significance order — the
# correctness oracle this project's C# test suite checks directly against the real type; no
# Ruby equivalent exists to test against here, so this stands in for it. Shared by the v6 and
# v7 #to_sql_order specs below, since the significance order itself doesn't depend on version.
def sql_guid_cmp(a, b)
  significance_order = [10, 11, 12, 13, 14, 15, 8, 9, 6, 7, 4, 5, 0, 1, 2, 3]
  significance_order.each do |i|
    cmp = a.getbyte(i) <=> b.getbyte(i)
    return cmp unless cmp.zero?
  end
  0
end

RSpec.describe HyperUuid do
  describe ".new_v4" do
    it "has version and variant bits set" do
      id = described_class.new_v4
      expect(id.version).to eq(4)
      expect(id.variant).to eq(0b10)
    end

    it "is non-deterministic" do
      results = Array.new(100) { described_class.new_v4 }
      expect(results.uniq.size).to eq(100)
    end
  end

  describe ".new_v5" do
    it "matches the RFC 9562 Appendix A.4 test vector" do
      id = described_class.new_v5(HyperUuid::Namespaces::DNS, "www.example.com")
      expect(id).to eq(HyperUuid::Uuid.parse("2ed6657d-e927-568b-95e1-2665a8aea6a2"))
    end

    it "matches Python's uuid documentation test vector" do
      id = described_class.new_v5(HyperUuid::Namespaces::DNS, "python.org")
      expect(id).to eq(HyperUuid::Uuid.parse("886313e1-3b8a-5372-9b90-0c9aee199e5d"))
    end

    it "is deterministic" do
      a = described_class.new_v5(HyperUuid::Namespaces::DNS, "same-name")
      b = described_class.new_v5(HyperUuid::Namespaces::DNS, "same-name")
      expect(a).to eq(b)
    end

    it "differs across namespaces" do
      dns = described_class.new_v5(HyperUuid::Namespaces::DNS, "test")
      url = described_class.new_v5(HyperUuid::Namespaces::URL, "test")
      expect(dns).not_to eq(url)
    end

    it "agrees for a String name and its raw ASCII-8BIT bytes" do
      a = described_class.new_v5(HyperUuid::Namespaces::URL, "test-name")
      b = described_class.new_v5(HyperUuid::Namespaces::URL, "test-name".b)
      expect(a).to eq(b)
    end

    it "handles multi-byte UTF-8 names" do
      a = described_class.new_v5(HyperUuid::Namespaces::URL, "café — 日本語")
      b = described_class.new_v5(HyperUuid::Namespaces::URL, "café — 日本語")
      expect(a).to eq(b)
    end

    it "accepts an empty name, matching Python's uuid5 for it" do
      # uuid.uuid5(uuid.NAMESPACE_URL, "") — the SHA-1 of the namespace alone. The empty name
      # crosses to the core as a valid pointer with length 0, never as NULL.
      id = described_class.new_v5(HyperUuid::Namespaces::URL, "")
      expect(id).to eq(HyperUuid::Uuid.parse("1b4db7eb-4057-5ddf-91e0-36dec72071f5"))
      expect(described_class.new_v5(HyperUuid::Namespaces::URL, "".b)).to eq(id)
    end

    it "treats a namespace that isn't a Uuid, or a name that isn't a String, as a caller bug" do
      expect { described_class.new_v5("6ba7b810-9dad-11d1-80b4-00c04fd430c8", "name") }
        .to raise_error(TypeError, /namespace must be a HyperUuid::Uuid; got String/)
      expect { described_class.new_v5(HyperUuid::Namespaces::DNS, :name) }
        .to raise_error(TypeError, /name must be a String; got Symbol/)
      expect { described_class.new_v5(HyperUuid::Namespaces::DNS, nil) }
        .to raise_error(TypeError, /name must be a String; got NilClass/)
    end
  end

  describe ".new_v6" do
    it "embeds the given timestamp" do
      id = described_class.new_v6(RFC_TEST_VECTOR_MS)
      expect(id.timestamp).to eq(Time.at(RFC_TEST_VECTOR_MS / 1000.0).utc)
    end

    it "has version and variant bits set" do
      id = described_class.new_v6(RFC_TEST_VECTOR_MS)
      expect(id.version).to eq(6)
      expect(id.variant).to eq(0b10)
    end

    it "sets the node ID multicast bit" do
      id = described_class.new_v6(RFC_TEST_VECTOR_MS)
      expect(id.bytes.getbyte(10) & 0x01).to eq(1)
    end

    it "is non-deterministic within the same millisecond" do
      results = Array.new(100) { described_class.new_v6(RFC_TEST_VECTOR_MS) }
      expect(results.uniq.size).to eq(100)
    end

    it "embeds the current time when called with no argument" do
      before = (Time.now.to_r * 1000).to_i
      id = described_class.new_v6
      after = (Time.now.to_r * 1000).to_i

      expect((id.timestamp.to_r * 1000).to_i).to be_between(before, after)
    end
  end

  describe ".new_v6_batch" do
    it "returns count UUIDs sharing the given timestamp" do
      ids = described_class.new_v6_batch(10, RFC_TEST_VECTOR_MS)
      expect(ids.size).to eq(10)
      ids.each do |id|
        expect(id.version).to eq(6)
        expect(id.timestamp).to eq(Time.at(RFC_TEST_VECTOR_MS / 1000.0).utc)
      end
    end

    it "produces pairwise-distinct UUIDs" do
      ids = described_class.new_v6_batch(100, RFC_TEST_VECTOR_MS)
      expect(ids.uniq.size).to eq(100)
    end

    it "returns an empty array for count zero" do
      expect(described_class.new_v6_batch(0, RFC_TEST_VECTOR_MS)).to eq([])
    end

    it "raises on an out-of-range timestamp" do
      expect { described_class.new_v6_batch(1, 0xFFFF_FFFF_FFFF_FFFF) }
        .to raise_error(HyperUuid::TimestampOutOfRangeError)
    end
  end

  describe ".new_v7" do
    it "embeds the given timestamp" do
      id = described_class.new_v7(RFC_TEST_VECTOR_MS)
      embedded_ms = id.bytes[0, 6].bytes.reduce(0) { |acc, b| (acc << 8) | b }
      expect(embedded_ms).to eq(RFC_TEST_VECTOR_MS)
    end

    it "has version and variant bits set" do
      id = described_class.new_v7(RFC_TEST_VECTOR_MS)
      expect(id.version).to eq(7)
      expect(id.variant).to eq(0b10)
    end

    it "raises on an out-of-range timestamp" do
      expect { described_class.new_v7(0x0001_0000_0000_0000) }
        .to raise_error(HyperUuid::TimestampOutOfRangeError)
    end

    it "produces a monotonically ordered batch within the same millisecond" do
      ids = Array.new(100) { described_class.new_v7(RFC_TEST_VECTOR_MS) }
      expect(ids.map(&:to_s)).to eq(ids.map(&:to_s).sort)
    end

    it "embeds the current time when called with no argument" do
      before = (Time.now.to_r * 1000).to_i
      id = described_class.new_v7
      after = (Time.now.to_r * 1000).to_i

      embedded_ms = id.bytes[0, 6].bytes.reduce(0) { |acc, b| (acc << 8) | b }
      expect(embedded_ms).to be_between(before, after)
    end
  end

  describe ".new_v7_batch" do
    it "returns count UUIDs sharing the given timestamp, sorted" do
      ids = described_class.new_v7_batch(1000, RFC_TEST_VECTOR_MS)
      expect(ids.size).to eq(1000)
      expect(ids.map(&:to_s)).to eq(ids.map(&:to_s).sort)
      ids.each { |id| expect(id.timestamp).to eq(Time.at(RFC_TEST_VECTOR_MS / 1000.0).utc) }
    end

    it "continues the same counter sequence as individual calls" do
      before = described_class.new_v7(RFC_TEST_VECTOR_MS)
      batch = described_class.new_v7_batch(10, RFC_TEST_VECTOR_MS)
      after = described_class.new_v7(RFC_TEST_VECTOR_MS)

      ids = [before, *batch, after]
      expect(ids.map(&:to_s)).to eq(ids.map(&:to_s).sort)
    end

    it "returns an empty array for count zero" do
      expect(described_class.new_v7_batch(0, RFC_TEST_VECTOR_MS)).to eq([])
    end

    it "raises on an out-of-range timestamp" do
      expect { described_class.new_v7_batch(1, 0x0001_0000_0000_0000) }
        .to raise_error(HyperUuid::TimestampOutOfRangeError)
    end
  end

  describe "Uuid#timestamp" do
    it "recovers the exact millisecond the v7 UUID was created with" do
      id = described_class.new_v7(RFC_TEST_VECTOR_MS)
      expect(id.timestamp).to eq(Time.at(RFC_TEST_VECTOR_MS / 1000.0).utc)
    end

    it "round-trips zero and the max 48-bit timestamp" do
      expect(described_class.new_v7(0).timestamp).to eq(Time.at(0).utc)

      max_ms = 0x0000_FFFF_FFFF_FFFF
      id = described_class.new_v7(max_ms)
      expect((id.timestamp.to_r * 1000).to_i).to eq(max_ms)
    end

    it "raises for a non-time-based version by default" do
      expect { described_class.new_v4.timestamp }.to raise_error(ArgumentError)
    end

    it "returns nil for a non-time-based version when raise_on_mismatch is false" do
      expect(described_class.new_v4.timestamp(raise_on_mismatch: false)).to be_nil
    end

    it "still returns the real timestamp for v6/v7 when raise_on_mismatch is false" do
      id = described_class.new_v6(RFC_TEST_VECTOR_MS)
      expect(id.timestamp(raise_on_mismatch: false)).to eq(Time.at(RFC_TEST_VECTOR_MS / 1000.0).utc)
    end
  end

  describe "creating from a Time" do
    it ".new_v6 accepts a Time in place of a millisecond integer" do
      time = Time.at(RFC_TEST_VECTOR_MS / 1000.0).utc
      id = described_class.new_v6(time)
      expect((id.timestamp.to_r * 1000).to_i).to eq(RFC_TEST_VECTOR_MS)
    end

    it ".new_v7 accepts a Time in place of a millisecond integer" do
      time = Time.at(RFC_TEST_VECTOR_MS / 1000.0).utc
      id = described_class.new_v7(time)
      expect((id.timestamp.to_r * 1000).to_i).to eq(RFC_TEST_VECTOR_MS)
    end
  end

  describe "Uuid#to_sql_order" do
    it "round-trips through #from_sql_order" do
      id = described_class.new_v7(RFC_TEST_VECTOR_MS)
      sql_ordered = id.to_sql_order
      expect(sql_ordered).not_to eq(id)
      expect(sql_ordered.from_sql_order).to eq(id)
    end

    it "preserves the version and variant bits at octets 7 and 8" do
      sql_ordered = described_class.new_v7(RFC_TEST_VECTOR_MS).to_sql_order
      expect(sql_ordered.bytes.getbyte(7) & 0xF0).to eq(0x70)
      expect(sql_ordered.bytes.getbyte(8) & 0xC0).to eq(0x80)
    end

    it "sorts by creation order under SqlGuid-style comparison" do
      ids = (0...200).map { |i| described_class.new_v7(RFC_TEST_VECTOR_MS + i) }
      ids += Array.new(200) { described_class.new_v7(RFC_TEST_VECTOR_MS + 1_000_000) }

      sql_ordered = ids.map { |id| id.to_sql_order.bytes }
      sorted = sql_ordered.sort { |a, b| sql_guid_cmp(a, b) }

      expect(sorted).to eq(sql_ordered)
    end
  end

  describe "Uuid#to_sql_order for v6" do
    it "round-trips through #from_sql_order" do
      id = described_class.new_v6(RFC_TEST_VECTOR_MS)
      sql_ordered = id.to_sql_order
      expect(sql_ordered).not_to eq(id)
      expect(sql_ordered.from_sql_order).to eq(id)
    end

    it "preserves the version and variant bits at v6's (different from v7's) offsets" do
      sql_ordered = described_class.new_v6(RFC_TEST_VECTOR_MS).to_sql_order
      expect(sql_ordered.bytes.getbyte(8) & 0xF0).to eq(0x60)
      expect(sql_ordered.bytes.getbyte(6) & 0xC0).to eq(0x80)
    end

    it "sorts by creation order under SqlGuid-style comparison for distinct timestamps" do
      # Unlike v7, v6 has no counter — two UUIDs at the same millisecond aren't guaranteed to
      # sort in creation order even in plain RFC order, so this only exercises strictly
      # increasing timestamps, where the timestamp alone determines order with no tie to break.
      ids = (0...300).map { |i| described_class.new_v6(RFC_TEST_VECTOR_MS + i) }

      sql_ordered = ids.map { |id| id.to_sql_order.bytes }
      sorted = sql_ordered.sort { |a, b| sql_guid_cmp(a, b) }

      expect(sorted).to eq(sql_ordered)
    end

    it "#from_sql_order round-trips both v6- and v7-sql-ordered values back to the right version" do
      v6_sql = described_class.new_v6(RFC_TEST_VECTOR_MS).to_sql_order
      v7_sql = described_class.new_v7(RFC_TEST_VECTOR_MS).to_sql_order

      expect(v6_sql.from_sql_order.version).to eq(6)
      expect(v7_sql.from_sql_order.version).to eq(7)
    end
  end

  describe "Uuid::NIL and Uuid::MAX" do
    it "NIL is all zero bytes" do
      expect(HyperUuid::Uuid::NIL.bytes).to eq("\x00".b * 16)
      expect(HyperUuid::Uuid::NIL.to_s).to eq("00000000-0000-0000-0000-000000000000")
    end

    it "MAX is all one bytes" do
      expect(HyperUuid::Uuid::MAX.bytes).to eq("\xFF".b * 16)
      expect(HyperUuid::Uuid::MAX.to_s).to eq("ffffffff-ffff-ffff-ffff-ffffffffffff")
    end

    it "round-trip through parse" do
      expect(HyperUuid::Uuid.parse(HyperUuid::Uuid::NIL.to_s)).to eq(HyperUuid::Uuid::NIL)
      expect(HyperUuid::Uuid.parse(HyperUuid::Uuid::MAX.to_s)).to eq(HyperUuid::Uuid::MAX)
    end
  end

  describe ".new_v7_batch_bytes" do
    it "returns one binary String of 16 bytes per UUID" do
      bytes = described_class.new_v7_batch_bytes(64, RFC_TEST_VECTOR_MS)
      expect(bytes).to be_a(String)
      expect(bytes.encoding).to eq(Encoding::BINARY)
      expect(bytes.bytesize).to eq(64 * 16)
    end

    it "decodes to version 7 UUIDs carrying the requested timestamp" do
      bytes = described_class.new_v7_batch_bytes(32, RFC_TEST_VECTOR_MS)
      ids = Array.new(32) { |i| HyperUuid::Uuid.new(bytes[i * 16, 16]) }
      expect(ids.map(&:version).uniq).to eq([7])
      expect(ids.map { |i| (i.timestamp.to_f * 1000).round }.uniq).to eq([RFC_TEST_VECTOR_MS])
    end

    it "is strictly increasing and free of duplicates" do
      bytes = described_class.new_v7_batch_bytes(256, RFC_TEST_VECTOR_MS)
      raw = Array.new(256) { |i| bytes[i * 16, 16] }
      expect(raw).to eq(raw.sort)
      expect(raw.uniq.size).to eq(256)
    end

    it "agrees structurally with the object-returning batch" do
      bytes = described_class.new_v7_batch_bytes(16, RFC_TEST_VECTOR_MS)
      from_bytes = Array.new(16) { |i| HyperUuid::Uuid.new(bytes[i * 16, 16]) }
      from_objects = described_class.new_v7_batch(16, RFC_TEST_VECTOR_MS)
      expect(from_bytes.map(&:version)).to eq(from_objects.map(&:version))
      expect(from_bytes.map(&:timestamp)).to eq(from_objects.map(&:timestamp))
    end

    it "defaults to the current time" do
      before = (Time.now.to_f * 1000).to_i
      bytes = described_class.new_v7_batch_bytes(1)
      after = (Time.now.to_f * 1000).to_i
      ms = (HyperUuid::Uuid.new(bytes[0, 16]).timestamp.to_f * 1000).round
      expect(ms).to be_between(before - 1000, after + 1000)
    end
  end

  describe ".new_v6_batch_bytes" do
    it "returns one binary String of 16 bytes per UUID" do
      bytes = described_class.new_v6_batch_bytes(8, RFC_TEST_VECTOR_MS)
      expect(bytes.bytesize).to eq(8 * 16)
      ids = Array.new(8) { |i| HyperUuid::Uuid.new(bytes[i * 16, 16]) }
      expect(ids.map(&:version).uniq).to eq([6])
    end
  end

  # Caller bugs are caught once, in the shared doors, so the error is the same whichever
  # backend is live — this whole suite runs under both. Left to the backends these cases
  # diverged: a RangeError from the extension where Fiddle wrapped the value and raised
  # TimestampOutOfRangeError, a NoMemoryError for a negative count.
  describe "caller bugs" do
    v6_doors = {
      "new_v6" => ->(ms) { HyperUuid.new_v6(ms) },
      "new_v6_batch" => ->(ms) { HyperUuid.new_v6_batch(2, ms) },
      "new_v6_batch_bytes" => ->(ms) { HyperUuid.new_v6_batch_bytes(2, ms) }
    }
    v7_doors = {
      "new_v7" => ->(ms) { HyperUuid.new_v7(ms) },
      "new_v7_batch" => ->(ms) { HyperUuid.new_v7_batch(2, ms) },
      "new_v7_batch_bytes" => ->(ms) { HyperUuid.new_v7_batch_bytes(2, ms) }
    }

    { HyperUuid::Runtime::V6_TIMESTAMP_OUT_OF_RANGE => v6_doors,
      HyperUuid::Runtime::V7_TIMESTAMP_OUT_OF_RANGE => v7_doors }.each do |message, doors|
      doors.each do |name, door|
        it "#{name} raises TimestampOutOfRangeError, with one message, for every out-of-range time" do
          # The first three never reach a backend (they cannot cross the ABI as a u64); the
          # last is in range for the ABI and refused by the core itself. Same class, same
          # message, either way.
          [-1, Time.at(-1), 2**64, 0xFFFF_FFFF_FFFF_FFFF].each do |time|
            expect { door.call(time) }.to raise_error(HyperUuid::TimestampOutOfRangeError, message)
          end
        end

        it "#{name} raises TypeError for a time that is neither a Time, an Integer nor nil" do
          [1.5, "1645557742000", :now].each do |time|
            expect { door.call(time) }.to raise_error(TypeError, /unix_millis must be a Time, an Integer or nil/)
          end
        end
      end
    end

    %i[new_v6_batch new_v7_batch new_v6_batch_bytes new_v7_batch_bytes].each do |door|
      it "#{door} raises ArgumentError for a count outside 0..2**32 - 1" do
        [-1, 2**32].each do |count|
          expect { described_class.public_send(door, count, RFC_TEST_VECTOR_MS) }
            .to raise_error(ArgumentError, /count must be between 0 and 4294967295; got #{count}/)
        end
      end

      it "#{door} raises TypeError for a count that isn't an Integer" do
        [2.0, "2", nil].each do |count|
          expect { described_class.public_send(door, count, RFC_TEST_VECTOR_MS) }
            .to raise_error(TypeError, /count must be an Integer/)
        end
      end
    end
  end

  describe "the exception classes" do
    it "live on HyperUuid itself, as StandardErrors" do
      expect(HyperUuid::TimestampOutOfRangeError.name).to eq("HyperUuid::TimestampOutOfRangeError")
      expect(HyperUuid::RandomSourceError.name).to eq("HyperUuid::RandomSourceError")
      expect(HyperUuid::TimestampOutOfRangeError.superclass).to be(StandardError)
      expect(HyperUuid::RandomSourceError.superclass).to be(StandardError)
    end

    it "keep their earlier names under Runtime as aliases of the same classes" do
      expect(HyperUuid::Runtime::TimestampOutOfRangeError).to be(HyperUuid::TimestampOutOfRangeError)
      expect(HyperUuid::Runtime::RandomSourceError).to be(HyperUuid::RandomSourceError)
      expect { described_class.new_v7(2**60) }.to raise_error(HyperUuid::Runtime::TimestampOutOfRangeError)
    end

    it "word a random-source failure the same way on every backend" do
      # The failure itself cannot be provoked from here; the message is built in one place
      # per backend, and this is the Ruby one the Fiddle backend raises.
      error = HyperUuid::Runtime.random_source_failure("uuid_new_v4")
      expect(error).to be_a(HyperUuid::RandomSourceError)
      expect(error.message).to eq("uuid_new_v4: the system random source failed")
    end
  end

  describe ".native_version and .available?" do
    it "reports the loaded core's version, pinned to this gem's own" do
      # VERSION and rust/Cargo.toml are swept together by prepare-release.yml, so the loaded
      # core must always report exactly this gem's version.
      expect(described_class.native_version).to match(/\A\d+\.\d+\.\d+\z/)
      expect(described_class.native_version).to eq(HyperUuid::VERSION)
    end

    it "answers available? without raising, and consistently with native_version" do
      expect(described_class.available?).to be(true)
      expect(described_class.available?).to be(true) # cached
    end

    it "answers available? false without raising when no library resolves, while the doors still raise" do
      # A Fiddle subprocess with the library path stubbed away: the probe answers quietly,
      # the first door call keeps its precise LoadError.
      lib = File.expand_path("../lib", __dir__)
      script = "HyperUuid::Runtime.singleton_class.define_method(:library_path) { nil }; " \
               "print HyperUuid.available?; print ' '; " \
               "begin; HyperUuid.new_v4; rescue LoadError; print 'raised'; end"
      out, status = Open3.capture2({ "HYPERUUID_PURE" => "1" },
                                   RbConfig.ruby, "-I", lib, "-r", "hyperuuid", "-e", script)
      expect(status).to be_success
      expect(out).to eq("false raised")
    end

    it "names the universal gem when a platform gem, which carries no Fiddle library, falls to Fiddle" do
      musl = Gem::Platform.new("x86_64-linux-musl")
      forced = HyperUuid::Runtime.send(:missing_library_message, musl, true)
      expect(forced).to include("x86_64-linux-musl platform gem", "HYPERUUID_PURE forces",
                                "gem install hyperuuid --platform ruby")
      unloaded = HyperUuid::Runtime.send(:missing_library_message, musl, false)
      expect(unloaded).to include("none of its extensions loads on this Ruby (#{RUBY_VERSION}",
                                  "gem install hyperuuid --platform ruby")
      expect(unloaded).not_to include("HYPERUUID_PURE")
      [["ruby", true], [nil, false]].each do |platform, pure|
        expect(HyperUuid::Runtime.send(:missing_library_message, platform, pure))
          .to match(/not found \(unsupported platform/)
      end
    end
  end

  describe "Uuid.parse" do
    it "accepts exactly the 8-4-4-4-12 shape #to_s produces, in either case" do
      id = described_class.new_v4
      expect(HyperUuid::Uuid.parse(id.to_s)).to eq(id)
      expect(HyperUuid::Uuid.parse(id.to_s.upcase)).to eq(id)
    end

    it "rejects everything else as an ArgumentError" do
      canonical = "6ba7b810-9dad-11d1-80b4-00c04fd430c8"
      [
        canonical.delete("-"),             # the bare 32 hex digits
        "6ba7b8109-dad-11d1-80b4-00c04fd430c8", # a hyphen in the wrong place
        "-#{canonical.delete('-')}---",    # the right number of hyphens, nowhere right
        "{#{canonical}}", "urn:uuid:#{canonical}", " #{canonical}", "#{canonical}\n",
        canonical[0..-2], "#{canonical}0", canonical.tr("6", "g"), "", nil, 42
      ].each do |text|
        expect { HyperUuid::Uuid.parse(text) }.to raise_error(ArgumentError, /invalid UUID string/)
      end
    end
  end

  # Runs under every backend, and matters most under Fiddle: Fiddle releases the GVL for the
  # duration of a call, so that is the one backend where Ruby threads run the core truly in
  # parallel, each through its own scratch buffer. (The Magnus extension holds the GVL.)
  describe "concurrent callers" do
    it "keep their own results: a deterministic v5 per thread never sees another thread's bytes" do
      names = Array.new(8) { |n| "thread-#{n}.example.com" }
      expected = names.map { |name| described_class.new_v5(HyperUuid::Namespaces::DNS, name) }
      results = names.map do |name|
        Thread.new { Array.new(500) { described_class.new_v5(HyperUuid::Namespaces::DNS, name) } }
      end.map(&:value)

      results.each_with_index { |ids, n| expect(ids.uniq).to eq([expected[n]]) }
    end

    it "mint distinct, well-formed UUIDs, single calls and batches interleaved" do
      ids = Array.new(8) do
        Thread.new do
          Array.new(100) { [described_class.new_v7, described_class.new_v4, *described_class.new_v7_batch(8)] }
        end
      end.flat_map(&:value).flatten

      expect(ids.size).to eq(8 * 100 * 10)
      expect(ids.uniq.size).to eq(ids.size)
      expect(ids.map(&:version).tally).to eq(7 => 8 * 100 * 9, 4 => 8 * 100)
      expect(ids).to all(satisfy { |id| id.variant == 0b10 })
    end
  end
end
