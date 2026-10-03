# Runs inside the ruby.wasm interpreter `rbwasm build` made from ./Gemfile, under Node
# (node-test.mjs) and in headless Chrome (index.html). Raises on the first failed check, so
# both hosts see an exception rather than a page to read; returns the report otherwise.
require "/bundle/setup"
require "hyperuuid"

checks = []
check = lambda do |label, ok, detail = nil|
  raise "FAIL #{label}#{" (#{detail})" if detail}" unless ok

  checks << "ok #{label}#{" (#{detail})" if detail}"
end

check.("platform", RUBY_PLATFORM.include?("wasm32-wasi"), "#{RUBY_PLATFORM}, Ruby #{RUBY_VERSION}")
check.("Magnus backend", HyperUuid::BACKEND == :native, HyperUuid::BACKEND.inspect)
check.("core version matches the gem", HyperUuid.native_version == HyperUuid::VERSION, HyperUuid.native_version)
check.("available?", HyperUuid.available?)
v4 = HyperUuid.new_v4.to_s
check.("v4", v4.match?(/\A\h{8}-\h{4}-4\h{3}-[89ab]\h{3}-\h{12}\z/), v4)
v5 = HyperUuid.new_v5(HyperUuid::Namespaces::DNS, "www.example.com").to_s
check.("v5 test vector", v5 == "2ed6657d-e927-568b-95e1-2665a8aea6a2", v5)
check.("v6", HyperUuid.new_v6.to_s[14] == "6")
check.("v7 at a fixed time", HyperUuid.new_v7(1_700_000_000_000).to_s.start_with?("018bcfe5-6800-7"))
# The batch doors take their count as a u32, which Magnus 0.8.2 rejected for every value on
# a 32-bit target (magnus#187).
batch = HyperUuid.new_v7_batch(100).map(&:to_s)
check.("v7 batch is sorted and unique", batch.size == 100 && batch == batch.sort && batch.uniq.size == 100)
check.("v6 batch", HyperUuid.new_v6_batch(3).size == 3)
check.("v7 batch bytes", HyperUuid.new_v7_batch_bytes(1000).bytesize == 16_000)
check.("1000 v4 unique", Array.new(1000) { HyperUuid.new_v4.to_s }.uniq.size == 1000)
checks.join("\n")
