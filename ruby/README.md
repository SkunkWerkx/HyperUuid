# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![RubyGems](https://img.shields.io/gem/v/hyperuuid.svg)](https://rubygems.org/gems/hyperuuid)

**Ruby's own stdlib stops at `SecureRandom.uuid` — random v4, full stop. No v5, no v6, no v7. This gem is the whole RFC, with zero gem dependency beyond `Fiddle` (which ships with every Ruby install) — and it's faster than `SecureRandom.uuid` too.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, with
three backends sharing one public surface. The fast path is a native extension built with
[Magnus](https://github.com/matsadler/magnus) — the Rust core linked directly into the Ruby
VM, auto-selected when loadable — which redefines the low-level `Runtime` methods in place
on require; everything above them (`Uuid`, the module doors, batch slicing) is shared
byte-for-byte between backends. The universal fallback calls the native `libhyperuuid`
shared library via [`Fiddle`](https://docs.ruby-lang.org/en/master/Fiddle.html) —
dlopen/dlsym plus a raw C-ABI call, no runtime bridge, nothing to compile on
`bundle install`. Set `HYPERUUID_PURE=1` to force the Fiddle backend;
`HyperUuid::BACKEND` reports which one is live. Bundles a native build for every supported
platform (linux/darwin/windows × x64/arm64) and picks the right one at runtime. A third
backend runs the same core as a WebAssembly module inside the
[`wasmtime`](https://rubygems.org/gems/wasmtime) gem, for any platform with no native build
at all — see [WebAssembly (wasmtime)](#webassembly-wasmtime).

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

## Bulk generation into bytes

`new_v6_batch_bytes` and `new_v7_batch_bytes` return the batch as one binary `String` of raw RFC 9562-ordered bytes — 16 per UUID — instead of an Array of `Uuid` objects:

```ruby
bytes = HyperUuid.new_v7_batch_bytes(1000)
first = bytes[0, 16]        # ready for a BINARY(16) bind parameter
```

**About 15x faster than `new_v7_batch`** for a 1000-UUID batch (24 µs versus 370 µs). The native call is identical — the difference is that `new_v7_batch` then allocates 1000 `Uuid` objects and 1000 String slices on top of it. This hands back the bytes the native core already produced, untouched.

The catch, and it inverts the advice: **if you need `Uuid` objects, keep using `new_v7_batch`.** Slicing these bytes into objects yourself just relocates the identical allocations into your own code, and measures no better — sometimes worse. Reach for the byte form only when bytes are the destination: a bind parameter, a wire format, a bulk load.

Slice it with `bytes[i * 16, 16]` — which is exactly what `new_v7_batch` does internally.

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

## WebAssembly (wasmtime)

The Rust core also ships inside this gem as a `wasm32-wasip1` module
(`lib/hyperuuid/native/wasm32-wasip1/hyperuuid.wasm`), and the
[`wasmtime`](https://rubygems.org/gems/wasmtime) gem can run it in-process. This is the
inverse of ruby.wasm — not Ruby inside a wasm sandbox, but a wasm module inside Ruby — and
it is the one backend that needs no shared library for the platform it runs on: no
`dlopen`, no Magnus extension, nothing compiled against this Ruby's ABI. Everything above
`Runtime` (`Uuid`, the module doors, batch slicing) is the same code the other two backends
run, and `spec/wasm_backend_spec.rb` pins that the outputs agree with the Fiddle backend
byte for byte.

`wasmtime` is deliberately **not** a dependency of this gem; a consumer who wants this path
installs it:

```sh
gem install wasmtime
HYPERUUID_WASM=1 ruby -rhyperuuid -e 'p HyperUuid::BACKEND'   # => :wasm
```

`HYPERUUID_WASM=1` forces the backend (and raises a `LoadError` naming the gem if it is
missing). Without it, the wasm backend is only ever chosen automatically when there is no
native library for this platform at all — no Magnus extension and no `libhyperuuid` for the
RID — and `wasmtime` happens to be installed. No supported platform's behavior changes just
because this backend exists.

Two things are different under the sandbox, both by necessity. A wasm guest only sees its own
linear memory, so every buffer the core fills comes from the module's own exported `malloc`
(the same wasi-libc allocator Rust's std uses on that target) and is read back with
`Memory#read` — using the guest's allocator rather than a host-picked offset is what keeps a
batch from being clobbered by the guest's next allocation. And a `Wasmtime::Store` is
single-threaded, so every call is serialized under one Mutex around one shared instance,
which is also what keeps the core's v7 counter (it lives inside the instance) monotonic
across threads and batches, exactly as the one dlopen'd library does natively.

Measured, same box as the benchmarks above (Ruby 4.0.6, linux-arm64, wasmtime 47.0.3):

| Call | wasmtime | native (Magnus) |
|---|---:|---:|
| `uuid_new_v7`, single, per call | 867 ns | ~450 ns |
| `uuid_new_v7_batch(1000)`, per call | 40.6 µs (50.6 µs with the 16 KB read back into Ruby) | 24 µs |

So roughly 2x the native cost per call, and the batch doors amortize it the same way they do
for Fiddle. The guest's own work is not where the time goes — the identical module runs at
14 µs per thousand under a JIT-compiled host — it is the crossing, and Ruby's is one of the
cheaper ones.

## Verifying provenance

Every gem RubyGems.org serves — the universal fallback and each of the six precompiled
platform gems — carries its own GitHub build-provenance attestation, signed directly by
this repo's own `release.yml` (the `rubygems-publish` job attests `ruby/pkg/*.gem` right
before the push), so plain `--repo` verifies any of them:

```sh
gem fetch hyperuuid -v X.Y.Z --platform <platform>   # or omit --platform for the universal gem
gh attestation verify hyperuuid-X.Y.Z-<platform>.gem --repo SkunkWerkx/HyperUuid
```

That's the release's second layer of checking, not the only one: before any gem gets built,
the same job verifies all ten native binaries it packs (six FFI libs, four Magnus
extensions) against *their own* attestations — those are signed from `SkunkWerkx/.github`
by `hyper-build-native.yml`, so that check needs `--signer-repo SkunkWerkx/.github` added —
and refuses to proceed on an unverified one. RubyGems.org has no unpublish and no
duplicate-version overwrite, so this all happens while a bad artifact is still reversible.
The release run's job summary then re-fetches every gem from the CDN and records
attested-vs-served digests, turning "rubygems.org stores an upload verbatim" into a
per-release measurement rather than an assumption — see
[csharp/README.md's provenance section](../csharp/README.md#native-binary-provenance) for
more on why `--signer-repo` is needed for some artifacts here and not others.

## Install

```sh
gem install hyperuuid
```

Published to [RubyGems.org](https://rubygems.org/gems/hyperuuid) as real precompiled
"platform gems" — `bundle`/`gem install` auto-selects the matching one for
linux-x64/arm64, osx-x64/arm64, x64-mingw-ucrt or aarch64-mingw-ucrt (the compiled Magnus
native extension, `backend: :native`), falling back automatically to the universal
`ruby`-platform gem (pure Fiddle, zero compile, bundles all 6 platforms' native libs)
everywhere else. No extra configuration needed either way.

Selection has **two** axes here, unlike every other binding in this repo. A Magnus extension
is bound to one Ruby minor ABI — there's no `abi3` equivalent to collapse the version axis the
way [the Python binding's](../python/) wheels do — so each platform gem is a "fat" gem
carrying one compiled extension per supported Ruby, under `lib/hyperuuid/<minor>/`, and picks
one at `require` time:

| Ruby | linux-x64/arm64, osx-x64/arm64, x64-mingw-ucrt, aarch64-mingw-ucrt | anywhere else (musl/Alpine, …) |
| --- | --- | --- |
| 4.0 (primary) | Magnus, `backend: :native` | Fiddle |
| 3.4 (floor, until its EOL 2028-03-31) | Magnus, `backend: :native` | Fiddle |
| 3.2 / 3.3 | Fiddle | Fiddle |

The platform gems declare `required_ruby_version >= 3.4, < 4.1` precisely so RubyGems
*declines* them outside that range and resolves the universal gem instead — a wrong-ABI
extension must never be installed in the first place. On Windows it would at least fail to
load cleanly (the extension imports `<arch>-ucrt-ruby<minor>.dll` by name —
`x64-ucrt-ruby400.dll` on x64, `aarch64-ucrt-ruby400.dll` on ARM), but Linux extensions
don't link libruby at all, so one can load successfully against the wrong ABI and misbehave
later. When 3.4 goes EOL it simply leaves the matrix and its users fall back to Fiddle, which
is exactly what the fallback is for.

An earlier edition of this section said the fallback covered "Windows included, since Magnus
doesn't target it." That was wrong in both halves. MinGW is the *only* Windows flavour
`rb-sys` targets — its own `data/toolchains.json` maps `x64-mingw-ucrt` to the
`x86_64-pc-windows-gnu` Rust target, `supported: true`; the target it has no support for is
`x86_64-pc-windows-msvc`. And Windows is where the Fiddle fallback cost the most: measured
on win-x64, Ruby 3.4, the Magnus backend does `new_v4` in 406ns against Fiddle's 2407ns
(**5.9x**) and `new_v7` in 595ns against 2759ns (**4.6x**) — a far wider gap than any Linux
or macOS leg shows.

Windows-on-ARM is no longer the exception it was. `rb-sys` maps `aarch64-mingw-ucrt` to the
`aarch64-pc-windows-gnullvm` Rust target (`supported: true`), and RubyInstaller ships an
`aarch64-mingw-ucrt` build of both ABIs in the table above, so win-arm64 now gets the same fat
platform gem as every other leg. Measured on real Windows-on-ARM hardware, Ruby 4.0.6, the
Magnus backend does `new_v4` in 416ns against Fiddle's 2299ns (**5.5x**) and `new_v7` in 621ns
against 2474ns (**4.0x**) — the same shape the x64 leg shows.

What differs from the x64 build follows from `gnullvm` being LLVM-based where
`x86_64-pc-windows-gnu` is GCC-based. The linker driver is `aarch64-w64-mingw32-clang` from
MSYS2's CLANGARM64 environment — RubyInstaller's own ARM devkit installs it locally, and
`ruby/setup-ruby` installs it on `windows-11-arm` — so there is no `link-self-contained=no`
dance. Two flags are load-bearing:

- **`-l static=unwind`.** rustc emits its `-lunwind` in the linker's *dynamic* section, where
  CLANGARM64 offers both `libunwind.a` and `libunwind.dll.a` and lld prefers the latter. Left
  alone, the extension acquires a runtime dependency on `libunwind.dll` — a file no consumer
  of the gem would have.
- **`BINDGEN_EXTRA_CLANG_ARGS=--target=aarch64-w64-mingw32`.** bindgen hands clang the *Rust*
  target triple when cargo cross-compiles, and clang rejects `aarch64-pc-windows-gnullvm`
  outright (`version 'llvm' in target triple ... is invalid`), then fails behind that on a
  missing `stdalign.h` it never reached its own resource dir to find. `aarch64-w64-mingw32`
  is the same ABI spelled the way clang accepts, exactly as x64 spells its gnu target
  `x86_64-w64-mingw32`.

That second one was briefly documented here as *unnecessary*, on the strength of a local
build where it genuinely was: that build used a `gnullvm` **host** toolchain, so host equals
target, cargo was not cross-compiling, and bindgen injected no `--target` at all. CI's host is
MSVC, so it does. A local build says nothing about that step unless its host triple matches
the runner's.

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.
