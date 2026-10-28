# Rust `uuid1` feature and MSRV 1.89

Written 2026-10-28 from ROADMAP Phase 1 (Rust row) and the researcher's brief. Status: **Draft
for owner review.** Ships in 1.1.0 with the rest of Phase 1.

## Goal

A Rust program that already holds `uuid::Uuid` (sqlx, diesel, serde, axum) takes a HyperUuid id,
or hands one back, with `.into()` instead of `Uuid::from_bytes(id.into_bytes())`. That removes
the hand-written adapter every adopter would otherwise carry, and the crate's own README concedes
`uuid` is "what most of the Rust ecosystem already expects to interoperate with". The feature is
off by default, so the `no_std`, no-alloc and no-panic claims and all nine other bindings are
untouched. The MSRV rises from 1.85 to 1.89 because current `uuid` (1.27 and later) declares it.

## Decisions

| Decision | Choice | Date |
| --- | --- | --- |
| Scope | `From` both ways, `From<&Uuid>`, cross-type `PartialEq`. No `unsafe`, no dependency but `uuid` | owner, 2026-10-28 |
| Feature name | `uuid1`; the dependency is `dep:uuid`, so `uuid1` is the only public name and `uuid2` sits beside it for uuid 2.0 | owner, 2026-10-28 |
| No-panic | `#[no_panic]` on the bodies, call sites in `examples/no_panic.rs`, `ci.yml` override | owner, 2026-10-28 |
| docs.rs | `features = ["uuid1"]`, `--cfg docsrs`, banners (see Design for the spelling) | owner, 2026-10-28 |
| MSRV | 1.85 to 1.89, follows `uuid` 1.29.0; the 1.26.1 lock pin is dropped | owner, 2026-10-28 |
| Siblings | Frozen until v1.1 ships. No sibling work here | owner, 2026-10-28 |
| Vehicle | 1.1.0 | owner, 2026-10-28 |

## Design

### The dependency (checked 2026-10-28)

Latest `uuid` is **1.29.0** (crates.io API, published 2026-10-23, `rust_version` 1.89.0; 1.27.0
and 1.28.0 are also 1.89, 1.26.1 is 1.85). No 2.x. In `rust/Cargo.toml`, `[dependencies]`:

```toml
uuid = { version = "1.29", optional = true, default-features = false }
```

and `[features]`: `uuid1 = ["dep:uuid"]`. `default-features = false` keeps `uuid` core-only
(`uuid` is `#![no_std]` unconditionally; its `std` feature only forwards to wasm-bindgen crates).
The floor `1.29` is deliberate: with the MSRV at 1.89 nothing older is useful, and a consumer
resolving `uuid` below 1.27 would unify to a different crate than ours. The existing
dev-dependency `uuid = { version = "1", features = ["v4","v5","v7"] }` stays as it is: Cargo
accepts one name in both tables and the probe built and tested with both. Its `"1"` resolves to
the same locked 1.29.0. `rust/Cargo.lock` moves `uuid` 1.26.1 to 1.29.0 and nothing else (diffed
in a scratch copy: same package names, one version line), committed with `--locked` in mind.

### The impls: new `rust/src/uuid1.rs`

Declared `#[cfg(feature = "uuid1")] mod uuid1;` after `mod uuid;` in `lib.rs`. The crate root
has a module named `uuid`, so the file refers to the dependency as `::uuid::Uuid` (the leading
`::` is required in `lib.rs` and kept everywhere for the same reading). Each impl is `#[inline]`
and carries `#[cfg_attr(feature = "no-panic", no_panic::no_panic)]` on the method, like
`uuid.rs:188`. Public API, in the order they appear in the file:

