# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/hyperuuid.svg)](https://crates.io/crates/hyperuuid)

**A high-performance, RFC 9562-compliant UUID generator for Rust. Benchmarked head-to-head against the `uuid` crate below: about 2x faster on v5 and v7 generation, 5-6x faster at reading a timestamp back out, level on v4 and v6, with a real batch API `uuid` doesn't have at all.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation. Genuinely `#![no_std]` under `default-features = false` (see [below](#no_std)) — compiler-enforced against a real bare-metal target, not just a `no_std`-friendly dependency set (`getrandom`, `sha1`, both `default-features = false`) — zero unsafe in the public API, and empirically zero-allocation across the whole public API, batch calls included — not just claimed, asserted by a real counting-allocator test (`tests/allocation_free.rs`).

```rust
use hyperuuid::{v4, v5, v6, v7};

let id = v4::new_v4()?;
let id2 = v5::new_v5(v5::namespace::DNS, b"example.com");
let id3 = v6::new_v6(unix_millis)?;
let id4 = v7::now_v7()?;

let ts = v7::unix_millis(&id4); // recover the embedded timestamp
let sql_ordered = v7::to_sql_order(&id4); // byte order SQL Server's uniqueidentifier needs to sort by creation order
let sql_ordered6 = v6::to_sql_order(&id3); // same, for a version 6 UUID

// The same Option<Timestamp> shape the `uuid` crate's own get_timestamp() returns — works on
// any Uuid, no need to already know (or separately check) the version:
let ts2: Option<hyperuuid::Timestamp> = hyperuuid::get_timestamp(&id4);
let id5 = v7::new_v7_at(ts2.unwrap())?; // pulls the millis back off Timestamp, same as new_v7

// One random-bytes fetch, one counter reservation for the whole batch:
let mut out = vec![0u8; 1000 * 16];
v7::new_v7_batch(unix_millis, 1000, &mut out)?;
```

`v5::namespace::{DNS, URL, OID, X500}` are RFC 9562 Section 6.6's well-known namespaces. `v6::unix_millis`/`v7::unix_millis` recover the embedded UTC timestamp from a version 6 or 7 UUID as a plain `u64` millisecond count; [`get_timestamp`]/[`Timestamp`] wrap that same value into the `uuid` crate's own `Option<Timestamp>` shape (`None` for any other version), and [`v6::new_v6_at`]/[`v7::new_v7_at`] accept one straight back — both are thin, `#[inline]`-eligible pass-throughs over `unix_millis`/`new_v6`/`new_v7`, not a second implementation, so they carry no measurable overhead beyond those. `v7::to_sql_order`/`v7::to_rfc_order` convert a version 7 UUID to and from the byte order SQL Server's `uniqueidentifier` needs on the wire to sort by creation order — the same permutation this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid)/[Svartalfheim](https://github.com/NorseArchitecture/Svartalfheim) already use for C#, computed once here and verified against the real `System.Data.SqlTypes.SqlGuid` comparator. `v6::to_sql_order`/`v6::to_rfc_order` do the same for version 6 — a much simpler whole-byte-group relocation, since v6 has no counter to repack at the bit level, just its 60-bit timestamp; note that same-millisecond v6 UUIDs aren't guaranteed to sort in creation order even after this conversion, since `clock_seq`/`node` are random per call rather than a counter (a pre-existing RFC 9562 v6 limitation, not something this introduces). This crate's own test suite verifies that sort behavior against a comparator replicating `SqlGuid`'s documented byte order. `new_v6_batch`/`new_v7_batch` generate `count` UUIDs into a caller-owned `&mut [u8]` sharing one timestamp capture, one counter reservation, and one `getrandom` call, instead of `count` of each. That entropy is drawn into the front of the caller's own buffer and moved out to each item's octets as the batch is written backwards, so there is no scratch buffer to allocate — not on the heap and not on the stack — and these are allocation-free like everything else here.

## Why not the `uuid` crate?

