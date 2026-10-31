require "spec_helper"
require "hyperuuid"
require "open3"

# Cross-backend agreement: the Magnus extension and the pure-Fiddle fallback must be
# indistinguishable through the public surface. The whole main spec suite already runs under
# both backends (HYPERUUID_PURE=1 forces Fiddle); this file pins the *agreement* between them
# by comparing deterministic outputs across a subprocess boundary.
RSpec.describe "native backend" do
  before(:all) do
    skip "Magnus extension not loaded (BACKEND=#{HyperUuid::BACKEND})" unless
      HyperUuid::BACKEND == :native
  end

  def fiddle_eval(expression)
    lib = File.expand_path("../lib", __dir__)
    out, status = Open3.capture2(
      { "HYPERUUID_PURE" => "1" },
      RbConfig.ruby, "-I", lib, "-r", "hyperuuid", "-e", "print (#{expression})"
    )
    raise "fiddle subprocess failed: #{out}" unless status.success?

    out
  end

  it "reports the native backend" do
    expect(HyperUuid::BACKEND).to eq(:native)
    expect(fiddle_eval("HyperUuid::BACKEND")).to eq("fiddle")
  end

  it "agrees with the Fiddle backend on deterministic v5 generation" do
    native = HyperUuid.new_v5(HyperUuid::Namespaces::DNS, "example.com").to_s
    expect(fiddle_eval('HyperUuid.new_v5(HyperUuid::Namespaces::DNS, "example.com").to_s')).to eq(native)
  end

  it "agrees with the Fiddle backend on an empty v5 name" do
    native = HyperUuid.new_v5(HyperUuid::Namespaces::URL, "").to_s
    expect(fiddle_eval('HyperUuid.new_v5(HyperUuid::Namespaces::URL, "").to_s')).to eq(native)
  end

  it "agrees with the Fiddle backend on v7 timestamp extraction" do
    id = HyperUuid.new_v7(1_645_557_742_000)
    expect(id.timestamp.to_i).to eq(1_645_557_742)
    fiddle = fiddle_eval("HyperUuid::Uuid.parse(#{id.to_s.inspect}).timestamp.to_r.to_s")
    expect(Rational(fiddle)).to eq(id.timestamp.to_r)
  end

  it "agrees with the Fiddle backend on the v7 SQL-order permutations" do
    [HyperUuid.new_v7(1_645_557_742_000), HyperUuid.new_v7(253_402_300_799_999)].each do |id|
      native = id.to_sql_order.to_s
      expect(fiddle_eval("HyperUuid::Uuid.parse(#{id.to_s.inspect}).to_sql_order.to_s")).to eq(native)
      expect(fiddle_eval("HyperUuid::Uuid.parse(#{native.inspect}).from_sql_order.to_s")).to eq(id.to_s)
      expect(id.to_sql_order.from_sql_order).to eq(id)
    end
  end

  it "agrees with the Fiddle backend on version, variant and rfc? in both layouts" do
    values = [
      HyperUuid::Uuid::NIL, HyperUuid::Uuid::MAX, HyperUuid.new_v4,
      HyperUuid.new_v5(HyperUuid::Namespaces::DNS, "x"),
      HyperUuid::Uuid.parse("919108f7-52d1-4320-d9ac-f847db4148a8"), # Microsoft variant
      HyperUuid::Uuid.parse("017f22e2-79b0-7cc3-68c4-dc0c0c07398f"), # a v7 nibble, NCS variant
      HyperUuid::Uuid.parse("1ec9414c-232a-6b00-b3c8-9f6bdeced846"), # a v6, RFC 9562 A.5
      *Array.new(16) { HyperUuid.new_v7(1_645_557_742_000) }
    ]
    values += values.select { |id| id.rfc?(7) }.map(&:to_sql_order)
    # One source for both sides: evaluated here, and handed to the Fiddle subprocess as text.
    probe_source = <<~RUBY.tr("\n", " ")
      lambda { |id| [
        id.version, id.variant, id.version(layout: :sql_server),
        (0..15).map { |v| [id.rfc?(v), id.rfc?(v, layout: :sql_server)] },
        id.timestamp(raise_on_mismatch: false)&.to_r,
        id.timestamp(layout: :sql_server, raise_on_mismatch: false)&.to_r
      ] }
    RUBY
    probe = eval(probe_source)
    native = values.map { |id| probe.(id) }
    # Base64 across the pipe: Marshal's output is binary, and Windows writes the subprocess's
    # stdout in text mode, turning every 0x0A byte into CRLF and the stream into garbage.
    dump = "Marshal.dump(#{values.map(&:to_s).inspect}.map { |s| (#{probe_source}).(HyperUuid::Uuid.parse(s)) })"
    script = "[#{dump}].pack('m0')"
    expect(Marshal.load(fiddle_eval(script).unpack1("m0"))).to eq(native)
  end

  it "agrees with the Fiddle backend on from_sql_order" do
    sql = Array.new(64) { HyperUuid.new_v7.to_sql_order }
    native = sql.map { |id| id.from_sql_order.to_s }
    script = "#{sql.map(&:to_s).inspect}.map { |s| HyperUuid::Uuid.parse(s).from_sql_order.to_s }.join(',')"
    fiddle = fiddle_eval(script)
    expect(fiddle.split(",")).to eq(native)
  end

  it "raises the same ArgumentError for a v7 batch past MAX_V7_BATCH, at the door and in the backend" do
    message = HyperUuid::Runtime::V7_BATCH_TOO_LARGE
    rescued = "begin; %s; rescue ArgumentError => e; e.message; end"
    [
      ["HyperUuid.new_v7_batch(HyperUuid::MAX_V7_BATCH + 1)",
       -> { HyperUuid.new_v7_batch(HyperUuid::MAX_V7_BATCH + 1) }],
      ["HyperUuid::Runtime.new_v7_batch(HyperUuid::MAX_V7_BATCH + 1, 0)",
       -> { HyperUuid::Runtime.new_v7_batch(HyperUuid::MAX_V7_BATCH + 1, 0) }]
    ].each do |source, call|
      expect(&call).to raise_error(ArgumentError, message)
      expect(fiddle_eval(format(rescued, source))).to eq(message)
    end
  end

  it "raises the same error classes and messages for every SQL-order refusal" do
    rescued = "begin; %s; 'none'; rescue ArgumentError => e; \"\#{e.class}: \#{e.message}\"; end"
    # Fixed vectors (RFC 9562 A.3's v4, A.5's v6, A.6's v7 and its SQL form), so the
    # refusals happen every run: fresh values pass for the other order one time in 16.
    %w[
      919108f7-52d1-4320-9bac-f847db4148a8 1ec9414c-232a-6b00-b3c8-9f6bdeced846
      017f22e2-79b0-7cc3-98c4-dc0c0c07398f 0c07398f-dc0c-8c84-b30d-017f22e279b0
    ].map { |text| HyperUuid::Uuid.parse(text) }.each do |id|
      %i[to_sql_order from_sql_order].each do |method|
        source = "HyperUuid::Uuid.parse(#{id.to_s.inspect}).#{method}"
        native =
          begin
            id.public_send(method)
            "none"
          rescue ArgumentError => e
            "#{e.class}: #{e.message}"
          end
        expect(fiddle_eval(format(rescued, source))).to eq(native)
      end
    end
  end

  it "agrees with the Fiddle backend on the core's version and availability" do
    expect(fiddle_eval("HyperUuid.native_version")).to eq(HyperUuid.native_version)
    expect(fiddle_eval("HyperUuid.available?")).to eq(HyperUuid.available?.to_s)
  end

  it "raises the package's own error classes from the extension, worded as Fiddle words them" do
    # 2**60 crosses the ABI and is refused by the core, so the message comes from the
    # extension's own Rust text — which has to match what runtime.rb gives Fiddle.
    rescued = "begin; %s; rescue HyperUuid::TimestampOutOfRangeError => e; e.message; end"
    v7 = HyperUuid::Runtime::V7_TIMESTAMP_OUT_OF_RANGE
    [
      ["HyperUuid.new_v7(2**60)", -> { HyperUuid.new_v7(2**60) }, v7],
      ["HyperUuid.new_v7_batch(2, 2**60)", -> { HyperUuid.new_v7_batch(2, 2**60) }, v7]
    ].each do |source, call, message|
      expect(&call).to raise_error(HyperUuid::TimestampOutOfRangeError, message)
      expect(fiddle_eval(format(rescued, source))).to eq(message)
    end
  end

  it "returns binary frozen bytes through Uuid exactly like the Fiddle backend" do
    id = HyperUuid.new_v4
    expect(id.bytes.encoding).to eq(Encoding::BINARY)
    expect(id.bytes).to be_frozen
    expect(id.bytes.bytesize).to eq(16)
  end

  # The extension caches Ruby objects in Rust statics, out of the garbage collector's sight. A
  # constant keeps them from being collected but not from being moved, so each is pinned when the
  # extension loads; unpinned, a compacting collection moved them and the next call used whatever
  # took their place (a segfault, or an exception class that was some other object). Every movable
  # object is moved here first, in a subprocess so a regression fails this example, not the run.
  it "keeps working after a compacting collection has moved everything it can" do
    skip "this Ruby's GC does not compact" unless GC.respond_to?(:verify_compaction_references)

    lib = File.expand_path("../lib", __dir__)
    script = <<~RUBY
      GC.verify_compaction_references(expand_heap: true, toward: :empty)
      begin
        HyperUuid.new_v7(1 << 60)
      rescue HyperUuid::TimestampOutOfRangeError => e
        print HyperUuid::BACKEND, " ", e.class
      end
    RUBY
    out, status = Open3.capture2e(RbConfig.ruby, "-I", lib, "-r", "hyperuuid", "-e", script)
    expect([status.success?, out]).to eq([true, "native HyperUuid::TimestampOutOfRangeError"])
  end
end
