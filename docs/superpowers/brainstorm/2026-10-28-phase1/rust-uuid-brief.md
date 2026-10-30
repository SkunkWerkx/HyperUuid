# Design brief: Rust `uuid` crate interop (ROADMAP Phase 1, Rust)

Researched 2026-10-28. Read-only; nothing in any repo was edited. One scratch probe lives under the session scratchpad (`.../scratchpad/probe`), outside every repo.

## 1. Goal

Let a Rust program that already holds `uuid::Uuid` (sqlx, diesel, serde stacks, axum extractors) take a HyperUuid id, or hand HyperUuid output on, with `.into()` instead of `Uuid::from_bytes(id.into_bytes())`. It sits behind an off-by-default Cargo feature, so the `no_std`/no-alloc/no-panic claims and every binding build stay exactly as they are. It matters now because 1.0.0 is on crates.io (271 downloads, crates.io API, 2026-10-28) and the README itself says `uuid` is "what most of the Rust ecosystem already expects to interoperate with" (`rust/README.md:90`). Without conversions, adopting the crate means a hand-written adapter in every consumer.

## 2. Current state

**The public type is already a newtype of ours.** `pub struct Uuid([u8; 16])` with `#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]` (`rust/src/uuid.rs:9-10`). It has no `#[repr(transparent)]`. It already has `from_bytes`/`as_bytes`/`into_bytes` (`uuid.rs:20-32`) and `impl From<[u8; 16]> for Uuid` (`uuid.rs:132`), plus `Display`, `Debug` and `FromStr` (`uuid.rs:138-218`). It is re-exported from `lib.rs:149`.

Generators return `Result<Uuid, _>`: `v4::new_v4` (`v4.rs:10`), `v5::new_v5(Uuid, &[u8]) -> Uuid` (`v5.rs:48`), `v7::new_v7`, `new_v7_at`, `now_v7` (`v7.rs:106,189,491`). The batch door writes raw bytes into the caller's buffer: `new_v7_batch(millis, count, out: &mut [u8])` (`v7.rs:229`). `to_sql_order`/`to_rfc_order` take and return `Uuid` (`v7.rs:576,597`).

So the orphan rule is not a problem. `impl From<uuid::Uuid> for hyperuuid::Uuid` and the reverse are both legal because `hyperuuid::Uuid` is local. I compiled both in the probe (edition 2024, `uuid =1.26.1`, `default-features = false`, `#![no_std]`) for `thumbv7em-none-eabi` and `wasm32-unknown-unknown`; both passed. The forbidden impl is only `From<uuid::Uuid> for [u8; 16]`, and nobody needs it: `uuid::Uuid::into_bytes()` already exists.

**Manifest and checks today** (`rust/Cargo.toml`):
- `rust-version = "1.85"` (:5). `default = ["std"]` (:57). Optional deps for python/ruby/php/no-panic (:25-28).
- `uuid = { version = "1", features = ["v4","v5","v7"] }` is a dev-dependency only (:96), used by the benches (`benches/uuid_benchmarks.rs:123-133`) and `tests/v5_reference.rs:12-22`. `rust/Cargo.lock` is tracked and pins uuid 1.26.1 (`Cargo.lock:1692`).
- There is no `[package.metadata.docs.rs]` and no `doc_cfg` anywhere in the crate.
- `no-panic` puts `#[no_panic]` on every generation, parse and conversion function (`Cargo.toml:84-91`; e.g. `uuid.rs:188`). The link-time proof is `examples/no_panic.rs`, built with `--features no-panic` (the Forge default `no_panic_command`, `hyper-check-core.yml:33-37`). `ci.yml` does not override it.
- The Forge's `check-core` (`ci.yml:199-204`; `HyperForge/.github/workflows/hyper-check-core.yml`) runs:
  - `cargo check --no-default-features --target thumbv7em-none-eabi` (:170);
  - a `default-features = false` consumer on the host and wasm32-unknown-unknown (:178-191);
  - the `extra_features` input, which is empty in HyperUuid's `ci.yml`: `cargo test --features X` (:247-248), clippy with it (:459-464), MSRV `cargo check --lib --features X` (:494);
  - an MSRV job that runs `cargo check --lib --no-default-features --features cdylib` (:495);
  - cargo-semver-checks with `feature-group: default-features` (:509-514).