```rust
impl From<::uuid::Uuid> for Uuid         { fn from(u: ::uuid::Uuid) -> Self }  // Uuid::from_bytes(u.into_bytes())
impl From<Uuid> for ::uuid::Uuid         { fn from(u: Uuid) -> Self }          // ::uuid::Uuid::from_bytes(u.into_bytes())
impl From<&Uuid> for ::uuid::Uuid        { fn from(u: &Uuid) -> Self }         // ::uuid::Uuid::from_bytes(*u.as_bytes())
impl PartialEq<::uuid::Uuid> for Uuid    { fn eq(&self, other: &::uuid::Uuid) -> bool }  // self.as_bytes() == other.as_bytes()
impl PartialEq<Uuid> for ::uuid::Uuid    { fn eq(&self, other: &Uuid) -> bool }          // same comparison
```

No `unsafe`: `from_bytes`, `into_bytes` and `as_bytes` exist on both types, and `Uuid` has them
as `const fn`. Orphan rule is satisfied because `hyperuuid::Uuid` is local to both directions.
Not provided, on purpose: `PartialOrd` across the types (not in the owner's scope; both orders
are bytewise, so it can be added later without a break), `From<&uuid::Uuid>`, and any conversion
for the batch door (`new_v7_batch` stays `&mut [u8]`; a `&mut [uuid::Uuid]` helper needs `unsafe`
or `bytemuck` and is out of scope).

**Hash/Eq consistency.** The std contract is `k1 == k2 ⇒ hash(k1) == hash(k2)` for one type's
`Hash` and `Eq`. Both types derive `Eq` and `Hash` over `[u8; 16]`, so each is internally
consistent, and `PartialEq<Other>` carries no `Hash` requirement of its own. A cross-type
`HashMap` lookup would need `Borrow`, which we do not implement, so the two key types cannot be
mixed in one map and the contract cannot be violated through us. The two hashes differ today (measured 2026-10-28, corrected
during implementation): `uuid` 1.29 hashes with one `state.write(&self.0)` (`lib.rs:951-955`),
while our derived `Hash` over `[u8; 16]` writes a length prefix first. A test,
`hashes_differ_today`, pins that and is labelled empirical. The docs say not to mix the types
as map keys.

### The no-panic proof

1. `#[no_panic]` on all five bodies, as above.
2. `rust/examples/no_panic.rs`: a block before `let ptr = out.as_mut_ptr();`, gated
   `#[cfg(feature = "uuid1")]`, built from the example's existing `id` (opaque to the optimizer
   through `Uuid::from_str(text)`):
   ```rust
   let u1: ::uuid::Uuid = black_box(id).into();
   let u2: ::uuid::Uuid = (&black_box(id)).into();
   let back: Uuid = black_box(u1).into();
   black_box(back == u2);
   black_box(u1 == back);
   ```
   covering all five impls. The header comment's build line gains `,uuid1` as a second form.
3. `.github/workflows/ci.yml`, `check-core` `with:` (after `no_std_rustflags`):
   ```yaml
   no_panic_command: cargo build --release --example no_panic --features no-panic,uuid1
   ```
   It replaces the Forge default, which builds without `uuid1`; the example still builds without
   the feature on any other invocation, so default-feature proof is a subset of this one. Runners
   stay the Forge's (ubuntu, windows).

### `extra_features` wiring

`ci.yml` `check-core` `with:` gains `extra_features: uuid1`. In the Forge that puts the feature
into clippy `--all-targets` (compile only, hyper-check-core.yml:459) and, in `check-msrv`, into
`cargo check --lib --features uuid1` on the manifest's `rust-version` (:487-492); the
`no_panic_command` override links the impls into the no-panic proof. CI does not run
`cargo test --features uuid1`: that Forge step sits in the `check-core` job, gated
`if: inputs.core_script != ''`, and HyperUuid sets no `core_script`. So `uuid1_interop` runs
locally only, until a two-line Forge gate change lands, offered to the owner separately.
The semver job is unchanged (see below).

### docs.rs

`rust/Cargo.toml` gains:

```toml
[package.metadata.docs.rs]
features = ["uuid1"]
rustdoc-args = ["--cfg", "docsrs"]
```

Never `all-features`: it would turn on `staticlib`, `cdylib` and the extension features. `lib.rs`
gains `#![cfg_attr(docsrs, feature(doc_cfg))]`.

