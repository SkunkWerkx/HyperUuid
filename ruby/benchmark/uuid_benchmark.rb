#!/usr/bin/env ruby
# frozen_string_literal: true

$LOAD_PATH.unshift(File.expand_path("../lib", __dir__))

require "benchmark/ips"
require "securerandom"
require "hyperuuid"

NAME = "example.com"

puts "== single-item generation =="
Benchmark.ips do |x|
  x.report("SecureRandom.uuid") { SecureRandom.uuid }
  x.report("HyperUuid.new_v4") { HyperUuid.new_v4 }
  x.report("HyperUuid.new_v5") { HyperUuid.new_v5(HyperUuid::Namespaces::DNS, NAME) }
  x.report("HyperUuid.new_v6") { HyperUuid.new_v6 }
  x.report("HyperUuid.new_v7") { HyperUuid.new_v7 }
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
