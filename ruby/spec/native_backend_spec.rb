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

  it "agrees with the Fiddle backend on v6 and v7 timestamp extraction" do
    [HyperUuid.new_v6(1_645_557_742_000), HyperUuid.new_v7(1_645_557_742_000)].each do |id|
      expect(id.timestamp.to_i).to eq(1_645_557_742)
      fiddle = fiddle_eval("HyperUuid::Uuid.parse(#{id.to_s.inspect}).timestamp.to_r.to_s")
      expect(Rational(fiddle)).to eq(id.timestamp.to_r)
    end
  end

  it "agrees with the Fiddle backend on the v6 and v7 SQL-order permutations" do
    [HyperUuid.new_v6(1_645_557_742_000), HyperUuid.new_v7(1_645_557_742_000)].each do |id|
      native = id.to_sql_order.to_s
      expect(fiddle_eval("HyperUuid::Uuid.parse(#{id.to_s.inspect}).to_sql_order.to_s")).to eq(native)
      expect(fiddle_eval("HyperUuid::Uuid.parse(#{native.inspect}).from_sql_order.to_s")).to eq(id.to_s)
      expect(id.to_sql_order.from_sql_order).to eq(id)
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
    v6 = HyperUuid::Runtime::V6_TIMESTAMP_OUT_OF_RANGE
    v7 = HyperUuid::Runtime::V7_TIMESTAMP_OUT_OF_RANGE
    [
      ["HyperUuid.new_v7(2**60)", -> { HyperUuid.new_v7(2**60) }, v7],
      ["HyperUuid.new_v7_batch(2, 2**60)", -> { HyperUuid.new_v7_batch(2, 2**60) }, v7],
      ["HyperUuid.new_v6(2**63)", -> { HyperUuid.new_v6(2**63) }, v6],
      ["HyperUuid.new_v6_batch(2, 2**63)", -> { HyperUuid.new_v6_batch(2, 2**63) }, v6]
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
