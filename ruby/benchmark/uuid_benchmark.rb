#!/usr/bin/env ruby
# frozen_string_literal: true

$LOAD_PATH.unshift(File.expand_path("../lib", __dir__))

require "benchmark/ips"
require "securerandom"
require "hyperuuid"

NAME = "example.com"
# The RFC 9562 test-vector timestamp, the same fixed input the PHP benchmark uses: the
# explicit-millis rows isolate the binding's own cost from the OS wall-clock read the
# default rows also pay (WSL2 prices clock_gettime(CLOCK_REALTIME) at ~1 µs — a real
# syscall, no vDSO fast path there; bare-metal Linux prices it at tens of nanoseconds).
RFC_TEST_VECTOR_MS = 1_645_557_742_000

puts "== single-item generation =="
Benchmark.ips do |x|
  x.report("SecureRandom.uuid") { SecureRandom.uuid }
  x.report("HyperUuid.new_v4") { HyperUuid.new_v4 }
  x.report("HyperUuid.new_v5") { HyperUuid.new_v5(HyperUuid::Namespaces::DNS, NAME) }
  x.report("HyperUuid.new_v6") { HyperUuid.new_v6 }
  x.report("HyperUuid.new_v7") { HyperUuid.new_v7 }
  x.report("HyperUuid.new_v6 (explicit ms)") { HyperUuid.new_v6(RFC_TEST_VECTOR_MS) }
  x.report("HyperUuid.new_v7 (explicit ms)") { HyperUuid.new_v7(RFC_TEST_VECTOR_MS) }
  x.compare!
end

puts
puts "== batch(1000) vs 1000 individual calls =="
Benchmark.ips do |x|
  x.report("new_v6 x1000 (individual)") { 1000.times { HyperUuid.new_v6 } }
  x.report("new_v6_batch(1000)") { HyperUuid.new_v6_batch(1000) }
  x.report("new_v7 x1000 (individual)") { 1000.times { HyperUuid.new_v7 } }
  x.report("new_v7_batch(1000)") { HyperUuid.new_v7_batch(1000) }
  x.compare!
end