**Attribute spelling, verified 2026-10-28 on `rustc 1.101.0-nightly (16d1ffac1 2026-10-26)`**
(scratch copy, `RUSTDOCFLAGS='--cfg docsrs' cargo +nightly doc --features uuid1 --no-deps`): the
auto-cfg behaviour is now part of `doc_cfg`. The impls sit in a `#[cfg(feature = "uuid1")]`
module, and `struct.Uuid.html` shows five "Available on crate feature `uuid1` only" banners with
`feature(doc_cfg)` alone and no per-item attribute; an explicit `#[doc(cfg(feature = "uuid1"))]`
is also still accepted. **Choice: no per-item attribute.** The module gate is the single source
of truth, and one fewer nightly-only spelling can break the whole docs.rs build. The banner is
what the decision asked for; the spelling is the implementation. Control: the same command
without `--cfg docsrs` produces zero banners (measured: 0).

### The MSRV: 1.85 to 1.89

`rust/Cargo.toml` `rust-version = "1.89"` is the single source (the Forge's `check-msrv` reads
it with `sed`, hyper-check-core.yml:483). Nothing in `ci.yml` or the Forge names 1.85 or 1.89:
searched `.github/` and `../HyperForge/.github` for `1.85` and `1.89`: the only hits are a Forge test fixture (`test_release_guard.py:564`, a sample manifest) and a README line that mentions `check-msrv`'s 'exactly the rust-version' without a number to change. Places that do name it, found by repo-wide grep
(excluding history and `target`):

| Place | Now | Change |
| --- | --- | --- |
| `rust/Cargo.toml:5` | `1.85` | `1.89` |
| `rust/README.md:330` | "Rust 1.85 or later (... edition 2024's own floor)" | see Contract changes |
| `README.md:155` (R source) | "needs Rust 1.85 or newer" | 1.89 |
| `README.md:165` (floors) | "Rust 1.85 (... edition 2024's own floor)" | see Contract changes |
| `r/configure:19,26`, `r/configure.win:19,26` | comment and message "1.85" and the version test | 1.89 |
| `r/README.md:21,31`, `r/cran-comments.md:32` | "1.85" | 1.89 |
| `rust/src/python_ext.rs:231` | comment "rust-version is 1.85, and as_chunks is 1.88" | replace the loop with `as_chunks` and drop the comment |
| Python sdist line "needs Rust >= 1.89" | owned by the Python item | already 1.89; this spec adds nothing |
| `CHANGELOG.md:707,749` | 1.0.0 history | untouched |
| `docs/superpowers/plans/2026-10-26-r-binding.md:264` | historical plan | untouched |

**A real consequence the brief missed: clippy.** Rust 1.89 as the MSRV makes
`clippy::chunks_exact_to_as_chunks` apply (as_chunks is stable from 1.88). Measured on stable
1.99 clippy with `rust-version = "1.89"`: `cargo clippy --features uuid1 -- -D warnings` fails at
`rust/src/v5.rs:63` (`chunks_exact(64)`) and `v5.rs:88` (`chunks_exact_mut(4)`). Both move to
`as_chunks` / `as_chunks_mut` (which cannot panic and keep the proof), plus `python_ext.rs:231`;
`--all-targets` may report more (`tests.rs:567`, `v7.rs:630,757` are `chunks_exact(16)`), fixed
the same way if clippy flags them. The v5 bytes are pinned by `corpus/v5_*.json` and
`tests/v5_reference.rs`, so a wrong rewrite fails loudly.

**R reaches 1.89 without using `uuid`.** Cargo refuses a path dependency whose `rust-version`
exceeds `rustc`, so every R source install now needs 1.89, though R never builds `uuid1`. That
is the owner's trade, and it is labelled in the audit.

### Dependabot