[`uuid`](https://docs.rs/uuid) is the de facto standard and it's a good crate — v4/v5/v6/v7 are all real, all correct, all there under feature flags. This isn't a "the standard crate is bad" pitch. The real differences, measured head-to-head on the same machine in the same benchmark run (`cargo bench` — see below):

1. **No batch API.** `uuid` generates exactly one UUID per call, always. `new_v6_batch`/`new_v7_batch` here share one timestamp capture and one counter reservation across the whole batch — 2.1-3.3x faster than the equivalent loop of individual calls (real numbers below).
2. **v5 and v7 generation are about twice as fast here.** v5: 58.2 ns against `uuid`'s 114.0 ns (~2.0x). v7: 33.3 ns against `uuid`'s 75.7 ns (~2.3x). One caveat for fairness on v7: `uuid`'s `Uuid::now_v7` captures the current time itself (an internal clock read) inside the timed region, while this benchmark calls this crate's `new_v7` with a pre-supplied timestamp. What that read costs is a property of the machine, not of either crate: it is cheap on the x64 box these numbers come from, and on a machine with a slow clock it swamps everything else in the call — so the comparison isn't perfectly apples-to-apples and is reported that way rather than smoothed over.
3. **v4 and v6 are a wash.** v4: 41.4 ns here, 40.5 ns for `uuid` — no real win either way. v6: 29.1 ns here, 28.3 ns for `uuid` — level, though the two are doing different work to get there: `Uuid::now_v6` reads the clock and takes its node ID from the caller, with a process-wide counter for the clock sequence, while `new_v6` here takes the timestamp from the caller and draws fresh random bits for both on every call.
4. **Verified zero-allocation, with no exceptions.** `tests/allocation_free.rs` wraps a counting `#[global_allocator]` around 1000 calls to each of v4/v5/v6/v7 and both batch functions, and asserts zero heap allocations for all of them. The crate doesn't so much as link `alloc`, which is why it carries the `no-std::no-alloc` category and not merely `no-std`. `uuid`'s docs don't make an allocation claim either way.

The honest trade-off: this crate's public surface is much narrower than `uuid`'s — no `Builder`, no `fmt` customization, no `serde`/`arbitrary`/`zerocopy` integrations, no v1/v3/v8. If you need any of that, or you're not chasing v5/v7 throughput, timestamp extraction or batch generation specifically, `uuid` is the better default and is what most of the Rust ecosystem already expects to interoperate with.

## Benchmarks

`cargo bench` from this directory (Criterion; HTML reports land in `target/criterion/report/index.html`). All numbers below are linux-x64 (an Intel Core i9-11900H), one run, reproducible on your own hardware with the same command.

### vs. the `uuid` crate (single-item)

| Version | This crate | `uuid` crate | Delta |
| --- | ---: | ---: | ---: |
| v4 | 41.4 ns | 40.5 ns | ~0.98x (level) |
| v5 | 58.2 ns | 114.0 ns | **1.96x faster** |
| v6 | 29.1 ns | 28.3 ns | ~0.97x (level)¹ |
| v7 | 33.3 ns | 75.7 ns | **2.27x faster**¹ |

¹ See the methodology caveat above — `uuid`'s v6/v7 capture the clock internally; this crate's benchmarked calls take a pre-supplied timestamp.

### Timestamp extraction vs. the `uuid` crate's `get_timestamp()`

`uuid` has real extraction logic of its own (`Uuid::get_timestamp() -> Option<Timestamp>`, defined for v1/v6/v7), so this is a genuine head-to-head, not a strawman — each call measured against a UUID generated once outside the timed loop, so only the extraction itself is timed:

| Version | This crate's `unix_millis` | `uuid`'s `get_timestamp()` | Delta |
| --- | ---: | ---: | ---: |
| v6 | 1.30 ns | 6.86 ns | **5.28x faster** |
| v7 | 1.03 ns | 6.66 ns | **6.44x faster** |

The two APIs used to return different shapes — this crate hands back a plain `u64` millisecond count, `uuid`'s `Option<Timestamp>` wraps 100ns Gregorian-epoch ticks — but both are doing the same underlying job (bit-shifting the embedded time back out of 16 bytes already in hand), so timing them head-to-head is fair. This crate wins here for the same reason it's competitive on generation: no allocation, no indirection beyond what the bit math itself needs. This crate now also exposes an `Option<Timestamp>`-returning [`get_timestamp`] matching `uuid`'s own shape (see the Quick start example above) — it's a thin pass-through over the exact `unix_millis` calls benchmarked here, so these numbers describe its cost too, not a separate, unmeasured path.

### Batch generation vs. an equivalent loop (this crate only — `uuid` has no batch API)

| Version | 1000 individual calls | `*_batch(1000)` | Speedup |
| --- | ---: | ---: | ---: |
| v6 | 26.8 µs | 12.5 µs | **2.1x** |
| v7 | 34.0 µs | 10.4 µs | **3.3x** |

Allocation-free claim: `cargo test --release --test allocation_free`.

## WebAssembly

This crate ships source only (crates.io has no compiled-artifact concept), so there's
nothing to package per wasm target the way the compiled-binary bindings in this repo need —
whether and how to target wasm is entirely your call as the consumer, via ordinary
`cargo build --target wasm32-...`. The one thing worth knowing before you do: this crate's
only source of entropy is [`getrandom`](https://docs.rs/getrandom), and `getrandom`'s wasm
support is target-specific, not universal —

- `wasm32-unknown-emscripten` and `wasm32-wasip1`/`wasip2` work with zero extra
  configuration (real OS-level randomness syscalls exist under both).
- `wasm32-unknown-unknown` — the target `wasm-pack`/`wasm-bindgen` workflows typically use,
  since it has no OS underneath at all — does **not** work out of the box. Add
  `getrandom = { version = "0.4", features = ["wasm_js"] }` to *your own* `Cargo.toml` to
  pull in the `Crypto.getRandomValues` JS backend. This crate deliberately doesn't enable
  that feature itself — `getrandom`'s own guidance is that libraries shouldn't, since it
  bloats every consumer's `Cargo.lock` and can break non-web wasm targets that happen to
  share this dependency — so it's a decision left where it belongs, on your side.

One wasm build of this crate is not left to the consumer, because four bindings in this repo
ship it: the `cdylib` for `wasm32-wasip1`, built from inside this directory so that
`.cargo/config.toml` applies —

```sh
cargo cdylib --target wasm32-wasip1
# rust/target/wasm32-wasip1/release/hyperuuid.wasm
```

That config adds two linker flags for this target only, `--export=malloc` and
`--export=free`, so the module's exports are the twelve `uuid_*` functions and
`hyperuuid_version` from `ffi.rs` plus wasi-libc's allocator. A wasm host cannot hand this library a pointer into its own
memory, so every embedder — GraalWasm inside the Java binding, wasmtime inside the Ruby,
Python and Go ones — asks the guest for the buffer it will fill and reads the result back
out of the exported `memory`. The exported allocator is what makes that safe: dlmalloc
claims the tail of the initial linear memory on its first use, so a host-chosen offset past
the data segments is not free, and a batch written there was observed corrupted by the next
allocation. The module imports five `wasi_snapshot_preview1` functions (`random_get`, plus
wasi-libc's `environ_*`, `fd_write` and `proc_exit`) and nothing else; there is no clock,
because `now_v7` is compiled out on `wasm32` and every other door takes the host's
timestamp. `ffi.rs` itself is untouched by any of this; on every native target the C ABI is
still exactly those thirteen exports.

The C# binding's Blazor WebAssembly support takes a second wasm build, the
`wasm32-unknown-emscripten` static library its NuGet package links into the app:
`cargo wasm-staticlib`, which leaves Rust's standard library out of it (the `staticlib`
feature supplies the panic handler in its place), so it can be linked into one app beside
HyperCast's.

## `no_std`

The `no-std` and `no-std::no-alloc` categories this crate publishes are compiler-enforced, not
asserted: the library is `#![no_std]` whenever its default-on `std` feature is off, and it
never links `alloc` in any configuration — there is no `extern crate alloc` in the crate, so a
consumer with no heap at all is one this crate can serve.

```sh
cargo add hyperuuid --no-default-features
```

Default-on rather than unconditional because the shared library every other binding in this
repo dlopens is built from this crate too (`cargo cdylib`, below), and a final linked artifact
needs a `#[panic_handler]`, which only std supplies. That library is not one of the manifest's
crate types, so cargo never builds it for a consumer, and `default-features = false` yields a
real `no_std` rlib on every target: your own machine, `wasm32-unknown-unknown`, and bare metal.
CI builds a `default-features = false` consumer for the first two on every run, and checks the
third:

```sh
cargo check --no-default-features --target thumbv7em-none-eabi
```

On a target with no operating system, two things are left to you, both of them things such a
consumer supplies anyway:

- **An entropy backend.** Off std there's no OS for `getrandom` to read from, and it stops
  compiling rather than guessing. Build with `RUSTFLAGS='--cfg getrandom_backend="custom"'`
  and export a `__getrandom_v03_custom` symbol — see
  [`getrandom`'s custom-backend docs](https://docs.rs/getrandom/0.4/getrandom/#custom-backend).
  Left on your side for the same reason the `wasm_js` backend above is.
- **A clock, where you need one.** `v7::now_v7` is the only API that reads the system clock,
  and it's compiled out without `std` exactly as it already is on `wasm32`. Pass your own
  timestamp to `v7::new_v7`/`v6::new_v6` instead; every other function in the crate is
  already timestamp-in, bytes-out.

## No panics

No input makes the public API panic. Every failure is a `Result`, or a return code through the C
exports, where a panic would abort the host process or trap its wasm instance. That is proven at
link time, not just tested: under the `no-panic` feature every generation, parsing and
conversion function, `v7::now_v7`, and every C export carries
[`#[no_panic]`](https://docs.rs/no-panic), the same check `ryu` and `itoa` use, and CI links a
release binary that calls each one, on Linux and on Windows. A panic path the optimizer cannot
remove fails that link and names the function:

```sh
cargo build --release --example no_panic --features no-panic
```

The proof needs optimization, so build it with `--release`. A debug build fails the link for
most of the functions, because the checks the optimizer would have proven dead are still in.

`v7::now_v7` reads the clock with `clock_gettime` on Unix rather than through std, whose
`SystemTime::now` unwraps that call; an unreadable clock is `TimestampOutOfRange` instead. On
Windows std's clock read cannot fail, so it is used as is.

## Optional native-extension features

This crate also carries the native-extension entry points for this repo's Python, Ruby, and
PHP bindings — `python` (PyO3), `ruby` (Magnus), and `php` (ext-php-rs, a benchmark spike,
not the shipped PHP backend) — instead of each living in a satellite crate that
path-depends back on this one. All three are `optional`, off by default, and additive: a
plain `cargo build` (or a `Cargo.toml` with no `features` line) pulls in nothing beyond
`getrandom`/`sha1`, unchanged from the dependency set above. Only one is ever enabled per
build invocation — each generates a different C entry point under the same crate, not
meant to coexist in one binary:

```sh
cargo cdylib                    # -> the plain shared library every FFI binding loads
cargo cdylib --features python  # -> PyInit__native (normally via maturin in python/)
cargo ruby-ext                  # -> Init_hyperuuid_native, in target/ruby/release/ (ruby/'s `rake native:dev` runs this and stages the result)
cargo php-ext                   # -> get_module, in target/php/release/
```

`cargo cdylib` is an alias in `.cargo/config.toml` for `cargo rustc --release --crate-type
cdylib`. The manifest declares only the rlib, so a plain `cargo build` produces no shared
library; the crate type is named per invocation, which is what keeps it out of every
consumer's build.

Each produces `libhyperuuid.{so,dylib}` (`hyperuuid.dll` on Windows), under `target/release/` for the raw `--features` form and under its own directory for the aliases — the
interpreter-specific loading/staging (module naming, `.pyd`/`.bundle` renaming, etc.) is this
repo's Python/Ruby/PHP packages' job, not this crate's; verified to work from the published
crate itself, not just an in-repo checkout — `cargo rustc --release --crate-type cdylib
--features <one> --manifest-path` against the packaged crate (`cargo package`'s own
output) produces the same three symbols above.

**Local dev trap worth knowing:** "each produces `target/release/libhyperuuid.so`" means
*the same file* — so a `--features python` build (or a `maturin build` in `python/`, which
is one) silently replaces the plain cdylib that every other binding's dev loop loads. The
extension build still exports the `uuid_*` symbols, but it also carries undefined
interpreter symbols that only resolve inside a CPython (or Ruby, or PHP) process, so the
next `./gradlew test` or `dotnet test` fails at native load with something unhelpful about a
missing symbol. Nothing is broken; a plain `cargo cdylib` puts it back. The `cargo ruby-ext` and
`cargo php-ext` aliases in `.cargo/config.toml` avoid it by building into `target/ruby/` and
`target/php/`, and `python/.cargo/config.toml` does the same for maturin (`python/target/`).
CI never hits this — each leg builds in its own job.

## Verifying provenance

The published `.crate` carries a GitHub build-provenance attestation, but not one signed by
this repo directly — `release.yml`'s `pack-crates` job hands off to a reusable workflow
(`hyper-publish-crate.yml`) that physically lives in `SkunkWerkx/.github`, and that's the
identity Fulcio records as the signer. `--repo` alone isn't enough; add `--signer-repo`,
or use `--owner` in place of both:

```sh
curl -LO https://static.crates.io/crates/hyperuuid/hyperuuid-X.Y.Z.crate
gh attestation verify hyperuuid-X.Y.Z.crate \
  --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
# or: gh attestation verify hyperuuid-X.Y.Z.crate --owner SkunkWerkx
```

Get the signer-repo wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`,
which reads like a bad signature but is only an identity mismatch — see
[csharp/README.md's provenance section](../csharp/README.md#native-binary-provenance) for the
full breakdown of which artifacts in this project are signed from which repo and why.

## Install

```sh
cargo add hyperuuid
```

Published to [crates.io](https://crates.io/crates/hyperuuid). Proven by CI building and testing this crate fresh on 5 real-hardware platform legs, on Intel macOS under Rosetta, and in Alpine containers for musl, plus the full `cargo test`/`cargo bench` suite before every release (`.github/workflows/ci.yml`); `release.yml` doesn't rebuild or retest anything itself — it just finds that already-green run for the tagged commit and republishes what it produced.

## License

[MIT](LICENSE)
