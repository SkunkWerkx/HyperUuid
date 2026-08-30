# hyperuuid

**Ruby's own stdlib stops at `SecureRandom.uuid` — random v4, full stop. No v5, no v6, no v7. This gem is the whole RFC, with zero gem dependency beyond `Fiddle` (which ships with every Ruby install) — and it's faster than `SecureRandom.uuid` too.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, with
two backends sharing one public surface. The fast path is a native extension built with
[Magnus](https://github.com/matsadler/magnus) — the Rust core linked directly into the Ruby
VM, auto-selected when loadable — which redefines the low-level `Runtime` methods in place
on require; everything above them (`Uuid`, the module doors, batch slicing) is shared
byte-for-byte between backends. The universal fallback calls the native `libhyperuuid`
shared library via [`Fiddle`](https://docs.ruby-lang.org/en/master/Fiddle.html) —
dlopen/dlsym plus a raw C-ABI call, no runtime bridge, nothing to compile on
`bundle install`. Set `HYPERUUID_PURE=1` to force the Fiddle backend;
`HyperUuid::BACKEND` reports which one is live. Bundles a native build for every supported
platform (linux/darwin/windows × x64/arm64) and picks the right one at runtime.

```ruby
require "hyperuuid"

id = HyperUuid.new_v4
id2 = HyperUuid.new_v5(HyperUuid::Namespaces::DNS, "example.com")
id3 = HyperUuid.new_v6
id4 = HyperUuid.new_v7

id4.timestamp # recover the embedded UTC Time
id4.timestamp(raise_on_mismatch: false) # nil instead of raising if id4 isn't v6/v7
id4.to_sql_order # byte order SQL Server's uniqueidentifier needs to sort by creation order

# One native call, one random-bytes fetch, one counter reservation for the whole batch:
batch = HyperUuid.new_v7_batch(1000)
```

Returns `HyperUuid::Uuid`, a minimal value object (`#bytes`, `#to_s`, `#version`, `#variant`,
comparable/hashable) — this gem has no runtime dependency on the `uuid` gem.
`HyperUuid::Namespaces::DNS`/`URL`/`OID`/`X500` are RFC 9562 Section 6.6's well-known
namespaces. `#timestamp` recovers the embedded UTC `Time` from a version 6 or 7 UUID; pass
`raise_on_mismatch: false` to get `nil` back for any other version instead of raising.
`.new_v6`/`.new_v7` also accept a `Time` directly in place of a raw millisecond count.
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
2. **No native-extension compile step.** Third-party UUID gems that go beyond v4 are typically pure Ruby or wrap a C extension compiled at install time; this gem ships its fast path as a prebuilt platform-gem extension and its fallback as a `dlopen`ed prebuilt library — either way, nothing to compile on `bundle install`.
3. **Batch generation.** `new_v7_batch(1000)` shares one timestamp capture, one random-bytes fetch, and one counter reservation across the whole batch instead of paying per-item overhead a thousand times over.
4. **Cross-language consistency.** The same Rust core mints v5 namespace UUIDs for Python, Go, C#, and every other binding in this repo — verified in CI to match Python's own `uuid.uuid5` byte-for-byte. If your system isn't Ruby-only, no Ruby-only gem can offer that.

The honest trade-off: this gem `dlopen`s a native library instead of being pure Ruby, so it needs a platform-specific `libhyperuuid.so`/`.dylib`/`.dll` bundled alongside it. If plain v4 randomness is all you need, `SecureRandom.uuid` is simpler and already in stdlib — that's a completely reasonable choice.

## Benchmarks

Real numbers, `benchmark-ips` on Ruby 4.0.6, linux-arm64 (`ruby benchmark/uuid_benchmark.rb`) — not claimed, measured. With the Magnus backend (the default wherever the extension loads):

| Call | i/s | vs `SecureRandom.uuid` |
|---|---:|---:|
| `SecureRandom.uuid` | 775,868 | baseline |
| `HyperUuid.new_v7` (explicit ms) | 2,275,763 | **2.9x faster** |
| `HyperUuid.new_v6` (explicit ms) | 2,197,698 | **2.8x faster** |
| `HyperUuid.new_v4` | 2,176,244 | **2.8x faster** |
| `HyperUuid.new_v5` | 1,458,385 | 1.9x faster |
| `HyperUuid.new_v7` (current time) | 701,373 | parity (1.1x slower) |
| `HyperUuid.new_v6` (current time) | 703,748 | parity (1.1x slower) |

An earlier edition of this section said single-item calls "lose to `SecureRandom.uuid`, full stop" and called the gap "structural, not a bug to fix — no amount of tuning closes that gap." That was wrong, and the receipts above are the correction: the gap was `Fiddle`'s per-call marshalling, and replacing the mechanism (the same play as this repo's Python PyO3 backend) closed it with room to spare. A `HyperUuid.new_v4` — real entropy, correct version/variant bits, minted by the shared Rust core — now costs a third of what `SecureRandom.uuid` does.

The two "current time" rows deserve their honest footnote: the explicit-ms rows isolate the binding's own cost (~440-460ns), and the difference is one `Process.clock_gettime(CLOCK_REALTIME)` wall-clock read — which this WSL2 measurement box prices at ~1µs because its Hyper-V clock defeats the vDSO fast path (verified: `CLOCK_REALTIME_COARSE` costs 102ns on the same box). On bare-metal Linux that read is tens of nanoseconds, and the default-time rows land next to the explicit-ms ones. `SecureRandom.uuid` never reads a clock — random v4 is the only thing it does.

The Fiddle fallback (`HYPERUUID_PURE=1`, and any platform without a prebuilt extension) keeps its own diet — a reused thread-local scratch buffer instead of two GC-finalizer-registering mallocs per call, zero-copy `String` passes for read-only inputs, an unsynchronized fast path past the load mutex — landing at 1.27x slower than `SecureRandom.uuid` for v4 (was 1.30x before the diet, from a worse baseline run) with the same structural story as before: `Fiddle`'s interpreted marshalling is the floor, and the batch doors are how you amortize it.

Batch generation still amortizes per-call cost on both backends — one native call for the whole batch:

| Call | i/s (Magnus backend) |
|---|---:|
| `new_v6` × 1000 (individual) | 731.3 |
| `new_v6_batch(1000)` | 2,655.6 (**3.6x**) |
| `new_v7` × 1000 (individual) | 710.0 |
| `new_v7_batch(1000)` | 2,744.3 (**3.9x**) |

The batch multiplier shrank from 11x to ~3.8x for the best reason available: the individual calls got 3x faster, so there's less waste left to amortize. If you need v5/v6/v7, need many at once, or need this Ruby service's IDs to agree byte-for-byte with a Go or Python service's, that's what this gem is for — and now it's the fast option too, not just the capable one.

## Install

```sh
gem install hyperuuid
```

Published to [RubyGems.org](https://rubygems.org/gems/hyperuuid) as real precompiled
"platform gems" — `bundle`/`gem install` auto-selects the matching one for
linux-x64/arm64 or osx-x64/arm64 (the compiled Magnus native extension, `backend: :native`),
falling back automatically to the universal `ruby`-platform gem (pure Fiddle, zero compile,
bundles all 6 platforms' native libs) everywhere else — Windows included, since Magnus
doesn't target it. No extra configuration needed either way.

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.