- R vendors only activated crates. `r/inst/AUTHORS` lists no pyo3 or magnus even though both are optional deps of the core, so an optional `uuid` should not enter the R tarball. Re-confirm by running `r/tools/vendor.sh` once (`vendor.sh:52-58`).

## 3. Upstream facts

| Fact | Source | Checked |
| --- | --- | --- |
| Latest `uuid` is 1.29.0 (2026-10-23). No 2.x on crates.io. | `https://crates.io/api/v1/crates/uuid` | 2026-10-28 |
| MSRV: 1.25.0 to 1.26.1 are `rust-version` 1.85.0; **1.27.0, 1.28.0 and 1.29.0 are 1.89.0**. | same API, and `uuid-1.29.0/Cargo.toml:14` in the local registry | 2026-10-28 |
| `uuid` is `#![no_std]` unconditionally. `std` is a feature (default on) that only forwards to the optional wasm-bindgen crates. `default-features = false` yields a core-only crate. | `uuid-1.29.0/src/lib.rs:209`, `Cargo.toml:96-135`; probe built on thumbv7em | 2026-10-28 |
| `uuid::Uuid` is `#[repr(transparent)] struct Uuid(Bytes)`, with a doc-level ABI guarantee ("always guaranteed to have the same ABI as `Bytes`"). | `uuid-1.29.0/src/lib.rs:446-466` | 2026-10-28 |
| Ordering is derived on `[u8; 16]`: lexicographic bytes, the same as ours. `Hash` is derived on both, but the two `Hash` impls are not required to agree. | `lib.rs:445`; ours `uuid.rs:9` | 2026-10-28 |
| Safe, cast-free conversions exist: `Uuid::from_bytes([u8;16])`, `into_bytes()`, `as_bytes()`, `from_bytes_ref(&[u8;16]) -> &Uuid`. There is no slice-level one. | `builder.rs:364,434`; `lib.rs:799,819` | 2026-10-28 |
| `uuid`'s optional `bytemuck` feature derives `Pod`/`TransparentWrapper` on `uuid::Uuid`. Our `Uuid` is not `Pod`. | `lib.rs:452-455` | 2026-10-28 |
| `uuid` already has the unstable docs.rs plumbing: `#![cfg_attr(docsrs, feature(doc_cfg))]`. | `lib.rs:208` | 2026-10-28 |
| cargo-semver-checks runs `default-features` only, so a new non-default feature's items are never diffed against 1.0.0. | `hyper-check-core.yml:509-514` | 2026-10-28 |

**UNVERIFIED:** whether current nightly rustdoc applies `doc_cfg` automatically to `#[cfg(feature = ...)]` items (the old `doc_auto_cfg` folded into `doc_cfg`). I did not check the rustdoc book. Verify with a local `cargo +nightly doc` before writing the docs.rs metadata. I also did not verify how Dependabot treats MSRV when bumping `uuid` (see risk 1).

## 4. Approaches

**(a) Trait impls on our newtype: `From`/`Into` both ways, plus `PartialEq` across the two types.**
Roughly 25 lines in a new `rust/src/uuid1.rs`, gated `#[cfg(feature = "uuid1")] mod uuid1;`:
- `impl From<uuid::Uuid> for Uuid` and `impl From<Uuid> for uuid::Uuid`, plus `From<&Uuid> for uuid::Uuid`;
- optionally `PartialEq<uuid::Uuid> for Uuid` and the reverse, which are sound because both orderings are bytewise;
- the bodies are `Uuid::from_bytes(u.into_bytes())`, safe, const-evaluable, no allocation.

Pros: this is what the ROADMAP names. It is idiomatic, with `.into()` at call sites and `?`-free conversions, and it needs no `unsafe`. Cons: it covers single ids only. The batch door stays `&mut [u8]`.

**(b) Free functions `new_v7_uuid() -> uuid::Uuid` and friends.**
They mirror every generator (v4, v5, v7, now_v7, at, to_sql_order). That is a second API surface of about 8 functions, each needing `no_panic` and docs. It saves one `.into()` per call.
Con: it violates parity thinking, because the other nine bindings have no such twin set, and it doubles the surface to keep in sync. Not recommended.

**(c) Extension traits (`trait IntoUuid1`, `trait HyperUuidExt for uuid::Uuid`).**
Needed only if `From` were impossible. It isn't, so this adds a trait the user must import for no gain. Not recommended.

