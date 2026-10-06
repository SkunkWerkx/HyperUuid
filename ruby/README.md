# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![RubyGems](https://img.shields.io/gem/v/hyperuuid.svg)](https://rubygems.org/gems/hyperuuid)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

**Ruby's own stdlib stops at `SecureRandom.uuid` — random v4, full stop. No v5, no v6, no v7. This gem is the whole RFC, with zero gem dependency beyond `Fiddle` (which ships with every Ruby install) — and it's faster than `SecureRandom.uuid` too.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, with
two backends sharing one public surface. Ruby 3.3 is the floor. The fast path is a native
extension built with [Magnus](https://github.com/matsadler/magnus) — the Rust core linked
directly into the Ruby VM, auto-selected when loadable — which redefines the low-level
`Runtime` methods in place on require; everything above them (`Uuid`, the module doors and
their argument checks, batch slicing) is shared byte-for-byte between backends. The
last-resort fallback, in the universal gem only, calls the native `libhyperuuid` shared
library via [`Fiddle`](https://docs.ruby-lang.org/en/master/Fiddle.html) — dlopen/dlsym plus
a raw C-ABI call, no runtime bridge. `HyperUuid::BACKEND` reports which one is live;
`HYPERUUID_PURE=1` forces Fiddle for testing — see [Backends](#backends).

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

1. **Full RFC 9562 coverage, one gem, zero extra dependency.** v4/v5/v6/v7 plus batch generation plus `Nil`/`Max`, and nothing added to your `Gemfile.lock` beyond `Fiddle` — Ruby's own bundled FFI layer, not a third-party C extension to compile — and on the precompiled platform gems not even that.
2. **No native-extension compile step.** Third-party UUID gems that go beyond v4 are typically pure Ruby or wrap a C extension compiled at install time; this gem ships its fast path as a prebuilt platform-gem extension and its fallback as a `dlopen`ed prebuilt library — either way, the gem itself compiles nothing at install time ([Install](#install) has the one caveat, which is Fiddle's own).
3. **Batch generation.** `new_v7_batch(1000)` shares one timestamp capture, one random-bytes fetch, and one counter reservation across the whole batch instead of paying per-item overhead a thousand times over.
4. **Cross-language consistency.** The same Rust core mints v5 namespace UUIDs for Python, Go, C#, and every other binding in this repo — verified in CI to match Python's own `uuid.uuid5` byte-for-byte. If your system isn't Ruby-only, no Ruby-only gem can offer that.

The honest trade-off: this gem is native code, not pure Ruby — a precompiled extension in each platform gem, and on the universal gem a platform-specific `libhyperuuid.so`/`.dylib`/`.dll` loaded through Fiddle — so it runs only where one of those was built. If plain v4 randomness is all you need, `SecureRandom.uuid` is simpler and already in stdlib — that's a completely reasonable choice.

## Bulk generation into bytes

`new_v6_batch_bytes` and `new_v7_batch_bytes` return the batch as one binary `String` of raw RFC 9562-ordered bytes — 16 per UUID — instead of an Array of `Uuid` objects:

```ruby
bytes = HyperUuid.new_v7_batch_bytes(1000)
first = bytes[0, 16]        # ready for a BINARY(16) bind parameter
```

**About 40x faster than `new_v7_batch`** for a 1000-UUID batch (4.9 µs versus 202 µs). The native call is identical — the difference is that `new_v7_batch` then allocates a `Uuid` object and its byte Strings for every item on top of it. This hands back the bytes the native core already produced, untouched.

The catch, and it inverts the advice: **if you need `Uuid` objects, keep using `new_v7_batch`.** Slicing these bytes into objects yourself just relocates the identical allocations into your own code, and measures no better — sometimes worse. Reach for the byte form only when bytes are the destination: a bind parameter, a wire format, a bulk load.

Slice it with `bytes[i * 16, 16]` — which is exactly what `new_v7_batch` does internally.

## Benchmarks

`ruby benchmark/uuid_benchmark.rb` reproduces every table here and the bytes-versus-objects
comparison above. Its first line names the backend, core version and Ruby it measured, so
run it again under `HYPERUUID_PURE=1` for the other backend.

Real numbers, `benchmark-ips` on Ruby 4.0.7, linux-x64 on an Intel Core i9-11900H (`ruby benchmark/uuid_benchmark.rb`) — not claimed, measured. With the Magnus backend (the default wherever the extension loads):

| Call | i/s | vs `SecureRandom.uuid` |
|---|---:|---:|
| `SecureRandom.uuid` | 943,917 | baseline |
| `HyperUuid.new_v4` | 5,236,000 | **5.5x faster** |
| `HyperUuid.new_v6` (explicit ms) | 3,811,000 | **4.0x faster** |
| `HyperUuid.new_v7` (explicit ms) | 3,785,000 | **4.0x faster** |
| `HyperUuid.new_v6` (current time) | 3,677,000 | **3.9x faster** |
| `HyperUuid.new_v7` (current time) | 3,574,000 | **3.8x faster** |
| `HyperUuid.new_v5` | 1,942,000 | 2.1x faster |

A `HyperUuid.new_v4` — real entropy, correct version/variant bits, minted by the shared Rust core — costs less than a fifth of what `SecureRandom.uuid` does, because the Magnus extension is an ordinary native method call with nothing marshalled around it.

The "current time" rows deserve a footnote, because they will not look like this everywhere. Here they land beside the explicit-ms rows: the only difference between the two is one `Process.clock_gettime(CLOCK_REALTIME)` wall-clock read, and on this machine that read is too cheap to see. On a machine with a slow clock it is the whole story — where a virtualized clock defeats the vDSO fast path the same read costs ~1µs, which puts both current-time rows at parity with `SecureRandom.uuid` while the explicit-ms rows stay well ahead of it. `SecureRandom.uuid` never reads a clock — random v4 is the only thing it does. If your clock is slow and you are minting many, read it once and pass the timestamp, or use the batch doors.

The Fiddle fallback (`HYPERUUID_PURE=1`, and any platform without a prebuilt extension) keeps its own diet — a reused thread-local scratch buffer instead of two GC-finalizer-registering mallocs per call, zero-copy `String` passes for read-only inputs, an unsynchronized fast path past the load mutex — landing at 1.16x slower than `SecureRandom.uuid` for v4 (1.24 µs against 1.07 µs in its own run) and about 1.3x slower for v6/v7, with the same structural story as before: `Fiddle`'s interpreted marshalling is the floor, and the batch doors are how you amortize it.

Batch generation still amortizes per-call cost on both backends — one native call for the whole batch:

| Call | i/s (Magnus backend) |
|---|---:|
| `new_v6` × 1000 (individual) | 3,761 |
| `new_v6_batch(1000)` | 4,857 (**1.3x**) |
| `new_v7` × 1000 (individual) | 3,742 |
| `new_v7_batch(1000)` | 4,850 (**1.3x**) |

The multiplier is small on this backend for the best reason available: the individual calls are cheap, so there is little waste left to amortize, and what `new_v7_batch` spends its 206 µs on is building a thousand `Uuid` objects — the byte form above does the same native work in 5 µs. On the Fiddle backend, where each call costs 1.4 µs, the same batch is 6.7x the loop. If you need v5/v6/v7, need many at once, or need this Ruby service's IDs to agree byte-for-byte with a Go or Python service's, that's what this gem is for — and now it's the fast option too, not just the capable one.

## Backends

| `HyperUuid::BACKEND` | What runs | Chosen when |
|---|---|---|
| `:native` | the core linked into a Magnus extension | a precompiled platform gem is installed — every one carries an extension for each Ruby it installs on; see [Install](#install) — or the `hyperuuid-wasm` gem is linked into a ruby.wasm interpreter; see [Ruby in the browser](#ruby-in-the-browser) |
| `:fiddle` | `libhyperuuid` for this platform, `dlopen`ed through Fiddle | no extension loads: the universal gem, on a Ruby or platform no platform gem covers; also the answer when nothing loads at all |

Selection happens once, at `require`, in that order.

`HYPERUUID_PURE` forces `:fiddle`, even where an extension would load. It is a testing and
diagnostic switch — CI runs the whole suite through it, and it is how to rule an extension
problem in or out — not a setting a deployment needs. It is read for presence, not value: set
to anything at all, `0` and the empty string included, it forces Fiddle. A forced backend that
turns out to have nothing to load does not fall through to another one: the first call raises,
and `HyperUuid.available?` answers `false`. That is what happens inside a platform gem, which
carries no Fiddle library: the `LoadError` names the universal gem
(`gem install hyperuuid --platform ruby`, or Bundler's `force_ruby_platform`), where the
Fiddle backend lives.

**Threads.** Every backend is safe to call from any number of threads. The extension runs
under the GVL. Fiddle releases the GVL for the duration of each call, so that is the one
backend where Ruby threads run the core truly in parallel — each through its own scratch
buffer, with the core's v7 counter shared and atomic. `spec/hyperuuid_spec.rb`'s "concurrent
callers" examples run under both.

**Ractors.** Main Ractor only, on every backend: called from another Ractor the doors raise
`Ractor::UnsafeError`.

## Verifying provenance

Every gem RubyGems.org serves — the universal fallback, each of the seven precompiled
platform gems and `hyperuuid-wasm` — carries its own GitHub build-provenance attestation, signed directly by
this repo's own `release.yml` (the `rubygems-publish` job attests `ruby/pkg/*.gem` right
before the push), so plain `--repo` verifies any of them:

```sh
gem fetch hyperuuid -v X.Y.Z --platform <platform>   # or omit --platform for the universal gem
gh attestation verify hyperuuid-X.Y.Z-<platform>.gem --repo SkunkWerkx/HyperUuid
```

That's the release's second layer of checking, not the only one: before any gem gets built,
the same job verifies every native binary it packs — the Fiddle libraries, the Magnus
extensions (one per Ruby ABI per platform), the `wasm32-wasip1` archives — against *their own*
attestations — those are signed from `SkunkWerkx/.github` by `hyper-build-native.yml` and
`hyper-build-wasm.yml`, so
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

Nine gems are published per release. This section is about eight of them: seven precompiled Magnus platform gems that
`gem install` and `bundle` auto-select when they match — `x86_64-linux-gnu`,
`aarch64-linux-gnu`, `x86_64-linux-musl`, `aarch64-linux-musl`, `arm64-darwin`,
`x64-mingw-ucrt` and `aarch64-mingw-ucrt` — and one universal `ruby`-platform gem. A platform gem carries its
Magnus extensions and nothing else native: no Fiddle library at all. The universal gem is
the last resort, Fiddle with every platform's native library bundled, and it is what
RubyGems resolves for a Ruby the platform gems do not cover (3.3, or a Ruby newer than the
release, such as 4.1 before a release ships for it) and on a platform no platform gem is
built for — Intel macOS among them, which runs on Fiddle. No extra configuration needed either
way. The ninth, `hyperuuid-wasm`, is only for ruby.wasm; see [Ruby in the browser](#ruby-in-the-browser).

Selection has **two** axes here, unlike every other binding in this repo. A Magnus extension
is bound to one Ruby minor ABI — there's no `abi3` equivalent to collapse the version axis the
way [the Python binding's](../python/) wheels do — so each platform gem is a "fat" gem
carrying one compiled extension per supported Ruby, under `lib/hyperuuid/<minor>/`, and picks
one at `require` time:

| Ruby | Linux (glibc and musl), Apple silicon macOS, Windows — x64 and arm64 | Gem installed |
| --- | --- | --- |
| a newer Ruby than the release covers (4.1+) | Fiddle | universal |
| 4.0 (primary) | Magnus, `BACKEND == :native` | platform |
| 3.4 (until its EOL 2028-03-31) | Magnus, `BACKEND == :native` | platform |
| 3.3 (the floor, until its EOL 2027-03-31) | Fiddle | universal |

What stands behind each cell: CI runs the whole suite through each Magnus extension on every
push — Ruby 3.4 and 4.0 on all seven platform-gem platforms, the two musl ones inside each
Ruby's own Alpine image — and the Fiddle suite on Ruby 4.0 on every one of them, plus on Ruby
3.3 inside Alpine. Intel macOS has no CI leg: its library is cross-built and tested at the
core, and Ruby there runs the universal gem's Fiddle backend over it. Anywhere else — a platform with no native build at
all — the universal gem installs but `HyperUuid.available?` answers `false`, and the first
call raises `HyperUuid::NativePlatform::UnsupportedPlatformError`.

The platform gems declare `required_ruby_version >= 3.4, < 4.1` precisely so RubyGems
*declines* them outside that range and resolves the universal gem instead — a wrong-ABI
extension must never be installed in the first place. On Windows it would at least fail to
load cleanly (the extension imports `<arch>-ucrt-ruby<minor>.dll` by name —
`x64-ucrt-ruby400.dll` on x64, `aarch64-ucrt-ruby400.dll` on ARM), but Linux extensions
don't link libruby at all, so one can load successfully against the wrong ABI and misbehave
later. When 3.4 goes EOL it simply leaves the matrix and its users fall back to Fiddle, which
is exactly what the fallback is for.

**musl.** Alpine has platform gems of its own, `x86_64-linux-musl` and
`aarch64-linux-musl`, whose extensions are built inside each Ruby's official `ruby:*-alpine`
image and need nothing beyond musl's libc. The glibc gems name their libc too
(`x86_64-linux-gnu`, `aarch64-linux-gnu`), which is what makes both `gem install` and
Bundler pick the right one on every supported Ruby: next to a plain `x86_64-linux` gem,
RubyGems before 4.0 resolves that one on Alpine instead, even under
`--platform x86_64-linux-musl`.

**Nothing in this gem is ever compiled, on any platform.** The platform gems depend on nothing
at all. The universal gem depends on `fiddle`, which can be: `fiddle` is a bundled gem on Ruby
4.0 and a default gem on 3.3 and 3.4, and `gem install` is satisfied by the copy Ruby ships.
Bundler resolves the newest `fiddle` on rubygems.org instead, and when that is newer than the
one your Ruby ships — true today on 3.3 and 3.4, not on 4.0 — it builds Fiddle's own C
extension, which takes a compiler and libffi's headers (`apk add build-base libffi-dev` on
Alpine). That only reaches you where the universal gem installs: Ruby 3.3, Intel macOS, a
Ruby newer than the release, or a platform with no platform gem. `bundle install
--prefer-local` makes Bundler use the copy Ruby ships where it can (it did on Ruby 3.4's
Bundler 2.6, not on 3.3's 2.5); otherwise install the compiler and headers. Pinning `fiddle`
to your Ruby's version in the `Gemfile.lock` does not stop the build.

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

## Ruby in the browser

ruby.wasm cannot load an extension at runtime. `rbwasm build` cross-compiles the extension of
every gem in a Gemfile and links them all into the one interpreter it builds, so the browser
gets a gem of its own: `hyperuuid-wasm`, the same library and the same Magnus extension,
prebuilt for `wasm32-wasip1`. List it **instead of** `hyperuuid` in the Gemfile you build the
interpreter from (the two carry the same `lib/` files):

```ruby
source "https://rubygems.org"

gem "hyperuuid-wasm"
gem "js" # JavaScript interop, which a browser app almost always wants

group :development do
  gem "ruby_wasm"
end
```

```sh
bundle install
bundle exec rbwasm build --ruby-version 4.0 -o ruby.wasm
```

Load `ruby.wasm` with [`@ruby/wasm-wasi`](https://www.npmjs.com/package/@ruby/wasm-wasi)
(`DefaultRubyVM` in a browser, `RubyVM.instantiateModule` under Node), then
`require "/bundle/setup"` and `require "hyperuuid"` as anywhere else. `HyperUuid::BACKEND` is
`:native`: every door, the batch and raw-bytes forms, and `new_v6`/`new_v7` with no argument,
which read the clock through WASI.

- **Ruby 3.4 and 4.0**, one archive each, the same minors the platform gems cover, picked by
  `--ruby-version`. Any other minor stops the build with a message naming the ones the gem
  carries. Built and tested against ruby_wasm 2.10.
- **No Rust toolchain.** The gem's `extconf.rb` only hands rbwasm the prebuilt archive, and
  rbwasm downloads its own wasi-sdk. The first `rbwasm build` compiles Ruby itself and takes
  15–20 minutes; later builds reuse it.
- **Static linking only,** into ruby.wasm's default `wasm32-unknown-wasip1` interpreter.
  rbwasm's dynamic-linking build (a pic target, for the component model) is not supported.
- **Size.** rbwasm packs every gem's files into the interpreter, the archives included, so the
  gem adds about 2 MB to `ruby.wasm` (both archives are gzipped).
- **With HyperCast.** `hypercast-wasm` links into the same interpreter. Every Rust extension
  that carries std defines a few of the same symbols (`rust_eh_personality`, which ruby.wasm's
  own wasi-vfs defines too, rb-sys's `ruby_abi_version`, and one of std's), and both gems
  rename them to names of their own when CI builds the archive, which also fails if a newer
  Rust starts exporting another.

What stands behind it: CI builds each minor's archive from the commit, packs the gem the way
it ships, links it into a fresh interpreter and runs [`wasm-smoke/test.rb`](wasm-smoke/test.rb)
under Node and in headless Chrome (the forge's `hyper-build-wasm.yml`). The published gem is
packed around those attested archives.

## Development

Everything below runs from a checkout, with `rust/` beside `ruby/`; none of it is needed to
use the gem. The Fiddle backend finds the in-repo build on its own when nothing
is staged under `lib/hyperuuid/native/`. The Magnus backend does not — an extension has to be
built for the Ruby you are running and put where `require` looks — and that is what
`rake native:dev` is for.

```sh
cd rust
cargo cdylib                           # libhyperuuid, what the Fiddle backend loads

cd ../ruby
bundle install
bundle exec rake native:dev          # build the Magnus extension for this Ruby and stage it
bundle exec rspec                    # BACKEND == :native
HYPERUUID_PURE=1 bundle exec rspec   # BACKEND == :fiddle
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
  bundle install --quiet --prefer-local
  HYPERUUID_PURE=1 bundle exec rspec'
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

The musl Magnus extension is built the way CI builds it by the forge's
`ruby-magnus-musl/build-magnus-musl.sh`, which runs on any machine with Docker: from the repo
root, `build-magnus-musl.sh hyperuuid linux-musl-x64 4.0 . <dir with the musl libhyperuuid.so> <out-dir>`
compiles it in `ruby:4.0-alpine`, then runs this suite through it on a bare copy of that image.

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
