require "spec_helper"
require "hyperuuid"
require "open3"
require "rbconfig"

# Driving the real v7 stream to its ceiling poisons the process-wide state for everything
# after it, so the whole script runs in a child process, once per backend.
RSpec.describe "v7 counter exhaustion" do
  message = "version 7 counter exhausted: the stream cannot advance without running more than " \
            "1000 ms ahead of the supplied timestamp or past its last millisecond"

  script = <<~RUBY
    M = 17_592_186_044_414 # 2**44 - 2, the last millisecond the stream holds
    MESSAGE = #{message.inspect}

    def exhausted?
      yield
      false
    rescue HyperUuid::CounterExhaustedError => e
      raise "wrong message: \#{e.message.inspect}" unless e.message == MESSAGE

      true
    end

    # 1. One single v7 at M succeeds.
    HyperUuid.new_v7(M)

    # 2. Batches of 65,536 at M until the first error: the batch site.
    raise "no batch exhausted the stream" unless
      1025.times.any? { exhausted? { HyperUuid.new_v7_batch_bytes(65_536, M) } }

    # 3. Singles at M until the first error: the single site.
    raise "no single exhausted the stream" unless
      1024.times.any? { exhausted? { HyperUuid.new_v7(M) } }

    # 4. Every v7 door now fails at once.
    raise "single" unless exhausted? { HyperUuid.new_v7(M) }
    raise "batch" unless exhausted? { HyperUuid.new_v7_batch(1, M) }
    raise "batch_bytes" unless exhausted? { HyperUuid.new_v7_batch_bytes(1, M) }
    print HyperUuid::BACKEND
  RUBY

  def run_child(script, env)
    lib = File.expand_path("../lib", __dir__)
    out, err, status = Open3.capture3(env, RbConfig.ruby, "-I", lib, "-r", "hyperuuid", "-e", script)
    [out, err, status]
  end

  it "raises CounterExhaustedError at every v7 door on the Fiddle backend" do
    out, err, status = run_child(script, { "HYPERUUID_PURE" => "1" })
    expect(status).to be_success, err
    expect(out).to eq("fiddle")
  end

  it "raises CounterExhaustedError at every v7 door on the Magnus backend" do
    skip "Magnus extension not loaded (BACKEND=#{HyperUuid::BACKEND})" unless HyperUuid::BACKEND == :native

    out, err, status = run_child(script, {})
    expect(status).to be_success, err
    expect(out).to eq("native")
  end
end
