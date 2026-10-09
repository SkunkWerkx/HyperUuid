require "hyperuuid"
require "securerandom"

# Version/variant inspection, the layout-aware doors and the v7 batch limit against values
# minted at run time, by this gem and by Ruby's own SecureRandom; corpus_spec.rb pins the
# fixed vectors.
RSpec.describe "UUID inspection" do
  ms = 1_645_557_742_000
  at_ms = Time.at(ms / 1000, ms % 1000, :millisecond).utc

  it "reports the version and the RFC variant of values from this gem and from SecureRandom" do
    [
      [HyperUuid::Uuid.parse(SecureRandom.uuid), 4], [HyperUuid::Uuid.parse(SecureRandom.uuid_v7), 7],
      [HyperUuid.new_v4, 4], [HyperUuid.new_v5(HyperUuid::Namespaces::DNS, "x"), 5],
      [HyperUuid.new_v6(ms), 6], [HyperUuid.new_v7(ms), 7]
    ].each do |id, version|
      expect(id.version).to eq(version)
      expect(id.version(layout: :rfc9562)).to eq(version)
      expect(id.variant).to eq(:rfc9562)
      expect(id.rfc?(version)).to be(true)
      expect(id.rfc?(version, layout: :rfc9562)).to be(true)
      expect(id.rfc?(version == 7 ? 6 : 7)).to be(false)
    end
  end

  it "classifies Nil and Max as the RFC says" do
    expect(HyperUuid::Uuid::NIL.version).to eq(0)
    expect(HyperUuid::Uuid::NIL.variant).to eq(:ncs)
    expect(HyperUuid::Uuid::MAX.version).to eq(15)
    expect(HyperUuid::Uuid::MAX.variant).to eq(:future)
    expect(HyperUuid::Uuid::NIL.rfc?(0)).to be(false)
    expect(HyperUuid::Uuid::MAX.rfc?(15)).to be(false)
  end

  it "never matches a version outside the nibble" do
    id = HyperUuid.new_v7(ms)
    [7 + 256, -1, 16, 2**64].each do |version|
      expect(id.rfc?(version)).to be(false)
      expect(id.to_sql_order.rfc?(version, layout: :sql_server)).to be(false)
    end
  end

  # A 6 or 7 in the nibble is not a version 6 or 7 UUID unless the variant is RFC 9562's.
  it "finds no timestamp in a 6 or 7 nibble under another variant, in either layout" do
    v7 = HyperUuid.new_v7(ms).bytes.dup
    v6 = HyperUuid.new_v6(ms).bytes.dup
    { 0x00 => :ncs, 0xC0 => :microsoft, 0xE0 => :future }.each do |bits, variant|
      [v6, v7].each do |rfc_bytes|
        broken = rfc_bytes.dup
        broken.setbyte(8, (broken.getbyte(8) & 0x1F) | bits)
        id = HyperUuid::Uuid.new(broken)
        found = id.version
        expect(id.variant).to eq(variant)
        expect(id.rfc?(found)).to be(false)
        expect(id.timestamp(raise_on_mismatch: false)).to be_nil
        expect { id.timestamp }.to raise_error(
          ArgumentError,
          "timestamp is only defined for version 6 or 7 UUIDs, got version #{found} with the #{variant.inspect} variant"
        )
      end
    end
    # And in SQL order, where the variant sits in octet 8 (v7) or octet 6 (v6).
    sql = HyperUuid.new_v7(ms).to_sql_order.bytes.dup
    sql.setbyte(8, sql.getbyte(8) & 0x3F)
    id = HyperUuid::Uuid.new(sql)
    expect(id.timestamp(layout: :sql_server, raise_on_mismatch: false)).to be_nil
    expect(id.version(layout: :sql_server)).to eq(0)
  end

  it "raises TypeError for a version that isn't an Integer" do
    expect { HyperUuid.new_v7(ms).rfc?("7") }.to raise_error(TypeError, /version must be an Integer/)
  end

  # In SQL order a v6's random clock_seq sits where a v7's version nibble does and reads as 7
  # one time in 16; enough draws that a confusion would surface.
  it "validates and reads SQL-ordered v6 and v7 in place without confusing them" do
    2048.times do
      six = HyperUuid.new_v6(ms).to_sql_order
      seven = HyperUuid.new_v7(ms).to_sql_order

      expect(six.version(layout: :sql_server)).to eq(6)
      expect(seven.version(layout: :sql_server)).to eq(7)
      expect(six.rfc?(6, layout: :sql_server)).to be(true)
      expect(six.rfc?(7, layout: :sql_server)).to be(false)
      expect(seven.rfc?(7, layout: :sql_server)).to be(true)
      expect(seven.rfc?(6, layout: :sql_server)).to be(false)

      expect(six.timestamp(layout: :sql_server)).to eq(at_ms)
      expect(seven.timestamp(layout: :sql_server)).to eq(at_ms)
      # Read straight from SQL order matches permuting back first.
      expect(seven.timestamp(layout: :sql_server)).to eq(seven.from_sql_order.timestamp)
      expect(six.from_sql_order.version).to eq(6)
      expect(seven.from_sql_order.version).to eq(7)
    end
  end

  # Fixed vectors only: a random v4's RFC bytes read as a SQL-ordered v7 one time in 16 (octet 8
  # already carries the RFC variant and octet 7 is random), which is correct, since the layout
  # is the caller's to know.
  it "finds no SQL version in values that aren't SQL-ordered v6 or v7" do
    [
      HyperUuid::Uuid::NIL, HyperUuid::Uuid::MAX, HyperUuid::Uuid.parse("919108f7-52d1-4320-9bac-f847db4148a8"),
      HyperUuid.new_v5(HyperUuid::Namespaces::DNS, "www.example.com")
    ].each do |id|
      expect(id.version(layout: :sql_server)).to eq(0)
      expect(id.timestamp(layout: :sql_server, raise_on_mismatch: false)).to be_nil
      expect { id.timestamp(layout: :sql_server) }
        .to raise_error(ArgumentError, "timestamp: not a SQL-ordered version 6 or 7 UUID")
      expect { id.from_sql_order }.to raise_error(ArgumentError, /not a recognized version 6 or 7/)
    end
  end

  it "raises ArgumentError for an unknown layout, before reaching the core" do
    id = HyperUuid.new_v7(ms)
    [nil, :rfc4122, "rfc9562", 1, 2, 0].each do |layout|
      message = "layout must be one of :rfc9562, :sql_server; got #{layout.inspect}"
      expect { id.version(layout: layout) }.to raise_error(ArgumentError, message)
      expect { id.rfc?(7, layout: layout) }.to raise_error(ArgumentError, message)
      expect { id.timestamp(layout: layout) }.to raise_error(ArgumentError, message)
      expect { id.timestamp(layout: layout, raise_on_mismatch: false) }.to raise_error(ArgumentError, message)
    end
  end

  it "requires exactly 16 bytes for a raw value" do
    [15, 17, 0].each do |size|
      expect { HyperUuid::Uuid.new("\x00".b * size) }.to raise_error(ArgumentError, /exactly 16 bytes/)
    end
  end

  describe "the v7 batch limit" do
    it "is the 26-bit counter space" do
      expect(HyperUuid::MAX_V7_BATCH).to eq(67_108_864)
    end

    # Refused before the backend is asked, so nothing is allocated; the backends refuse it on
    # their own as well, before their buffer, should anything reach them.
    %i[new_v7_batch new_v7_batch_bytes].each do |door|
      it "#{door} refuses one past it on every form, before reaching the backend" do
        allow(HyperUuid::Runtime).to receive(:new_v7_batch).and_call_original
        [[ms], [at_ms], []].each do |time|
          expect { HyperUuid.public_send(door, HyperUuid::MAX_V7_BATCH + 1, *time) }
            .to raise_error(ArgumentError, HyperUuid::Runtime::V7_BATCH_TOO_LARGE)
        end
        expect(HyperUuid::Runtime).not_to have_received(:new_v7_batch)
      end
    end

    it "is refused by the backend itself without allocating the batch" do
      expect { HyperUuid::Runtime.new_v7_batch(HyperUuid::MAX_V7_BATCH + 1, ms) }
        .to raise_error(ArgumentError, HyperUuid::Runtime::V7_BATCH_TOO_LARGE)
    end

    # Exactly the counter space: 1 GiB of bytes (2 GiB at peak), in strictly increasing order
    # end to end, and at most a millisecond past the supplied timestamp (the roll-forward over
    # the counter's wrap). Opt-in for its memory and its minute of Ruby-loop comparison.
    it "mints a batch of exactly the limit in strictly increasing order" do
      skip "set HYPERUUID_BIG_BATCH=1 to run the 1 GiB batch" unless ENV["HYPERUUID_BIG_BATCH"]

      bytes = HyperUuid.new_v7_batch_bytes(HyperUuid::MAX_V7_BATCH, ms)
      expect(bytes.bytesize).to eq(HyperUuid::MAX_V7_BATCH * 16)
      previous = bytes.byteslice(0, 16)
      out_of_order = (1...HyperUuid::MAX_V7_BATCH).count do |i|
        current = bytes.byteslice(i * 16, 16)
        bad = current <= previous
        previous = current
        bad
      end
      expect(out_of_order).to eq(0)
      last = HyperUuid::Uuid.new(previous)
      expect((last.timestamp.to_r * 1000).to_i).to be_between(ms, ms + 1)
    end
  end
end
