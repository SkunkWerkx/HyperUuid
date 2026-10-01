# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![RubyGems](https://img.shields.io/gem/v/hyperuuid.svg)](https://rubygems.org/gems/hyperuuid)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

**Ruby's own stdlib stops at `SecureRandom.uuid` — random v4, full stop. No v5, no v6, no v7. This gem is the whole RFC, with zero gem dependency beyond `Fiddle` (which ships with every Ruby install) — and it's faster than `SecureRandom.uuid` too.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, with
three backends sharing one public surface. Ruby 3.3 is the floor. The fast path is a native
extension built with [Magnus](https://github.com/matsadler/magnus) — the Rust core linked
directly into the Ruby VM, auto-selected when loadable — which redefines the low-level
`Runtime` methods in place on require; everything above them (`Uuid`, the module doors and
their argument checks, batch slicing) is shared byte-for-byte between backends. The
universal fallback calls the native `libhyperuuid` shared library via
[`Fiddle`](https://docs.ruby-lang.org/en/master/Fiddle.html) — dlopen/dlsym plus a raw C-ABI
call, no runtime bridge. A third backend runs the same core as a WebAssembly module inside
the [`wasmtime`](https://rubygems.org/gems/wasmtime) gem, for any platform with no native
build at all. `HyperUuid::BACKEND` reports which one is live; `HYPERUUID_PURE=1` forces
Fiddle, `HYPERUUID_WASM=1` the wasm module — see [Backends](#backends).

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

## The doors

| Door | Returns | Takes |
|---|---|---|
| `new_v4` | `Uuid` | — |
| `new_v5(namespace, name)` | `Uuid`, the same one for the same pair | a `Uuid` namespace (`Namespaces::DNS`/`URL`/`OID`/`X500`, or your own) and a `String` name — text is hashed as UTF-8, a binary String as its bytes, and it may be empty |
| `new_v6(time = nil)` `new_v7(time = nil)` | `Uuid` | nothing (now), a `Time`, or an Integer of Unix-epoch milliseconds |
| `new_v6_batch(count, time = nil)` `new_v7_batch(count, time = nil)` | `Array` of `Uuid` | a count, and the same time forms |
| `new_v6_batch_bytes(count, time = nil)` `new_v7_batch_bytes(count, time = nil)` | one binary `String`, 16 bytes per UUID | as the batch doors — see [Bulk generation into bytes](#bulk-generation-into-bytes) |
| `native_version` | the loaded core's `"major.minor.patch"` | — |
| `available?` | `true`/`false`, never raising | — |

Every generating door returns `HyperUuid::Uuid`, a minimal value object — this gem has no
runtime dependency on the `uuid` gem:

| On `Uuid` | |
|---|---|
| `Uuid.parse(text)` | the 8-4-4-4-12 hyphenated form `#to_s` produces, either case, and nothing else — an `ArgumentError` otherwise |
| `Uuid.new(bytes)` | wraps 16 raw RFC 9562-ordered bytes |
| `Uuid::NIL` `Uuid::MAX` | the RFC 9562 §5.9/§5.10 special values |
| `#bytes` `#to_s` `#version` `#variant` | the frozen binary String, the hyphenated text, the version nibble, the variant bits |
| `#timestamp(raise_on_mismatch: true)` | the UTC `Time` embedded in a version 6 or 7 UUID; `nil` instead of an `ArgumentError` for any other version when passed `false` |
| `#to_sql_order` `#from_sql_order` | to and from the byte order SQL Server's `uniqueidentifier` sorts by |
| `==` `eql?` `hash` `<=>` | value equality and byte-order comparison (`Comparable`) |

`#to_sql_order`/`#from_sql_order` convert a version 6 or 7 UUID to and from the byte order SQL
Server's `uniqueidentifier` needs on the wire to sort by creation order (`#to_sql_order`
dispatches on the UUID's own version, matching `#timestamp`'s convention) — computed once in
the native Rust core rather than reimplemented in Ruby, and verified there (and independently
against the real `System.Data.SqlTypes.SqlGuid` comparator in the C# binding's test suite).
Same-millisecond v6 UUIDs aren't guaranteed to sort correctly afterward — v6 has no counter,
so `clock_seq`/`node` (not the timestamp) decide ties, the same pre-existing RFC 9562 v6
limitation plain order already has. `#from_sql_order` figures out which version to invert by
checking a byte position that's provably collision-free between the two (see the method's own
doc comment).

### Errors

Two exceptions are this gem's own, and both carry the same message on every backend:

- `HyperUuid::TimestampOutOfRangeError` — the time cannot be embedded: past version 7's
  48-bit millisecond field, past version 6's 60-bit one, or negative (a `Time` before the Unix
  epoch included).
- `HyperUuid::RandomSourceError` — the operating system's random source failed.

Through 0.3.0 these were reachable only as `HyperUuid::Runtime::TimestampOutOfRangeError` and
`HyperUuid::Runtime::RandomSourceError`; those names remain, as aliases of the same classes.

Everything else is a caller bug and raises Ruby's own error for it, checked once in the shared
doors rather than left to whichever backend is live: a `TypeError` for a time that is not a
`Time`, an Integer or `nil`, for a count that is not an Integer, for a namespace that is not a
`Uuid` or a name that is not a `String`; an `ArgumentError` for a count outside
0..2³² − 1.

### Is the native core there?

`HyperUuid.available?` answers whether a backend actually loaded — probed once, cached, never
raising — for a consumer with a fallback of its own:

```ruby
id = HyperUuid.available? ? HyperUuid.new_v7.to_s : SecureRandom.uuid
```

`HyperUuid.native_version` reads the version out of the loaded core itself (its
`hyperuuid_version` export), so a mismatch against `HyperUuid::VERSION` can be named before
the first UUID is minted. `HyperUuid::BACKEND` says which backend was *selected*, which is not
the same question: it reads `:fiddle` on a platform with no library at all, because that is
the backend whose first call explains what is missing — a `LoadError` naming the path it
looked for, or `HyperUuid::NativePlatform::UnsupportedPlatformError` naming the platform.

## Why not `SecureRandom.uuid`?

`SecureRandom.uuid` only ever gives you a random v4 UUID — Ruby's stdlib has no built-in v5, v6, or v7 at all. If you need more than that, the choice is really "which gem":

1. **Full RFC 9562 coverage, one gem, zero extra dependency.** v4/v5/v6/v7 plus batch generation plus `Nil`/`Max`, and the only thing this gem adds to your `Gemfile.lock` beyond `Fiddle` — which is Ruby's own bundled FFI layer, not a third-party C extension to compile.
2. **No native-extension compile step.** Third-party UUID gems that go beyond v4 are typically pure Ruby or wrap a C extension compiled at install time; this gem ships its fast path as a prebuilt platform-gem extension and its fallback as a `dlopen`ed prebuilt library — either way, the gem itself compiles nothing at install time ([Install](#install) has the one caveat, which is Fiddle's own).
3. **Batch generation.** `new_v7_batch(1000)` shares one timestamp capture, one random-bytes fetch, and one counter reservation across the whole batch instead of paying per-item overhead a thousand times over.
4. **Cross-language consistency.** The same Rust core mints v5 namespace UUIDs for Python, Go, C#, and every other binding in this repo — verified in CI to match Python's own `uuid.uuid5` byte-for-byte. If your system isn't Ruby-only, no Ruby-only gem can offer that.

The honest trade-off: this gem `dlopen`s a native library instead of being pure Ruby, so it needs a platform-specific `libhyperuuid.so`/`.dylib`/`.dll` bundled alongside it. If plain v4 randomness is all you need, `SecureRandom.uuid` is simpler and already in stdlib — that's a completely reasonable choice.

## Bulk generation into bytes

`new_v6_batch_bytes` and `new_v7_batch_bytes` return the batch as one binary `String` of raw RFC 9562-ordered bytes — 16 per UUID — instead of an Array of `Uuid` objects:

```ruby
bytes = HyperUuid.new_v7_batch_bytes(1000)
first = bytes[0, 16]        # ready for a BINARY(16) bind parameter
```

**About 15x faster than `new_v7_batch`** for a 1000-UUID batch (24 µs versus 370 µs). The native call is identical — the difference is that `new_v7_batch` then allocates a `Uuid` object and its byte Strings for every item on top of it. This hands back the bytes the native core already produced, untouched.

The catch, and it inverts the advice: **if you need `Uuid` objects, keep using `new_v7_batch`.** Slicing these bytes into objects yourself just relocates the identical allocations into your own code, and measures no better — sometimes worse. Reach for the byte form only when bytes are the destination: a bind parameter, a wire format, a bulk load.

Slice it with `bytes[i * 16, 16]` — which is exactly what `new_v7_batch` does internally.

## Benchmarks

`ruby benchmark/uuid_benchmark.rb` reproduces every table here and the bytes-versus-objects
comparison above. Its first line names the backend, core version and Ruby it measured, so
run it again under `HYPERUUID_PURE=1` or `HYPERUUID_WASM=1` for the other two backends.

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

## Backends

| `HyperUuid::BACKEND` | What runs | Chosen when |
|---|---|---|
| `:native` | the core linked into a Magnus extension | a precompiled platform gem carries an extension for this Ruby's ABI — see [Install](#install) |
| `:fiddle` | `libhyperuuid` for this platform, `dlopen`ed through Fiddle | no extension loads; also the answer when nothing loads at all |
| `:wasm` | the core as a `wasm32-wasip1` module inside the `wasmtime` gem | no extension and no library for this platform, and `wasmtime` is installed — see [WebAssembly (wasmtime)](#webassembly-wasmtime) |

Selection happens once, at `require`, in that order. Two environment variables override it:

| Variable | Effect |
|---|---|
| `HYPERUUID_WASM` | forces `:wasm`; `require` raises a `LoadError` naming the gem if `wasmtime` is missing |
| `HYPERUUID_PURE` | forces `:fiddle`, even where an extension would load |

Both are read for presence, not value — set to anything at all, `0` and the empty string
included, they force their backend — and `HYPERUUID_WASM` wins when both are set. A forced
backend that turns out to have nothing to load does not fall through to another one: the
first call raises, and `HyperUuid.available?` answers `false`.

**Threads.** Every backend is safe to call from any number of threads. The extension runs
under the GVL. Fiddle releases the GVL for the duration of each call, so that is the one
backend where Ruby threads run the core truly in parallel — each through its own scratch
buffer, with the core's v7 counter shared and atomic. The wasm backend serializes every call
on one Mutex around one instance. `spec/hyperuuid_spec.rb`'s "concurrent callers" examples
run under all three.

**Ractors.** Main Ractor only, on every backend: called from another Ractor the doors raise
`Ractor::UnsafeError` (`Ractor::IsolationError` under wasm).

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
the same job verifies every native binary it packs — the Fiddle libraries, the Magnus
extensions (one per Ruby ABI per platform) and the wasm module — against *their own*
attestations — those are signed from `SkunkWerkx/.github` by `hyper-build-native.yml`, so
that check needs `--signer-repo SkunkWerkx/.github` added — and refuses to proceed on an
unverified one. RubyGems.org has no unpublish and no
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

Seven gems are published per release: one universal `ruby`-platform gem (Fiddle, with every
platform's native library and the wasm module bundled) plus six precompiled Magnus platform
gems (`x86_64-linux`, `aarch64-linux`, `x86_64-darwin`, `arm64-darwin`, `x64-mingw-ucrt`,
`aarch64-mingw-ucrt`) that `gem install` and `bundle` auto-select when they match. A platform
gem carries the same libraries and module beside its extensions, so the Fiddle and wasm
backends are still there behind `HYPERUUID_PURE` and `HYPERUUID_WASM`. No extra configuration
needed either way.

Selection has **two** axes here, unlike every other binding in this repo. A Magnus extension
is bound to one Ruby minor ABI — there's no `abi3` equivalent to collapse the version axis the
way [the Python binding's](../python/) wheels do — so each platform gem is a "fat" gem
carrying one compiled extension per supported Ruby, under `lib/hyperuuid/<minor>/`, and picks
one at `require` time:

| Ruby | glibc Linux, macOS, Windows — x64 and arm64 | musl Linux (Alpine) — x64 and arm64 | anywhere else |
| --- | --- | --- | --- |
| 4.0 (primary) | Magnus, `BACKEND == :native` | Fiddle | wasm, if `wasmtime` is installed |
| 3.4 (until its EOL 2028-03-31) | Magnus, `BACKEND == :native` | Fiddle | wasm, if `wasmtime` is installed |
| 3.3 (the floor) | Fiddle | Fiddle | wasm, if `wasmtime` is installed |

What stands behind each cell: CI runs the whole suite for the first column on every push —
Magnus on Ruby 3.4 and 4.0, Fiddle and wasm on 4.0, on all six platforms — and the Fiddle
suite for the musl column inside an Alpine container. The 3.3 row is the same Fiddle code
path, run with the Docker command under [Development](#development) (Ruby 3.3 with the
Fiddle 1.1.2 it ships) rather than on every push. The last column — wasm chosen
automatically because nothing native exists — has no leg of its own: CI runs the wasm
backend's suite forced, on platforms that have native builds too.

The platform gems declare `required_ruby_version >= 3.4, < 4.1` precisely so RubyGems
*declines* them outside that range and resolves the universal gem instead — a wrong-ABI
extension must never be installed in the first place. On Windows it would at least fail to
load cleanly (the extension imports `<arch>-ucrt-ruby<minor>.dll` by name —
`x64-ucrt-ruby400.dll` on x64, `aarch64-ucrt-ruby400.dll` on ARM), but Linux extensions
don't link libruby at all, so one can load successfully against the wrong ABI and misbehave
later. When 3.4 goes EOL it simply leaves the matrix and its users fall back to Fiddle, which
is exactly what the fallback is for.

**musl.** RubyGems does not tell musl from glibc for these gems: on Alpine, `gem install`
picks the `x86_64-linux` or `aarch64-linux` platform gem exactly as it does on any other
Linux (checked with RubyGems 4.0.20 on `x86_64-linux-musl`). The Magnus extension inside is
linked against glibc and cannot load there — `require` fails cleanly on the missing
`ld-linux` loader — so the gem falls back to Fiddle, which loads the musl build of the core
(`native/linux-musl-x64` or `native/linux-musl-arm64`, chosen from `RUBY_PLATFORM`) that
every gem carries. Alpine therefore works as installed, on the Fiddle backend and at Fiddle's
speed. Releases through 0.3.0 carried no musl library: there the fallback found only the
glibc one, and the first call raised `Fiddle::DLError`.

**Nothing in this gem is ever compiled, on any platform.** Its one dependency can be:
`fiddle` is a bundled gem on Ruby 4.0 and a default gem on 3.3 and 3.4, and `gem install` is
satisfied by the copy Ruby ships. Bundler resolves the newest `fiddle` on rubygems.org
instead, and when that is newer than the one your Ruby ships — true today on 3.3 and 3.4, not
on 4.0 — it builds Fiddle's own C extension, which takes a compiler and libffi's headers
(`apk add build-base libffi-dev` on Alpine). Holding `fiddle` at your Ruby's version in the
`Gemfile.lock` avoids that.

Both Windows architectures get a Magnus gem. MinGW is the *only* Windows flavour `rb-sys`
targets (`x64-mingw-ucrt` and `aarch64-mingw-ucrt`, both `supported: true` in its own
`data/toolchains.json`); the one it has no support for is MSVC. Windows is also where the
Fiddle fallback costs the most: measured on win-x64, Ruby 3.4, the Magnus backend does
`new_v4` in 406ns against Fiddle's 2407ns (**5.9x**) and `new_v7` in 595ns against 2759ns
(**4.6x**) — a far wider gap than any Linux or macOS leg shows — and on real Windows-on-ARM
hardware, Ruby 4.0.6, `new_v4` in 416ns against Fiddle's 2299ns (**5.5x**) and `new_v7` in
621ns against 2474ns (**4.0x**), the same shape. Both extensions are built for the `gnullvm`
Rust targets rather than `gnu` — the same mingw-w64/UCRT ABI RubyInstaller's Ruby uses,
linked with LLVM and compiler-rt instead of GCC and a statically-linked libgcc, which is what
keeps the shipped extension small. The build script, and the two flags that are load-bearing
on the ARM leg (a static libunwind, and a clang-spelled `--target` for bindgen), live in the
forge's `ruby-magnus` action (`build-magnus.sh`), shared with every other Hyper* repo.

## Development

Everything below runs from a checkout, with `rust/` beside `ruby/`; none of it is needed to
use the gem. The Fiddle and wasm backends find the in-repo builds on their own when nothing
is staged under `lib/hyperuuid/native/`. The Magnus backend does not — an extension has to be
built for the Ruby you are running and put where `require` looks — and that is what
`rake native:dev` is for.

```sh
cd rust
cargo build --release                           # libhyperuuid, what the Fiddle backend loads
cargo build --release --target wasm32-wasip1    # hyperuuid.wasm, what the wasm backend loads

cd ../ruby
bundle install
bundle exec rake native:dev          # build the Magnus extension for this Ruby and stage it
bundle exec rspec                    # BACKEND == :native
HYPERUUID_PURE=1 bundle exec rspec   # BACKEND == :fiddle
HYPERUUID_WASM=1 bundle exec rspec   # BACKEND == :wasm
bundle exec rake docs:check          # every public object carries a doc comment
ruby benchmark/uuid_benchmark.rb     # prints the backend it measured
```

`rake native:dev` runs `cargo ruby-ext` (an alias in `rust/.cargo/config.toml` that builds the
`ruby` feature into `rust/target/ruby/`, never over the plain library in
`rust/target/release/`) and copies the result to
`lib/hyperuuid/<minor>/hyperuuid_native.<so|bundle>` — the path `lib/hyperuuid.rb` tries
first. The copy is also a rename, and it is the step `cargo ruby-ext` alone does not do: cargo
names its output `libhyperuuid.so`, and Ruby derives the `Init_` function it calls from the
file name it was asked to `require`. Three things worth knowing:

- **Run it again after changing `rust/`.** The staged file is a copy; nothing rebuilds it.
- **One staging per Ruby.** An extension is bound to one Ruby minor, so under a second Ruby
  (`RBENV_VERSION=3.4.11 bundle exec rake native:dev`, say) the task rebuilds for that ABI
  and stages beside the first, each Ruby loading its own.
- **Without it, `bundle exec rspec` still passes — on Fiddle.** The examples under "native
  backend" report themselves pending with `BACKEND=fiddle`; that line, or
  `ruby -Ilib -rhyperuuid -e 'p HyperUuid::BACKEND'`, is how to tell which backend a run
  exercised. Deleting `lib/hyperuuid/<minor>/` goes back to Fiddle.

The task covers Linux and macOS. The Windows extensions need the `gnullvm` targets and linker
flags in the forge's `build-magnus.sh`, which is also what CI uses on every platform.

The musl build has no host to run on outside a container. With the core built for musl at
`rust/target/musl/linux-musl-x64/libhyperuuid.so`, this runs the Fiddle suite against it on
Alpine, with the checkout mounted read-only:

```sh
docker run --rm -v "$PWD/..":/src:ro ruby:4.0-alpine sh -euc '
  mkdir /work && cp -r /src/ruby /work/ruby
  mkdir -p /work/ruby/lib/hyperuuid/native/linux-musl-x64
  cp /src/rust/target/musl/linux-musl-x64/libhyperuuid.so /work/ruby/lib/hyperuuid/native/linux-musl-x64/
  cd /work/ruby && rm -f Gemfile.lock
  BUNDLE_WITHOUT=wasm bundle install --quiet --prefer-local
  HYPERUUID_PURE=1 BUNDLE_WITHOUT=wasm bundle exec rspec'
```

The floor's test is the same container on `ruby:3.3-alpine`. Bundler would compile a newer
Fiddle there (see [Install](#install)), so this one goes around Bundler and runs against the
Fiddle that Ruby 3.3 ships:

```sh
docker run --rm -v "$PWD/..":/src:ro ruby:3.3-alpine sh -euc '
  mkdir /work && cp -r /src/ruby /work/ruby
  mkdir -p /work/ruby/lib/hyperuuid/native/linux-musl-x64
  cp /src/rust/target/musl/linux-musl-x64/libhyperuuid.so /work/ruby/lib/hyperuuid/native/linux-musl-x64/
  cd /work/ruby && rm -f Gemfile Gemfile.lock
  gem install rspec -v "~> 3.13" --no-document --silent
  HYPERUUID_PURE=1 rspec'
```

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
