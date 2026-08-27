# hyperuuid

**Ruby's own stdlib stops at `SecureRandom.uuid` — random v4, full stop. No v5, no v6, no v7. This gem is the whole RFC, with zero gem dependency beyond `Fiddle` (which ships with every Ruby install).**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, calling
directly into the native `libhyperuuid` shared library via
[`Fiddle`](https://docs.ruby-lang.org/en/master/Fiddle.html) — dlopen/dlsym plus a raw C-ABI
call, no runtime bridge. Bundles a native build for every supported platform (linux/darwin/
windows × x64/arm64) and picks the right one at runtime, the same trick the Go/Java
bindings use since RubyGems has no per-platform native selection wired up here yet.

```ruby
require "hyperuuid"

id = HyperUuid.new_v4
id2 = HyperUuid.new_v5(HyperUuid::Namespaces::DNS, "example.com")
id3 = HyperUuid.new_v6
id4 = HyperUuid.new_v7

id4.timestamp # recover the embedded UTC Time
id4.to_sql_order # byte order SQL Server's uniqueidentifier needs to sort by creation order

# One native call, one random-bytes fetch, one counter reservation for the whole batch:
batch = HyperUuid.new_v7_batch(1000)
```

Returns `HyperUuid::Uuid`, a minimal value object (`#bytes`, `#to_s`, `#version`, `#variant`,
comparable/hashable) — this gem has no runtime dependency on the `uuid` gem.
`HyperUuid::Namespaces::DNS`/`URL`/`OID`/`X500` are RFC 9562 Section 6.6's well-known
namespaces. `#timestamp` recovers the embedded UTC `Time` from a version 6 or 7 UUID.
`#to_sql_order`/`#from_sql_order` convert a version 6 or 7 UUID to and from the byte order SQL
Server's `uniqueidentifier` needs on the wire to sort by creation order (`#to_sql_order`
dispatches on the UUID's own version, matching `#timestamp`'s convention) — computed once in
the native Rust core rather than reimplemented in Ruby, and verified there (and independently
against the real `System.Data.SqlTypes.SqlGuid` comparator in the C# binding's test suite).
Same-millisecond v6 UUIDs aren't guaranteed to sort correctly afterward — v6 has no counter,
so `clock_seq`/`node` (not the timestamp) decide ties, the same pre-existing RFC 9562 v6
limitation plain order already has. `#from_sql_order` figures out which version to invert by checking a byte position that's
provably collision-free between the two (see the method's own doc comment).
`HyperUuid::Uuid::NIL`/`MAX` are the RFC 9562 §5.9/§5.10 special-value UUIDs.
`HyperUuid.new_v6_batch(count)`/`new_v7_batch(count)` generate `count` UUIDs sharing one
timestamp capture and one native call, instead of `count` of each.

## Why not `SecureRandom.uuid`?

`SecureRandom.uuid` only ever gives you a random v4 UUID — Ruby's stdlib has no built-in v5, v6, or v7 at all. If you need more than that, the choice is really "which gem":

1. **Full RFC 9562 coverage, one gem, zero extra dependency.** v4/v5/v6/v7 plus batch generation plus `Nil`/`Max`, and the only thing this gem adds to your `Gemfile.lock` beyond `Fiddle` — which is Ruby's own bundled FFI layer, not a third-party C extension to compile.
2. **No native-extension compile step.** Third-party UUID gems that go beyond v4 are typically pure Ruby or wrap a C extension compiled at install time; this gem `dlopen`s a prebuilt native library instead — nothing to compile on `bundle install`.
3. **Batch generation.** `new_v7_batch(1000)` shares one timestamp capture, one random-bytes fetch, and one counter reservation across the whole batch instead of paying per-item overhead a thousand times over.
4. **Cross-language consistency.** The same Rust core mints v5 namespace UUIDs for Python, Go, C#, and every other binding in this repo — verified in CI to match Python's own `uuid.uuid5` byte-for-byte. If your system isn't Ruby-only, no Ruby-only gem can offer that.

The honest trade-off: this gem `dlopen`s a native library instead of being pure Ruby, so it needs a platform-specific `libhyperuuid.so`/`.dylib`/`.dll` bundled alongside it. If plain v4 randomness is all you need, `SecureRandom.uuid` is simpler and already in stdlib — that's a completely reasonable choice.

## Benchmarks

Real numbers, `benchmark-ips` on Ruby 4.0.6, linux-arm64 (`ruby benchmark/uuid_benchmark.rb`) — not claimed, measured:

| Call | i/s | vs `SecureRandom.uuid` |
|---|---:|---:|
| `SecureRandom.uuid` | 651,909 | baseline |
| `HyperUuid.new_v4` | 500,765 | 1.30x slower |
| `HyperUuid.new_v5` | 319,013 | 2.04x slower |
| `HyperUuid.new_v7` | 249,230 | 2.62x slower |
| `HyperUuid.new_v6` | 243,862 | 2.67x slower |

Honestly: single-item calls lose to `SecureRandom.uuid`, and that gap is structural, not a bug to fix — every `HyperUuid.new_v*` call crosses into native code via `Fiddle`'s `dlopen`/`dlsym` call path, while `SecureRandom.uuid` never leaves the Ruby/C stdlib boundary Ruby itself was built with. No amount of tuning closes that gap for a single call; it's the fixed cost of getting v5/v6/v7 and cross-language consistency that `SecureRandom.uuid` structurally cannot offer at any speed.

Batch generation is where that fixed cost gets amortized away — one `Fiddle` crossing for the whole batch instead of one per item:

| Call | i/s |
|---|---:|
| `new_v6` × 1000 (individual) | 249.2 |
| `new_v6_batch(1000)` | 2,721.2 (**11.2x**) |
| `new_v7` × 1000 (individual) | 253.2 |
| `new_v7_batch(1000)` | 2,792.3 (**11.0x**) |

If you're minting one UUID at a time and only need v4, `SecureRandom.uuid` is faster and simpler — a genuinely reasonable choice. If you need v5/v6/v7, need many at once, or need this Ruby service's IDs to agree byte-for-byte with a Go or Python service's, that's what this gem is for.

## Install

Not yet published to RubyGems.org under a registered `SkunkWerkx`/`buvinghausen` presence —
for now this is proven by CI building and testing the native core plus this gem on real
hardware for every platform leg. Consume via a direct
`gem "hyperuuid", git: "https://github.com/SkunkWerkx/HyperUuid", glob: "ruby/*.gemspec"` in
the meantime.

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.