`rust/` is in the cargo entry's `directories` (monthly, one grouped PR, 7-day cooldown). Whether
Dependabot's cargo updater honours `rust-version` is **unverified**; assume it does not.
Behaviour after this change: it can bump `uuid` within 1.x freely (all satisfy 1.89) and may
bump any crate whose floor exceeds 1.89. That fails `check-msrv` on the PR (a pull-request check,
visible before merge), which is the guard the existing comment in `dependabot.yml:30-31` already
names. No `dependabot.yml` change. The owner then holds the pin or raises the floor
deliberately.

### semver-checks

The Forge runs `cargo-semver-checks` with `feature-group: default-features` against 1.0.0, so
`uuid1`'s items are never diffed in CI. Run by hand on the scratch copy (cargo-semver-checks
0.51.0, 2026-10-28), both default and `--features uuid1`: "229 checks: 229 pass, 31 skip, no
semver update required". An MSRV bump is not a semver-checks lint; the crate treats it as a
minor-release change and says so in the CHANGELOG.

### Feature isolation

The cdylib, staticlib, python, ruby and php features never name `uuid1`, and `uuid` is only
reachable through `dep:uuid`. Measured in the scratch copy with `cargo tree --offline -e normal
--features <f> | grep -c ' uuid v'`: default 0, `cdylib` 0, `staticlib` 0, `python` 0, `ruby` 0,
`php` 0, **`uuid1` 1** (the control). The wasm-bindgen count under `--no-default-features
--features uuid1` is 0. TypeScript depends on `../rust` with default features; R's wrapper takes
`default-features = false, features = ["staticlib"]` (`r/src/rust/Cargo.toml:11`); neither names
`uuid1`. The R tarball: `cargo vendor` skips an unactivated optional dependency (pyo3 and magnus
are absent from `r/inst/AUTHORS`), so `uuid` stays out; proven below by running `vendor.sh`.

## Contract changes

**`rust/README.md`**, new subsection after "Why not the `uuid` crate?" and its trade-off
paragraph, which ends "and is what most of the Rust ecosystem already expects to interoperate
with." Add one sentence there: "Enable the `uuid1` feature to convert both ways."

> ### Interoperating with the `uuid` crate
>
> The `uuid1` feature (off by default) adds `From` conversions between `hyperuuid::Uuid` and
> `uuid::Uuid` 1.x, in both directions and from `&Uuid`, and `==` across the two types. It pulls
> in `uuid` 1.29 or newer with default features off, nothing else, and the conversions are a
> copy of the 16 bytes with no `unsafe`.
>
> ```toml
> hyperuuid = { version = "<version>", features = ["uuid1"] }
> ```
> ```rust
> let id: uuid::Uuid = hyperuuid::v7::now_v7()?.into();
> assert_eq!(id, hyperuuid::Uuid::from(id));
> ```
>
> The name carries the major version of `uuid`; a future `uuid2` feature would sit beside it.
> The two types do not hash alike, so do not use one as a map key and look up with the other.

The example is run by hand in the implementation pass (`?` needs a `Result` main). The features
list (README around :248) gains `uuid1`. Do **not** claim the conversions are panic-free beyond
"proved by the `no-panic` build", which is the claim the rest of the README already makes.

**`rust/README.md:330` and root `README.md:165`**, the floor sentences, as they should finally
read:

> Requires Rust 1.89 or later (`rust-version` in the manifest, the floor of the current `uuid`
> 1.x); CI checks the library, with and without std, on exactly that toolchain on every PR.

Root `README.md:165`: "The crate's own floor is Rust 1.89 (`rust-version` in its manifest, the
floor of the current `uuid` 1.x), and CI checks the library, with and without std, on exactly that
toolchain." `README.md:155`: "needs Rust 1.89 or newer".

**`CHANGELOG.md`**, `[Unreleased]`:

> - **Rust — `uuid1` feature.** `From<uuid::Uuid> for Uuid`, `From<Uuid> for uuid::Uuid`,
>   `From<&Uuid> for uuid::Uuid` and `==` across the two types, behind an off-by-default feature
>   that adds `uuid` 1.29 or newer without its default features. The bodies carry `#[no_panic]`
>   and are linked into the no-panic proof. *(Rust)*
> - **Rust, R — minimum Rust is now 1.89** (was 1.85), the floor of current `uuid` 1.x. Source
>   installs of the R package and builds from this repository need `rustc` 1.89. *(Rust, R)*

