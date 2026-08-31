require "spec_helper"
require "open3"

# Cross-backend agreement: the Magnus extension and the pure-Fiddle fallback must be
# indistinguishable through the public surface. Ruby is the only binding that still needs
# this contract — Python retired its second backend once abi3 wheels made ctypes redundant,
# leaving nothing to disagree with. The whole main spec suite already runs under both
# backends (HYPERUUID_PURE=1 forces Fiddle); this file pins the *agreement* between them by
# comparing deterministic outputs across a subprocess boundary.
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
  end

  it "agrees with the Fiddle backend on deterministic v5 generation" do
    native = HyperUuid.new_v5(HyperUuid::Namespaces::DNS, "example.com").to_s
    expect(fiddle_eval('HyperUuid.new_v5(HyperUuid::Namespaces::DNS, "example.com").to_s')).to eq(native)
  end

  it "agrees with the Fiddle backend on v7 timestamp extraction" do
    id = HyperUuid.new_v7(1_645_557_742_000)
    expect(id.timestamp.to_i).to eq(1_645_557_742)
    fiddle = fiddle_eval("HyperUuid::Uuid.parse(#{id.to_s.inspect}).timestamp.to_r.to_s")
    expect(Rational(fiddle)).to eq(id.timestamp.to_r)
  end

  it "agrees with the Fiddle backend on the SQL-order permutation" do
    id = HyperUuid.new_v7(1_645_557_742_000)
    native = id.to_sql_order.to_s
    expect(fiddle_eval("HyperUuid::Uuid.parse(#{id.to_s.inspect}).to_sql_order.to_s")).to eq(native)
    expect(id.to_sql_order.from_sql_order).to eq(id)
  end

  it "raises the package's own error classes from the extension" do
    expect { HyperUuid.new_v7(2**60) }
      .to raise_error(HyperUuid::Runtime::TimestampOutOfRangeError)
  end

  it "returns binary frozen bytes through Uuid exactly like the Fiddle backend" do
    id = HyperUuid.new_v4
    expect(id.bytes.encoding).to eq(Encoding::BINARY)
    expect(id.bytes).to be_frozen
    expect(id.bytes.bytesize).to eq(16)
  end
end
