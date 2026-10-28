#!/usr/bin/env ruby
# frozen_string_literal: true

$LOAD_PATH.unshift(File.expand_path("../lib", __dir__))

require "benchmark/ips"
require "securerandom"
require "hyperuuid"

NAME = "example.com"
# The explicit-millis row isolates the binding's own cost from the OS wall-clock read the
# default rows also pay (a machine whose clock defeats the vDSO fast path — WSL2 on arm64
# was one — prices clock_gettime(CLOCK_REALTIME) at ~1 µs, a real syscall; most Linux prices
# it at tens of nanoseconds, and there the two sets of rows land together). Its millisecond is
# the present, read once per benchmark-ips cycle outside the calls: the default rows move the
# v7 stream to the present, so a fixed timestamp from the past would be more than 11 s behind
# it and time the out-of-stream path instead.
def now_ms = Process.clock_gettime(Process::CLOCK_REALTIME, :millisecond)

# Every number below belongs to one backend on one Ruby, so say which before printing any:
# run it again under HYPERUUID_PURE=1 for the other one.
puts "backend: #{HyperUuid::BACKEND} (core #{HyperUuid.native_version}) — #{RUBY_DESCRIPTION}"
puts

puts "== single-item generation =="
Benchmark.ips do |x|
  x.report("SecureRandom.uuid") { SecureRandom.uuid }
  x.report("HyperUuid.new_v4") { HyperUuid.new_v4 }
  x.report("HyperUuid.new_v5") { HyperUuid.new_v5(HyperUuid::Namespaces::DNS, NAME) }
  x.report("HyperUuid.new_v7") { HyperUuid.new_v7 }
  ms = now_ms
  explicit_ms = -> { HyperUuid.new_v7(ms) }
  # benchmark-ips's own loop for a block that takes no argument, with the clock read added
  # once per cycle, so this row pays the same per-call block overhead as the rows above.
  x.report("HyperUuid.new_v7 (explicit ms)") do |times|
    ms = now_ms
    i = 0
    while i < times
      explicit_ms.call
      i += 1
    end
  end
  x.compare!
end

puts
puts "== batch(1000) vs 1000 individual calls =="
Benchmark.ips do |x|
  x.report("new_v7 x1000 (individual)") { 1000.times { HyperUuid.new_v7 } }
  x.report("new_v7_batch(1000)") { HyperUuid.new_v7_batch(1000) }
  x.compare!
end

# The README's "Bulk generation into bytes" figures: the same native call as the batch
# above, minus the Uuid objects built on top of it.
puts
puts "== batch(1000) as Uuid objects vs as raw bytes =="
Benchmark.ips do |x|
  x.report("new_v7_batch(1000)") { HyperUuid.new_v7_batch(1000) }
  x.report("new_v7_batch_bytes(1000)") { HyperUuid.new_v7_batch_bytes(1000) }
  x.compare!
end