(Pre-1.0 wording rules do not apply: it is 1.x, a plain entry, no upgrade hand-wringing.)
`ROADMAP.md:59` Rust row is ticked.

## Files

**[own]**
- `rust/src/uuid1.rs` (new); `rust/src/lib.rs` (the `mod` line, `cfg_attr(docsrs…)`, one
  module-doc bullet at :7-19); `rust/Cargo.toml` (`rust-version`, the dependency, `uuid1`,
  `[package.metadata.docs.rs]`); `rust/Cargo.lock` (uuid 1.29.0 only)
- `rust/tests/uuid1_interop.rs` (new, `#![cfg(feature = "uuid1")]`): round trip both ways, `&Uuid`,
  both `==` directions, `Display` parity, a `now_v7` pair ordering identically, the Hash
  agreement pin
- `rust/examples/no_panic.rs`; `rust/README.md` (:90 sentence, new subsection, features list,
  :330); `rust/src/v5.rs:63,88`, `rust/src/python_ext.rs:231` (clippy under 1.89), plus any
  `--all-targets` hit in `tests.rs`/`v7.rs`
- `r/configure`, `r/configure.win`, `r/README.md`, `r/cran-comments.md` (1.85 to 1.89)

**[shared]** (4 touches, each limited to the lines named)
- `CHANGELOG.md`: the two `[Unreleased]` bullets above
- `ROADMAP.md`: tick the Rust row (:59)
- `.github/workflows/ci.yml`: `check-core` `with:` gains `extra_features: uuid1` and
  `no_panic_command: …` only
- `README.md`: :155 and :165 version tokens and the :165 clause only

**Forge:** none. `extra_features` and `no_panic_command` already exist as inputs.

Not touched: C-export lists, `check_exports.py`, `.github/hyper.json`, `cdylib_smoke.py`,
`dependabot.yml`, the other eight bindings (they interoperate through bytes and strings with their
native UUID type, which the CHANGELOG need not say).

## Ordering constraints

None across repos. Inside the branch: the clippy `as_chunks` rewrites land with the
`rust-version` bump, not after it, or CI's clippy fails. The `Cargo.lock` update must be
committed with the manifest (`cargo update -p uuid --precise 1.29.0`), or `--locked` jobs fail.
The Python item's sdist sentence and this spec both say 1.89; either merges first.

## Verification

**Local** (from `rust/`; control follows each):
1. `rustup toolchain install 1.89`, then `cargo +1.89 check --lib --features uuid1`. Control: the
   already-installed `1.88` fails with "hyperuuid@1.0.0 requires rustc 1.89" (observed).
2. `cargo test --features uuid1`. Control: flip one byte in one `From` body; `uuid1_interop`
   fails.
3. `cargo clippy --all-targets --features uuid1 -- -D warnings` and `cargo fmt --check`. Control:
   the same clippy on the tree before the `as_chunks` rewrites fails at `v5.rs:63` (observed).
4. Isolation: the `cargo tree` loop above for each feature. Control: `--features uuid1` returns 1.
5. no_std: `RUSTFLAGS='--cfg getrandom_backend="custom"' cargo check --lib --no-default-features
   --features uuid1 --target thumbv7em-none-eabi` and `--target wasm32-unknown-unknown` (both
   passed in the scratch copy). This is the compile proof: on the no_std target a `std`-requiring
   crate fails to build. Control: a scratch copy whose `uuid1.rs` adds `extern crate std;` and one
   `std::string::String` conversion must fail the thumbv7em check.
   **Dependency isolation is a separate check** (amended after review, 2026-10-28): `cargo tree
   --no-default-features --features uuid1 -e normal --target wasm32-unknown-unknown` lists `uuid`
   and nothing below it. Control: a scratch copy that turns on `uuid`'s `js` feature, which
   activates `dep:wasm-bindgen`, makes the wasm-bindgen count non-zero. Enabling `std` is no
   control for this: in `uuid` 1.29 it is `wasm-bindgen?/std` and `js-sys?/std`, weak features
   that activate nothing.