**(d) Batch helper `new_v7_batch_uuid(millis, count, out: &mut [uuid::Uuid])`.**
`new_v7_batch` writes into `&mut [u8]` today. A `&mut [uuid::Uuid]` variant is the one place conversions would otherwise cost a second pass. Options:
- **d1, a safe two-pass:** generate into a stack chunk and copy. This breaks the single-reservation semantics and adds scratch space, which the README forbids ("not on the heap and not on the stack", `v7.rs:200-201`).
- **d2, a cast with a few lines of `unsafe`:** `core::slice::from_raw_parts_mut(ptr.cast::<u8>(), len * 16)` plus `const { assert!(size_of::<uuid::Uuid>() == 16 && align_of::<uuid::Uuid>() == 1) }`. It is sound because `uuid` documents the ABI as `Bytes` (`lib.rs:446`). Cost: the crate's `unsafe` is currently limited to `ffi`, the extension shims, entropy and the clock read (`v7.rs:505-506`), and the README advertises "zero unsafe in the public API". The wrapper would be a safe fn over a private unsafe cast, so the public API claim holds. A `no_panic` proof of it is plausible but unproven.
- **d3, `uuid`'s `bytemuck` feature** plus `bytemuck::try_cast_slice_mut`: no `unsafe` of ours, but it pulls `bytemuck` in whenever the feature is on. `try_cast_*` returns `Result`, so panic-freedom still depends on the optimizer folding the checks.

Callers who want a `&mut [uuid::Uuid]` can also generate into `[[u8;16]]` through `as_flattened_mut` (stable since 1.80) and map. That is a one-liner, so (d) is a convenience, not a gap.

**Recommendation: (a) now, with `PartialEq` across the two types; skip (b) and (c); decide (d) separately and default to leaving it out.** It is the smallest surface that delivers the ROADMAP line. It adds no `unsafe`, and the one new dependency is `uuid` itself with `default-features = false`. (d2/d3 can be added later without breaking (a); the reverse is not true.)

**Feature name: `uuid1`.**
- `uuid1 = ["dep:uuid"]`: the `dep:` form suppresses the implicit `uuid` feature, so the name is ours to choose.
- The name carries the major of the public dependency. A `uuid` 2.0 would arrive as a new additive `uuid2` feature (with its own renamed dependency, `uuid2 = { package = "uuid", version = "2", optional = true }`). `uuid1` never changes meaning, so no semver break.
- If we shipped `uuid`, a later `uuid2` would leave an unqualified `uuid` stuck on 1.x, which is the trap `uuid1` avoids.
- Cost: slightly less discoverable than `uuid`; a single README sentence covers it.
- Because the dev-dependency is also named `uuid`, the optional dependency should be declared with `package = "uuid"` under a distinct key (`uuid1 = { package = "uuid", version = "1", default-features = false, optional = true }`). That avoids a dev-dep/dep name clash and mirrors the future `uuid2` entry. Whether a dev-dep and an optional dep with the same key coexist cleanly was not probed; the renamed key sidesteps the question.

**Dependency requirement:** `version = "1"` (floor is nothing we need; `from_bytes`/`into_bytes` are in every 1.x). Build MSRV caveat is in risk 1.

## 5. Touch list (recommended: (a) with feature `uuid1`)

Marks: `[own]` only this item; `[shared]` another Phase 1 item might also touch.

This repo:
- `rust/src/uuid1.rs` `[own]`: new. The impls, with `#[cfg_attr(docsrs, doc(cfg(feature = "uuid1")))]`.
- `rust/src/lib.rs` `[own]`: `#[cfg(feature = "uuid1")] mod uuid1;` near :126-137; `#![cfg_attr(docsrs, feature(doc_cfg))]`; a module-doc bullet (:7-19).
- `rust/Cargo.toml` `[own]`: the optional dep (:22-28), the `uuid1` feature (:48-91), and `[package.metadata.docs.rs]` with `features = ["uuid1"]` and `rustdoc-args = ["--cfg","docsrs"]`. It must **not** use `all-features`, which would enable `staticlib` and `cdylib` (panic handler, asm) plus the extension features.
- `rust/Cargo.lock` `[own]`: no new package, since uuid is already locked; confirm no diff beyond the dep edge.
- `rust/tests/uuid1_interop.rs` `[own]`: new, `#![cfg(feature = "uuid1")]`. Round-trip, `now_v7` ordering vs `uuid::Uuid` compare, Display parity.
- `rust/examples/no_panic.rs` `[own]`: under `cfg(feature = "uuid1")` call each new `From` (only if they carry `#[no_panic]`).
- `rust/README.md` `[own]`: a short section plus an example verified by hand; update the "Why not" caveat (:90) and the feature list (:248+).
- `CHANGELOG.md` `[shared]`: Unreleased entry; a Rust-only line, but the file is touched by every item.
- `ROADMAP.md` `[shared]`: tick the Rust row.
- `.github/workflows/ci.yml` `[shared]`: `extra_features: uuid1` on `check-core` (:199-204), which gets test, clippy and MSRV coverage; optionally `no_panic_command: cargo build --release --example no_panic --features no-panic,uuid1`.
- Root `README.md` `[shared]`: only if the Rust paragraph mentions interop; otherwise skip.

