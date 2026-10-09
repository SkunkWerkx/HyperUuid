require "hyperuuid"
require "json"

# Replays the repository's conformance corpus (corpus/README.md): fixed vectors every binding
# must reproduce byte for byte, whichever backend is live — the whole suite runs under both.
# Every hex value is the bytes in memory order, and a Uuid holds exactly those bytes in either
# layout: #to_sql_order returns the core's SQL Server bytes as they are, so a `sql` hex string
# is wrapped as-is, the same as an `rfc` one.
CORPUS_DIR = begin
  dir = __dir__
  dir = File.dirname(dir) until File.exist?(File.join(dir, "corpus", "README.md")) || File.dirname(dir) == dir
  File.join(dir, "corpus")
end

def corpus(name)
  JSON.parse(File.read(File.join(CORPUS_DIR, name)))
end

def uuid_of(hex)
  HyperUuid::Uuid.new([hex].pack("H*"))
end

CORPUS_LAYOUTS = { "rfc9562" => :rfc9562, "sql_server" => :sql_server }.freeze

RSpec.describe "conformance corpus" do
  it "is found by walking up from the spec" do
    expect(File).to exist(File.join(CORPUS_DIR, "inspect.json"))
  end

  describe "v5.json" do
    namespaces = {
      "dns" => HyperUuid::Namespaces::DNS, "url" => HyperUuid::Namespaces::URL,
      "oid" => HyperUuid::Namespaces::OID, "x500" => HyperUuid::Namespaces::X500
    }

    corpus("v5.json").each do |row|
      it "#{row['namespace']} #{row['name_hex']} through the bytes door" do
        name = [row["name_hex"]].pack("H*")
        expect(HyperUuid.new_v5(namespaces.fetch(row["namespace"]), name)).to eq(uuid_of(row["expect"]))
      end

      next unless row.key?("name")

      it "#{row['namespace']} #{row['name'].inspect} through the text door" do
        name = row["name"].encode(Encoding::UTF_8)
        expect(HyperUuid.new_v5(namespaces.fetch(row["namespace"]), name)).to eq(uuid_of(row["expect"]))
      end
    end
  end

  describe "sql_order.json" do
    corpus("sql_order.json").each do |row|
      it "v#{row['version']} #{row['rfc']} <-> #{row['sql']}" do
        rfc = uuid_of(row["rfc"])
        sql = uuid_of(row["sql"])
        expect(rfc.to_sql_order.bytes.unpack1("H*")).to eq(row["sql"])
        expect(sql.from_sql_order.bytes.unpack1("H*")).to eq(row["rfc"])
        expect(sql.version(layout: :sql_server)).to eq(row["version"])
      end
    end
  end

  describe "timestamp.json" do
    corpus("timestamp.json").each do |row|
      it "#{row['layout']} #{row['uuid']}" do
        id = uuid_of(row["uuid"])
        layout = CORPUS_LAYOUTS.fetch(row["layout"])
        code = HyperUuid::Uuid::LAYOUTS.fetch(layout)
        millis = row["unix_millis"]
        expected = millis && Time.at(millis / 1000, millis % 1000, :millisecond).utc

        expect(id.version(layout: layout)).to eq(row["version"])
        expect(id.timestamp(layout: layout, raise_on_mismatch: false)).to eq(expected)
        # The one strict read #timestamp is, straight from the backend.
        expect(HyperUuid::Runtime.get_timestamp(id.bytes, code)).to eq(millis)
        if layout == :rfc9562
          expect(id.timestamp(raise_on_mismatch: false)).to eq(expected)
          expect(id.version).to eq(row["version"])
        end

        if millis.nil?
          expect { id.timestamp(layout: layout) }.to raise_error(ArgumentError)
        else
          expect(id.timestamp(layout: layout)).to eq(expected)
          expect((id.timestamp(layout: layout).to_r * 1000).to_i).to eq(millis)
          # The per-version millisecond reads, which trust the caller's version and check nothing.
          expect(HyperUuid::Runtime.public_send(:"v#{row['version']}_unix_millis_in", id.bytes, code)).to eq(millis)
          if layout == :rfc9562
            expect(HyperUuid::Runtime.public_send(:"v#{row['version']}_unix_millis", id.bytes)).to eq(millis)
          end
        end
      end
    end
  end

  describe "inspect.json" do
    corpus("inspect.json").each do |row|
      it "#{row['layout']} #{row['uuid']} (#{row['note']})" do
        id = uuid_of(row["uuid"])
        layout = CORPUS_LAYOUTS.fetch(row["layout"])
        version = row["version"]

        expect(id.version(layout: layout)).to eq(version)
        expect(id.rfc?(version, layout: layout)).to eq(row["is_rfc"])
        (0..15).each do |other|
          next if other == version

          expect(id.rfc?(other, layout: layout)).to be(false)
        end

        next unless row.key?("variant")

        # The corpus spells the variants exactly as #variant's Symbols.
        expect(id.variant).to eq(row["variant"].to_sym)
        expect(id.version).to eq(version)
        expect(id.rfc?(version)).to eq(row["is_rfc"])
      end
    end
  end
end