6. docs: the nightly command above shows five banners. Control: without `--cfg docsrs`, zero.
7. semver: both commands above. Control: delete `Clone` from `Uuid`'s derive; `semver-checks`
   fails.
8. R tarball: run `r/tools/vendor.sh`; `tar tf r/src/rust/vendor.tar.xz | grep -c '^vendor/uuid-'`
   is 0 and `inst/AUTHORS` is unchanged. Control: in a scratch copy add `"uuid1"` to the wrapper's
   `features`, re-vendor, and the count is 1.
9. no-panic: **not provable on this Mac.** The negative control did not fire here (a deliberate
   `panic!` in a `From` body still linked), because the proof needs GNU ld; the Forge restricts it
   to ubuntu and windows for that reason. Run it on Linux in a `rust:1.89` container (docker is
   installed): `cargo build --release --example no_panic --features no-panic,uuid1` must pass,
   and with a `panic!` branch in one body it must fail with `ERROR[no-panic]`.

**CI-only:** `check-core` (x64, arm64) running clippy `--all-targets --features uuid1` (compile
only; `uuid1_interop` does not run in CI, see `extra_features` wiring) and the Android clippy leg; the `no_panic` job on ubuntu and windows with the new command; `check-msrv`
on 1.89 with `--features uuid1`; `check-semver` (default group, a no-op for this feature).

**Release-time:** `cargo package --features uuid1` dry run; after publish, open the docs.rs page
for 1.1.0 and see the banners (a docs.rs failure does not block the crate but loses the docs; a
nightly spelling change is the risk). First CRAN and source-install builds run on 1.89.

## Nitpick audit

| Hole | Closure |
| --- | --- |
| "Panic-free" for conversions not in the proof | The `ci.yml` override and example call sites put all five in the link-time proof; on a Mac the control cannot run, so Linux/CI is named as the proof |
| `PartialEq` across types without `Hash` agreement | Argued above: no `Borrow`, so no mixed map. The hashes differ today (`hashes_differ_today`, empirical), and the README says not to mix keys |
| Cargo.toml says `uuid` 1.29 but a user has older | `version = "1.29"` makes Cargo resolve at least 1.29; stated in README |
| docs.rs breaks on nightly attribute churn | Only `feature(doc_cfg)` and the module gate; verified on nightly 2026-10-26; release-time docs.rs check |
| `uuid1` leaks into other builds | `cargo tree` loop with control; R vendoring check with control |
| MSRV 1.89 hits R source builds, and the Python sdist once it ships, that never use `uuid` | Real and accepted by the owner; CHANGELOG, READMEs and `configure` messages say 1.89. CRAN's machines checked against eitsupi/cran-rust-version's 2026-10-25 snapshot (lowest 1.91.1, macOS oldrel arm64); re-checked before the hand submission |
| Dependabot ignoring `rust-version` | Unverified; `check-msrv` on the PR catches it |
| "uuid1 works in no_std" | Passes locally (thumbv7em, wasm32); CI's no_std jobs do not enable `uuid1`, so the README makes no such claim |
| `uuid` 2.0 | A new `uuid2` feature; `uuid1` never changes meaning |
| Downloads, benchmark claims | None added; no number in this change |

## Out of scope / Later

A batch helper over `&mut [uuid::Uuid]` (needs `unsafe` or `bytemuck`); `PartialOrd` across the
types; `serde`/`sqlx` integrations; `uuid2`. HyperCast and HyperTabular get their own roadmaps
after v1.1.

## Open items

1. Resolved 2026-10-28: one combined CHANGELOG bullet, tagged *(Rust, R)*. Python is not named
   until its sdist ships; the Python item adds its own 1.89 line.
