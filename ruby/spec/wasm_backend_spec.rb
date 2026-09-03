require "spec_helper"
require "hyperuuid"
require "open3"

# Cross-backend agreement for the WebAssembly backend, the same contract
# native_backend_spec.rb pins for the Magnus extension: through the public surface, the
# core running as a wasm32-wasip1 module inside wasmtime must be indistinguishable from the
# same core dlopen'd through Fiddle. The whole main suite already runs under this backend
# (HYPERUUID_WASM=1); this file pins the agreement itself by comparing deterministic outputs
# across a subprocess boundary.
RSpec.describe "wasm backend" do
  before(:all) do
    skip "wasm backend not loaded (BACKEND=#{HyperUuid::BACKEND})" unless
      HyperUuid::BACKEND == :wasm
  end

  def fiddle_eval(expression)
    lib = File.expand_path("../lib", __dir__)
    out, status = Open3.capture2(
      { "HYPERUUID_PURE" => "1", "HYPERUUID_WASM" => nil },
      RbConfig.ruby, "-I", lib, "-r", "hyperuuid", "-e", "print (#{expression})"
    )
    raise "fiddle subprocess failed: #{out}" unless status.success?
    out
  end

  it "reports the wasm backend" do
    expect(HyperUuid::BACKEND).to eq(:wasm)
  end

  it "agrees with the Fiddle backend on deterministic v5 generation" do
    wasm = HyperUuid.new_v5(HyperUuid::Namespaces::DNS, "example.com").to_s
    expect(fiddle_eval('HyperUuid.new_v5(HyperUuid::Namespaces::DNS, "example.com").to_s')).to eq(wasm)
  end

  it "agrees with the Fiddle backend on an empty v5 name" do
    wasm = HyperUuid.new_v5(HyperUuid::Namespaces::URL, "").to_s
    expect(fiddle_eval('HyperUuid.new_v5(HyperUuid::Namespaces::URL, "").to_s')).to eq(wasm)
  end

  it "agrees with the Fiddle backend on v7 timestamp extraction" do
    id = HyperUuid.new_v7(1_645_557_742_000)
    expect(id.timestamp.to_i).to eq(1_645_557_742)
    fiddle = fiddle_eval("HyperUuid::Uuid.parse(#{id.to_s.inspect}).timestamp.to_r.to_s")
    expect(Rational(fiddle)).to eq(id.timestamp.to_r)
  end

  it "agrees with the Fiddle backend on the SQL-order permutation" do
    id = HyperUuid.new_v7(1_645_557_742_000)
    wasm = id.to_sql_order.to_s
    expect(fiddle_eval("HyperUuid::Uuid.parse(#{id.to_s.inspect}).to_sql_order.to_s")).to eq(wasm)
    expect(id.to_sql_order.from_sql_order).to eq(id)
  end

  it "raises the package's own error classes from the module" do
    expect { HyperUuid.new_v7(2**60) }
      .to raise_error(HyperUuid::Runtime::TimestampOutOfRangeError)
    expect { HyperUuid.new_v6_batch(1, 0xFFFF_FFFF_FFFF_FFFF) }
      .to raise_error(HyperUuid::Runtime::TimestampOutOfRangeError)
  end

  it "keeps a batch strictly ascending through the guest's own allocator" do
    # The regression this backend exists to avoid: a batch destination the guest allocator
    # could reclaim. 1000 items sharing one timestamp must come back in counter order.
    ids = HyperUuid.new_v7_batch(1000, 1_645_557_742_000)
    expect(ids).to eq(ids.sort)
    expect(ids.uniq.size).to eq(1000)
  end

  it "returns binary frozen bytes through Uuid exactly like the Fiddle backend" do
    id = HyperUuid.new_v4
    expect(id.bytes.encoding).to eq(Encoding::BINARY)
    expect(id.bytes).to be_frozen
    expect(id.bytes.bytesize).to eq(16)
  end

  it "serializes concurrent callers on the one shared instance" do
    ids = Array.new(8) { Thread.new { Array.new(200) { HyperUuid.new_v7 } } }.flat_map(&:value)
    expect(ids.uniq.size).to eq(1600)
    expect(ids).to all(satisfy { |id| id.version == 7 })
  end
end