Not touched (verified): the C-export lists, `check_exports.py`, `.github/hyper.json`, `cdylib_smoke.py`, the other eight bindings, `r/` (re-run `vendor.sh` to confirm the tarball is unchanged), and the Forge (it already offers `extra_features` and `no_panic_command`). CLAUDE.md `[shared]` only if the "Lists that move together" gets a `uuid1` line; not needed.

Forge (`../HyperForge`): none required. `[shared]` caveat: if the owner wants `extra_features` to feed the `no_panic` job natively, that is a Forge edit that every consumer sees at master.

## 6. Sibling cascade

- **HyperCast, HyperTabular:** nothing analogous. `uuid = "1"` in HyperCast is a dev-dependency (`HyperCast/rust/Cargo.toml:103`, benchmark comparison), and HyperTabular has no `uuid` dependency at all. Neither exposes a `Uuid` newtype: HyperCast's `cast_uuid` returns 16 bytes through its C ABI (`HyperCast/rust/src/ffi.rs:226-228`). No floors or features move. If HyperCast later wants `cast_uuid -> uuid::Uuid`, it would be a different design.
- **Floors drift to flag:** CLAUDE.md says floors match across the three repos, but the Rust MSRV is 1.85 here against 1.88 in HyperCast (`Cargo.toml:5`) and HyperTabular (`Cargo.toml:10`). This item does not change that; it is a separate decision.
- **Other bindings:** none. Rust-only by nature (the feature exists for Rust's type system). The "parity" statement for the CHANGELOG is that the other nine bindings already interoperate through bytes and strings with their language's native UUID type.

## 7. Verification

Local (from `rust/`), each with its negative control:
1. `cargo test --features uuid1` runs the new interop tests. Negative control: temporarily flip a byte in one `From` body and see `uuid1_interop` fail.
2. `cargo check --no-default-features --target thumbv7em-none-eabi` with `RUSTFLAGS='--cfg getrandom_backend="custom"'` and `--features uuid1`. Proves `uuid` in no_std. Control: add `uuid` default features (std) and check it still passes (it will), so the real control is `cargo tree -e features --no-default-features --features uuid1 | grep -c wasm-bindgen` returning 0.
3. `cargo build --release --example no_panic --features no-panic,uuid1`. Control: add a deliberately panicking `.unwrap()` in one `From` body and see `ERROR[no-panic]` at link.
4. A default build must not see `uuid`: `cargo tree --no-default-features -e normal | grep -c '^.*uuid '` is 0 for the default features, `cdylib`, `staticlib`, `python`, `ruby` and `php`. Control: the same command with `--features uuid1` returns non-zero.
5. MSRV: `rustup run 1.85 cargo check --lib --features uuid1` (with the committed lock, uuid 1.26.1). Control: `cargo update -p uuid --precise 1.29.0` then rerun; it must fail on 1.89's requirement, which proves the check is live.
6. wasm32: `cargo check --target wasm32-unknown-unknown --no-default-features --features uuid1` (with the CI `RUSTFLAGS`). The probe passed with uuid alone.
7. Semver: `cargo semver-checks` is a no-op for this feature (default group only). Run `cargo semver-checks --features uuid1` once by hand to confirm the diff is purely additive. Control: delete the `Clone` derive on `Uuid` and confirm it fails.
8. docs: `RUSTDOCFLAGS='--cfg docsrs' cargo +nightly doc --features uuid1 --no-deps`; look for the "Available on crate feature `uuid1` only" banner. Control: the same command without the metadata must show no banner.
9. R: run `r/tools/vendor.sh` and confirm `vendor.tar.xz` and `inst/AUTHORS` are byte-identical.
10. `cargo fmt --check` and `cargo clippy --features uuid1 -- -D warnings`.

Only CI can prove:
- the Forge's MSRV job with `extra_features` (a Linux x64 run);
- both `check-core` runners (x64, arm64);
- the Android clippy configuration;
- cargo-semver-checks against the published 1.0.0 (default group, expected to be a no-op for this change);
- the wasm32-wasip1 suite (unchanged, default features only).

Release-time: the first publish carrying the feature. Run `cargo package --features uuid1` as a dry run; docs.rs builds the pages with the metadata above and needs a check afterward (nightly-only `doc_cfg`; a build failure there does not block the crate but loses the docs).

## 8. Risks and unknowns (ranked)

1. **MSRV vs `uuid` >= 1.27 (MSRV 1.89).** Our floor is 1.85. A consumer with `uuid >= 1.27` locked already needs 1.89 regardless of us, so this is not a new constraint for them. The risk is on our side: a Dependabot bump of `rust/Cargo.lock` to uuid 1.29 would break the MSRV job on 1.85 (UNVERIFIED whether Dependabot's `cargo update` honours the MSRV-aware resolver). Edition 2024 defaults to the `rust-version`-aware resolver for fresh resolution, which is why the lock is at 1.26.1 (the last 1.85-compatible). Mitigation: keep `version = "1"` as a floor, and note this in the CHANGELOG and README install line.
2. **docs.rs nightly dependence.** `doc_cfg` is unstable, so the feature banner depends on the docs.rs toolchain. UNVERIFIED which `cfg` spelling today's nightly wants (`doc(cfg(...))` vs auto). Probe it locally (check 8). A wrong spelling breaks the docs.rs build for the whole crate, not just the feature.
3. **`no_panic` coverage is not automatic.** The feature is invisible to the default `no_panic_command`, so without the `ci.yml` override the `From` impls are unproven. The impls are trivially panic-free, but the project's claim is "every function carries it", so the override should ship with the feature.
4. **Hash/Eq contract across the two types.** If `PartialEq<uuid::Uuid>` is added, `Hash` is not guaranteed to agree between them (`uuid`'s derive is on a different struct). Eq across types is fine for equality, but the two must not be mixed as `HashMap` keys. Documentation only. Decide whether to ship cross-`PartialEq` at all.
5. **Public dependency.** `uuid` 1.x types appear in our public API under the feature. A `uuid` 2.0 forces a new feature (`uuid2`) and leaves `uuid1` supported for as long as 1.x is. Plan for it, no action now.
6. **(d) soundness.** If a batch helper is wanted, d2's `unsafe` is justified only by `uuid`'s documented ABI note; a future `uuid` change that dropped that note would silently invalidate it. The `const` size/align assert catches layout drift but not a semantic one.

## 9. Owner questions

1. **Feature name?**
   - (recommended) `uuid1`, with `uuid2` additive when 2.0 lands.
   - `uuid`, familiar to Rust users, but a later `uuid2` leaves an unqualified `uuid` frozen on 1.x.
   - `uuid-1`/`with-uuid-1` (Cargo accepts hyphens; matches tokio-postgres style).
2. **Scope of the first cut?**
   - (recommended) (a) only: `From` both ways, `From<&Uuid>`, cross-type `PartialEq`.
   - (a) without `PartialEq`, to keep the `Hash`/`Eq` question out of the API.
   - (a) plus the batch helper (d2/d3), which brings in `unsafe` or `bytemuck`.
3. **Prove the impls panic-free in CI?**
   - (recommended) yes: put `#[no_panic]` on the `From` bodies, add the `uuid1` call sites to `examples/no_panic.rs`, and override `no_panic_command` in `ci.yml`.
   - no: skip it and document them as trivially total.
4. **docs.rs: how to present the feature?**
   - (recommended) `[package.metadata.docs.rs] features = ["uuid1"]` with `--cfg docsrs` and `doc(cfg)` banners.
   - list the feature in prose only and leave docs.rs on defaults, so `uuid1` items never appear on docs.rs.
5. **Release vehicle?**
   - (recommended) a plain Unreleased CHANGELOG line, shipped with the next minor, since the feature is additive and off by default.
   - hold it for the 1.1.0 npm/CRAN release to keep that release's proof set small.
