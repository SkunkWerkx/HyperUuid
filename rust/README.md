# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/hyperuuid.svg)](https://crates.io/crates/hyperuuid)

**A high-performance, RFC 9562-compliant UUID generator for Rust. Benchmarked head-to-head against the `uuid` crate below: 1.7-2.4x faster on v4, v5 and v7 generation, 9x faster at reading a timestamp back out, with a real batch API `uuid` doesn't have at all.**

RFC 9562 UUID v4 (random), v5 (deterministic) and v7 (time-sortable) generation, plus the version 8 layout SQL Server sorts a v7 by. Genuinely `#![no_std]` under `default-features = false` (see [below](#no_std)) — compiler-enforced against a real bare-metal target, not just a `no_std`-friendly dependency set (`getrandom`, `sha1`, both `default-features = false`, and `chacha20` on Linux and Android) — zero unsafe in the public API, and empirically zero-allocation across the whole public API, batch calls included — not just claimed, asserted by a real counting-allocator test (`tests/allocation_free.rs`).

```rust
use hyperuuid::{v4, v5, v7};

let id = v4::new_v4()?;
let id2 = v5::new_v5(v5::namespace::DNS, b"example.com");
let id4 = v7::now_v7()?;

let ts = v7::unix_millis(&id4); // recover the embedded timestamp
let sql_ordered = v7::to_sql_order(&id4)?; // the version 8 layout SQL Server's uniqueidentifier sorts by creation order

// The same Option<Timestamp> shape the `uuid` crate's own get_timestamp() returns — works on
// any Uuid, no need to already know (or separately check) the version:
let ts2: Option<hyperuuid::Timestamp> = hyperuuid::get_timestamp(&id4);
let id5 = v7::new_v7_at(ts2.unwrap())?; // pulls the millis back off Timestamp, same as new_v7

// Inspection: is this an RFC 9562 UUID of version 7? And the same questions of a value
// still in SQL Server order, read from its permuted bytes in place:
use hyperuuid::{Layout, Variant};
assert!(id4.is_rfc(7) && id4.variant() == Variant::Rfc9562);
assert_eq!(sql_ordered.version_in(Layout::SqlServer), 7);
let ts3 = hyperuuid::get_timestamp_in(&sql_ordered, Layout::SqlServer);

// One random-bytes fetch, one counter reservation for the whole batch (at most
// v7::MAX_BATCH, the 26-bit counter space; always strictly increasing):
let mut out = vec![0u8; 1000 * 16];
v7::new_v7_batch(ts, 1000, &mut out)?;
```

`v5::namespace::{DNS, URL, OID, X500}` are RFC 9562 Section 6.6's well-known namespaces. `v7::unix_millis` recovers the embedded UTC timestamp from a version 7 UUID as a plain `u64` millisecond count; [`get_timestamp`]/[`Timestamp`] wrap that same value into the `uuid` crate's own `Option<Timestamp>` shape (`None` for anything but an RFC 9562 v7 — the variant is checked too, and a v1 or v6 from another generator gets `None` as well), and [`v7::new_v7_at`] accepts one straight back — both are thin, `#[inline]`-eligible pass-throughs over `unix_millis`/`new_v7`, not a second implementation, so they carry no measurable overhead beyond those. `v7::to_sql_order`/`v7::to_rfc_order` convert a version 7 UUID to and from the byte order SQL Server's `uniqueidentifier` needs on the wire to sort by creation order. The converted value is an RFC 9562 version 8, the version §6.11 gives any byte order but network order: an 8 and the RFC variant sit where SQL Server's text form and a .NET `Guid` show them, and the other 122 bits fill the comparator's slots. That is the permutation this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid) uses for C#, with an 8 where SequentialGuid keeps the 7. A v7 and a v8 have the same 122 free bits, so every v7 converts and the conversion back is exact; [`SqlOrderError`] refuses only a value that isn't a v7 going in, or doesn't carry the layout's 8 and variant coming back. [`sql`](https://docs.rs/hyperuuid/latest/hyperuuid/sql/) has the bit layout. This crate's own test suite verifies the sort against a comparator replicating `SqlGuid`'s documented byte order. [`Uuid::version`]/[`Uuid::variant`]/[`Uuid::is_rfc`] inspect a value: `is_rfc(n)` is the one-call guard, true only for the RFC variant with version `n`. [`Uuid::version_in`]/[`Uuid::is_rfc_in`]/[`get_timestamp_in`] and `v7::unix_millis_in` ask the same of a value held in a given [`Layout`]. In `Layout::SqlServer`, they read a value straight from `to_sql_order` (or a `uniqueidentifier` column) without converting it back; there `version_in` answers 7, the version the value was converted from, for the layout's 8 and variant, and 0 for anything else. `new_v7_batch` generates `count` UUIDs into a caller-owned `&mut [u8]` sharing one timestamp capture, one counter reservation, and one entropy draw, instead of `count` of each. A v7 batch takes at most `v7::MAX_BATCH` (2^26) and is always strictly increasing: one that crosses the process-wide counter's wrap stamps the UUIDs from there on a millisecond later ([v7 ordering, precisely](https://github.com/SkunkWerkx/HyperUuid#v7-ordering-precisely)). The draw is one `getrandom` call, except that on Linux and Android a batch of 32 UUIDs or more takes one 32-byte `getrandom` draw and stretches it with a SIMD ChaCha20 keystream, the construction the kernel uses behind `getrandom` itself. That is 2.4x faster on glibc and 3.7x on musl; Windows and wasm, where it was no faster, keep `getrandom` throughout. That entropy is drawn into the front of the caller's own buffer and moved out to each item's octets as the batch is written backwards, so there is no scratch buffer to allocate — not on the heap and not on the stack — and these are allocation-free like everything else here.

`Timestamp::from_unix(seconds, subsec_nanos)` returns `Option<Timestamp>` and
rejects a nanosecond component of one billion or more. Valid sub-millisecond digits
round-trip through `to_unix`; `to_unix_millis` truncates them. Construction from a
millisecond count with `Timestamp::from_unix_millis` remains infallible.

`Uuid` implements `FromStr`; its `ParseUuidError` implements `core::error::Error`, so
`text.parse::<Uuid>()?` composes with standard error handling in ordinary Rust code.

## Binding to SQL Server

Bind `to_sql_order`'s result as bytes, not as a `uuid::Uuid`. tiberius 0.13 sends a `Uuid`, and a
string, in SQL Server's mixed-endian text order, which stores the first three groups
byte-reversed and loses the creation order ([details](https://github.com/SkunkWerkx/HyperUuid#sql-server-ordering)). A `&[u8]` goes in as binary and
lands byte for byte:

```rust
let stored = v7::to_sql_order(&id4)?;
client.execute("INSERT INTO orders (id) VALUES (@P1)", &[&&stored.as_bytes()[..]]).await?;
```

Reading has the same trap: tiberius returns a `uniqueidentifier` as a `Uuid` with those groups
reversed. Select `CAST(id AS binary(16))` and rebuild the value with `Uuid::from_bytes` before
handing it to the `Layout::SqlServer` reads.

## Why not the `uuid` crate?

[`uuid`](https://docs.rs/uuid) is the de facto standard and it's a good crate — v4/v5/v7 are all real, all correct, all there under feature flags, alongside v1, v3, v6 and v8. This isn't a "the standard crate is bad" pitch. The real differences, measured head-to-head on the same machine in the same benchmark run (`cargo bench` — see below):

1. **No batch API.** `uuid` generates exactly one UUID per call, always. `new_v7_batch` here shares one timestamp capture, one counter reservation and one entropy draw across the whole batch — 5.7x faster than the equivalent loop of individual calls (real numbers below).
2. **v4, v5 and v7 generation are about twice as fast here.** v4: 23.4 ns against `uuid`'s 40.6 ns (~1.7x). v5: 49.6 ns against `uuid`'s 97.6 ns (~2.0x). v7: 30.9 ns against `uuid`'s 72.8 ns (~2.4x). On Linux, v4 and v7 draw their random bits from a user-space ChaCha20 pool rather than a `getrandom` call per UUID; it is built like the kernel's own vDSO generator, with fast key erasure, and its memory is wiped in a forked child. Each slot takes a fresh OS key every megabyte, and no key is used for more than a second of wall-clock time: every draw checks the key's age first, which is what keeps a process restored from a snapshot (a cloned virtual machine, AWS Lambda SnapStart, a CRaC or CRIU checkpoint) from replaying bytes its other copies also hold. What that leaves open is a snapshot taken within a second of a key being drawn and restored onto a clock nothing has moved on, where the copies share bytes until the second runs out; the kernel's own generator, which is told about a restore, has no such window. The check is a coarse clock read, 3 to 4 ns. `new_v7` skips it when the timestamp it is handed is the current time, as a `now` door's is: 27.8 ns (`single_item/v7_current_timestamp`), where the tables below, like most of this repository's benchmarks, pass a fixed timestamp from 2022 and pay for the read. Elsewhere every call still goes to `getrandom`. One caveat for fairness on v7: `uuid`'s `Uuid::now_v7` captures the current time itself (an internal clock read) inside the timed region, while this benchmark calls this crate's `new_v7` with a pre-supplied timestamp. What that read costs is a property of the machine, not of either crate: it is cheap on the x64 box these numbers come from, and on a machine with a slow clock it swamps everything else in the call — so the comparison isn't perfectly apples-to-apples and is reported that way rather than smoothed over.
3. **Verified zero-allocation, with no exceptions.** `tests/allocation_free.rs` wraps a counting `#[global_allocator]` around 1000 calls to each of v4/v5/v7 and the batch function, and asserts zero heap allocations for all of them. The crate doesn't so much as link `alloc`, which is why it carries the `no-std::no-alloc` category and not merely `no-std`. `uuid`'s docs don't make an allocation claim either way.

The honest trade-off: this crate's public surface is much narrower than `uuid`'s — no `Builder`, no `fmt` customization, no `serde`/`arbitrary`/`zerocopy` integrations, no v1/v3/v6, and v8 only as the SQL Server layout. If you need any of that, or you're not chasing v5/v7 throughput, timestamp extraction or batch generation specifically, `uuid` is the better default and is what most of the Rust ecosystem already expects to interoperate with.

## Benchmarks

`cargo bench` from this directory (Criterion, which keeps each run's figures under `target/criterion/`). All numbers below are linux-x64 (an Intel Core i9-11900H), one run, reproducible on your own hardware with the same command.

### vs. the `uuid` crate (single-item)

| Version | This crate | `uuid` crate | Delta |
| --- | ---: | ---: | ---: |
| v4 | 23.4 ns | 40.6 ns | **1.73x faster** |
| v5 | 49.6 ns | 97.6 ns | **1.97x faster** |
| v7 | 30.9 ns | 72.8 ns | **2.35x faster**¹ |

¹ See the methodology caveat above — `uuid`'s v7 captures the clock internally; this crate's benchmarked calls take a pre-supplied timestamp.

### Timestamp extraction vs. the `uuid` crate's `get_timestamp()`

`uuid` has real extraction logic of its own (`Uuid::get_timestamp() -> Option<Timestamp>`, defined for v1/v6/v7), so this is a genuine head-to-head, not a strawman — each call measured against a UUID generated once outside the timed loop, so only the extraction itself is timed:

| Version | This crate's `unix_millis` | `uuid`'s `get_timestamp()` | Delta |
| --- | ---: | ---: | ---: |
| v7 | 0.66 ns | 5.94 ns | **8.99x faster** |

The two APIs return different shapes — this crate hands back a plain `u64` millisecond count, `uuid`'s `Option<Timestamp>` wraps 100ns Gregorian-epoch ticks — but both are doing the same underlying job (bit-shifting the embedded time back out of 16 bytes already in hand), so timing them head-to-head is fair. This crate wins here for the same reason it's competitive on generation: no allocation, no indirection beyond what the bit math itself needs. This crate also exposes an `Option<Timestamp>`-returning [`get_timestamp`] matching `uuid`'s own shape (see the Quick start example above) — it's a thin pass-through over the exact `unix_millis` calls benchmarked here, so these numbers describe its cost too, not a separate, unmeasured path.

### Batch generation vs. an equivalent loop (this crate only — `uuid` has no batch API)

| Version | 1000 individual calls | `*_batch(1000)` | Speedup |
| --- | ---: | ---: | ---: |
| v7 | 25.2 µs | 4.40 µs | **5.7x** |

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
  `v7::now_v7` is not compiled for wasm32 — a browser has no OS clock to read it from — so
  pass the time in yourself: `v7::new_v7(js_sys::Date::now() as u64)`. `rust/browser-test/`
  is exactly this setup, and CI runs it in headless Chrome on every PR (`wasm-pack test
  --headless --chrome` there).

A `wasm32-wasip1` build of the crate (`cargo rustc --release --crate-type cdylib --target
wasm32-wasip1`, from inside this directory) imports one `wasi_snapshot_preview1` function,
`random_get`, and nothing else;
there is no clock, because `now_v7` is compiled out on `wasm32` and every other door takes
the host's timestamp. Nothing in this repo ships that module: the Go and Swift bindings link
the crate's `wasm32-wasip1` static archive instead ([WebAssembly](../README.md#webassembly)).
`ffi.rs` itself is untouched by any of this; on every native target the C ABI is still
exactly those thirteen exports.

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

Default-on rather than unconditional for the crates.io consumer, the tests and the native
extensions. The artifacts this repository ships leave it out: the static libraries and the
shared library every FFI binding loads (`cargo cdylib`, below) are all `#![no_std]`, each
bringing the abort-on-panic handler std would otherwise supply — which takes the linux-x64
shared library from 291,432 bytes to 25,944 with the same thirteen exports and the same code
behind them (commit `0b57513`, measured 2026-10-25 with rustc 1.99.1 in the `rust:latest`
image: `cargo rustc --release --crate-type cdylib` with std, `cargo cdylib` without).
That library is not one of the manifest's crate types, so cargo never builds it for a consumer, and `default-features = false` yields a
real `no_std` rlib on every target: your own machine, `wasm32-unknown-unknown`, and bare metal.
CI builds a `default-features = false` consumer for the first two on every run, and checks the
third:

```sh
RUSTFLAGS='--cfg getrandom_backend="custom"' cargo check --no-default-features --target thumbv7em-none-eabi
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
  timestamp to `v7::new_v7` instead; every other function in the crate is
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

To run the same proof from your own crate, turn on fat LTO and a single codegen unit in its
release profile. Cargo ignores a dependency's profile, so a consumer's stock `--release` build
compiles this crate, `getrandom` and `sha1` (and `chacha20` on Linux and Android) separately,
leaves the panic paths across those boundaries in, and fails the link for the generators. It
takes both settings: fat LTO alone, thin LTO, or `codegen-units = 1` alone still fails:

```toml
[dependencies]
hyperuuid = { version = "<version>", features = ["no-panic"] }

[profile.release]
lto = true
codegen-units = 1
```

`v7::now_v7` reads the clock with `clock_gettime` on Unix rather than through std, whose
`SystemTime::now` unwraps that call; an unreadable clock is `TimestampOutOfRange` instead. On
Windows std's clock read cannot fail, so it is used as is.

## Optional native-extension features

This crate also carries the native-extension entry points for this repo's Python, Ruby, and
PHP bindings — `python` (PyO3), `ruby` (Magnus), and `php` (ext-php-rs, a benchmark spike,
not the shipped PHP backend) — instead of each living in a satellite crate that
path-depends back on this one. All three are `optional`, off by default, and additive: a
plain `cargo build` (or a `Cargo.toml` with no `features` line) pulls in nothing beyond
`getrandom`/`sha1` (and `chacha20` on Linux and Android), unchanged from the dependency
set above. Only one is ever enabled per build invocation — each generates a different C
entry point under the same crate, not meant to coexist in one binary:

```sh
cargo cdylib                    # -> the plain shared library every FFI binding loads (no_std)
cargo rustc --release --crate-type cdylib --features python  # -> PyInit__native (normally via maturin in python/)
cargo ruby-ext                  # -> Init_hyperuuid_native, in target/ruby/release/ (ruby/'s `rake native:dev` runs this and stages the result)
cargo php-ext                   # -> get_module, in target/php/release/
```

`cargo cdylib` is an alias in `.cargo/config.toml` for `cargo rustc --release --crate-type
cdylib` with the `cdylib` feature in place of `std` and panics set to abort. The manifest
declares only the rlib, so a plain `cargo build` produces no shared library; the crate type
is named per invocation, which is what keeps it out of every consumer's build. The
extensions are built with std and unwinding, so a panic in one still reaches the host as
an exception — which is why the python one is spelled out rather than run through the
alias.

Each produces `libhyperuuid.{so,dylib}` (`hyperuuid.dll` on Windows), under `target/release/` for the raw `--features` form and under its own directory for the aliases — the
interpreter-specific loading/staging (module naming, `.pyd`/`.bundle` renaming, etc.) is this
repo's Python/Ruby/PHP packages' job, not this crate's; verified to work from the published
crate itself, not just an in-repo checkout — `cargo rustc --release --crate-type cdylib
--features <one> --manifest-path` against the packaged crate (`cargo package`'s own
output) produces the same three symbols above.

**Local dev trap worth knowing:** the raw `--features` form, run in `rust/`, writes
`target/release/libhyperuuid.{so,dylib}`, *the same file* `cargo cdylib` writes — so a
`cargo rustc --release --crate-type cdylib --features python` (or `ruby`, or `php`) there
silently replaces the plain cdylib that every other binding's dev loop loads. The
extension build still exports the `uuid_*` symbols, but it also carries undefined
interpreter symbols that only resolve inside a CPython (or Ruby, or PHP) process, so the
next `./gradlew test` or `dotnet test` fails at native load with something unhelpful about a
missing symbol. Nothing is broken; a plain `cargo cdylib` puts it back. The `cargo ruby-ext` and
`cargo php-ext` aliases in `.cargo/config.toml` avoid it by building into `target/ruby/` and
`target/php/`, and `python/.cargo/config.toml` does the same for maturin (`python/target/`), so
neither the aliases nor a `maturin build` or `maturin develop` in `python/` hits it.
CI never hits this — each leg builds in its own job.

## Verifying provenance

The published `.crate` verifies from the release tag alone: the release attestation, signed by
this repository's `release.yml` at the tag, names it among every file the release ships, with
the release inventory as its predicate.

```sh
curl -LO https://static.crates.io/crates/hyperuuid/hyperuuid-<version>.crate
gh attestation verify hyperuuid-<version>.crate --repo SkunkWerkx/HyperUuid \
  --source-ref refs/tags/v<version> --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
```

Its build provenance is the second check. The crate is packed by `hyper-pack-crate.yml`, a
reusable workflow that lives in `SkunkWerkx/.github`, and that is the identity Fulcio records
as the signer, so `--repo` alone is not enough there: add `--signer-repo`, or use `--owner` in
place of both.

```sh
gh attestation verify hyperuuid-<version>.crate \
  --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
# or: gh attestation verify hyperuuid-<version>.crate --owner SkunkWerkx
```

Get the signer wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`, which
reads like a bad signature but is only an identity mismatch. The root README's
[Provenance](../README.md#provenance) section has the whole picture: the inventory, which
artifacts are signed from where, and the gates every byte passes before publication.

## Install

```sh
cargo add hyperuuid
```

Requires Rust 1.85 or later (`rust-version` in the manifest, edition 2024's own floor); CI
checks the library, with and without std, on exactly that toolchain on every PR.

Published to [crates.io](https://crates.io/crates/hyperuuid). Proven by CI building and testing this crate fresh on 5 real-hardware platform legs, on Intel macOS under Rosetta, and in Alpine containers for musl, plus the full `cargo test` suite (the benchmarks are compiled by clippy, not run) before every release (`.github/workflows/ci.yml`); `release.yml` doesn't retest anything itself — it finds that already-green run for the tagged commit and packs the crate from that same commit, with `cargo package`'s own verify build the only compile it does.

## License

[MIT](LICENSE)
