require "hyperuuid"

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
        .to raise_error(HyperUuid::Runtime::TimestampOutOfRangeError)
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
        .to raise_error(HyperUuid::Runtime::TimestampOutOfRangeError)
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
        .to raise_error(HyperUuid::Runtime::TimestampOutOfRangeError)
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
end
