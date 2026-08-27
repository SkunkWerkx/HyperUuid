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

# One native call, one random-bytes fetch, one counter reservation for the whole batch:
batch = HyperUuid.new_v7_batch(1000)
```

Returns `HyperUuid::Uuid`, a minimal value object (`#bytes`, `#to_s`, `#version`, `#variant`,
comparable/hashable) — this gem has no runtime dependency on the `uuid` gem.
`HyperUuid::Namespaces::DNS`/`URL`/`OID`/`X500` are RFC 9562 Section 6.6's well-known
namespaces. `#timestamp` recovers the embedded UTC `Time` from a version 6 or 7 UUID.
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

## Install

Not yet published to RubyGems.org under a registered `SkunkWerkx`/`buvinghausen` presence —
for now this is proven by CI building and testing the native core plus this gem on real
hardware for every platform leg. Consume via a direct
`gem "hyperuuid", git: "https://github.com/SkunkWerkx/HyperUuid", glob: "ruby/*.gemspec"` in
the meantime.

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.
