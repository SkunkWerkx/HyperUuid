# Rust `uuid1` feature and MSRV 1.89 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `hyperuuid::Uuid` converts to and from `uuid::Uuid` 1.x behind an off-by-default `uuid1` feature, and the crate's MSRV rises from 1.85 to 1.89.

**Architecture:**
- **The conversions:** five safe impls over the 16 bytes, in a new feature-gated module `rust/src/uuid1.rs`.
- **The MSRV bump:** brings two clippy rewrites to `as_chunks`, and the 1.85 → 1.89 sweep through the R build scripts and the READMEs.
- **CI:** passes the feature to the existing Forge inputs. The Forge itself is unchanged.

**Tech Stack:** Rust (edition 2024), `uuid` 1.29, `no-panic`, the Forge's `hyper-check-core.yml`.

**Spec:** `docs/superpowers/specs/2026-10-28-rust-uuid1-msrv-design.md`. It is plan 1 of 5 for Phase 1. Its order relative to the other four is in `.superpowers/brainstorm/2026-10-28-phase1/integration.md`.

## Global Constraints

- **The MSRV:** `rust-version = "1.89"` in `rust/Cargo.toml` is its single source.
- **The dependency:** `uuid = { version = "1.29", optional = true, default-features = false }`, with the feature `uuid1 = ["dep:uuid"]`.
- **The public API is exactly five impls:**
  - `From<::uuid::Uuid> for Uuid`;
  - `From<Uuid> for ::uuid::Uuid`;
  - `From<&Uuid> for ::uuid::Uuid`;
  - `PartialEq<::uuid::Uuid> for Uuid`;
  - `PartialEq<Uuid> for ::uuid::Uuid`.
- **What the module may not use:** no `unsafe`, and no dependency beyond `uuid`. It always refers to the dependency as `::uuid::Uuid`, because the crate root already has a `mod uuid`.
- **No-panic:** every impl body carries `#[inline]` and `#[cfg_attr(feature = "no-panic", no_panic::no_panic)]`.
- **docs.rs:** set `features = ["uuid1"]` and `rustdoc-args = ["--cfg", "docsrs"]`, and add `#![cfg_attr(docsrs, feature(doc_cfg))]`. There is no per-item `doc(cfg)` attribute, and never `all-features`.
- **Lock:** `rust/Cargo.lock` moves `uuid` 1.26.1 → 1.29.0 and nothing else.
- **Stays out of every other build:** no other feature, binding or vendored R crate names `uuid1`.
- **Git:** the owner drives it (CLAUDE.md). Implementers stage nothing and commit nothing; the work stays in the working tree.
- **Docs:** README install snippets keep `<version>`. READMEs state the current state only.
- **Checks:** every check runs with its negative control. Copy the flags from CI or the repo aliases.
- **Formatting:** `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` stay clean.

## Review Focus

1. **The edge values.** The nil and max UUIDs must round-trip and compare equal across both types. Pinned in Task 2's `nil_and_max_cross_both_ways`.
2. **Byte order.** A conversion must not reverse or reorder bytes. That covers a v7, a v5 and a SQL Server-layout v8, whose layout is persisted data. Both types' `Display` strings must be identical. Pinned in Task 2's `display_matches_for_every_layout`.
3. **v5 after the `as_chunks` rewrite.** Every name length 0–300, plus 1000 and 4096, across four namespaces must give bytes identical to `uuid`'s own `new_v5`. Pinned by Task 1's extension of `every_name_length_matches_the_uuid_crate`.
4. **Borrowing.** `From<&Uuid>` leaves the original usable, and two converted `now_v7` ids keep their order. Pinned in Task 2's `borrow_conversion_keeps_the_original` and `v7_order_survives_conversion`.
5. **Hash.** The two types hash differently today: `uuid` writes the 16 bytes with no length prefix. This is labelled empirical. Pinned in Task 2's `hashes_differ_today`; the README says not to mix them as map keys.

---

### Task 1: MSRV 1.89 and the `as_chunks` rewrites

**Files:**
- Modify: `rust/Cargo.toml:5`, `rust/src/v5.rs:63` (the `chunks_exact(64)` loop) and `:88` (`chunks_exact_mut(4)`), and `rust/src/python_ext.rs:229-233` (drop the 1.85 comment and use `as_chunks::<16>()`).
- Modify: any other `--all-targets` clippy hit. Candidates are `rust/src/tests.rs:567` and `rust/src/v7.rs:630,757`.
- Modify: the R scripts and docs:
  - `r/configure:19,23,26` and `r/configure.win:19,23,26`: the comment, `-lt 85` → `-lt 89`, and the message;
  - `r/README.md:21,31` and `r/cran-comments.md:32`.
- Modify: `rust/README.md:330`, and in `README.md` line 155 plus line 165's Rust clause only. Use the spec's wording, under "Contract changes".
- Test: `rust/tests/v5_reference.rs`: the existing `every_name_length_matches_the_uuid_crate` is extended.

**Interfaces:**
- Produces: `rust-version = "1.89"`, which Task 2's `uuid` 1.29 needs.

- [ ] **Step 1: Extend the reference test.** In `every_name_length_matches_the_uuid_crate` (`rust/tests/v5_reference.rs:9`), keep the `0..=300` loop. Add lengths 1000 and 4096 with the same name pattern and assertion. `v5::new_v5` takes the namespace by value.
- [ ] **Step 2: Run it on the current tree.**
  ```
  cargo test --test v5_reference
  ```
  Expected: PASS. It is the guard for Step 4's rewrite, not a red test. It runs again in Step 5.
- [ ] **Step 3: RED for the bump.** Set `rust-version = "1.89"`, then run:
  ```
  cargo clippy --all-targets -- -D warnings
  ```
  Expected: FAIL with `chunks_exact_to_as_chunks` at `v5.rs:63`, plus any other sites. Record every site it names.
- [ ] **Step 4: Rewrite each named site.**
  - Use `as_chunks::<N>()` and `as_chunks_mut::<N>()` (both `(chunks, remainder)`). The remainder carries on where the old remainder did.
  - Drop the now-unneeded `try_from`. No `expect` and no indexing that can panic.
  - In `python_ext.rs`, the remainder is empty by construction, so iterate the chunks and ignore it.
- [ ] **Step 5: GREEN.** Run, in `rust/`:
  - `cargo clippy --all-targets -- -D warnings`;
  - `cargo test`;
  - `cargo test --test v5_reference`;
  - `cargo fmt --check`.

  Expected: all PASS, with the corpus v5 vectors unchanged.
- [ ] **Step 6: The floor rejects older toolchains.** Run:
  ```
  rustup toolchain install 1.89 1.88 --profile minimal
  cargo +1.89 check --locked --lib
  ```
  Expected: PASS.
  - NC: `cargo +1.88 check --locked --lib` must FAIL with "requires rustc 1.89".
  - The full MSRV matrix runs in Task 5, once the final manifest and lock exist.
- [ ] **Step 7: The 1.85 sweep.** Edit the R and README lines listed under Files. Then run:
  ```
  grep -rn '1\.85' --exclude-dir=target --exclude=CHANGELOG.md --exclude-dir=plans --exclude-dir=specs .
  ```
  Expected: no hits in live docs or scripts.
  - NC for `r/configure`: running it with a fake `rustc` on `PATH` that prints `rustc 1.88.0` must exit 1 with the 1.89 message. The same fake printing `rustc 1.89.0` must pass the version gate.

### Task 2: The `uuid1` feature and its conversions

**Files:**
- Modify: `rust/Cargo.toml`: the optional dependency, `[features] uuid1`, and `[package.metadata.docs.rs]`. `rust/Cargo.lock` changes through `cargo update -p uuid --precise 1.29.0` only.
- Create: `rust/src/uuid1.rs`, holding the five impls in the order the spec gives and a module doc of 2–4 lines.
- Modify: `rust/src/lib.rs`:
  - `#[cfg(feature = "uuid1")] mod uuid1;` after `mod uuid;` (line 134);
  - `#![cfg_attr(docsrs, feature(doc_cfg))]` beside the other crate attributes;
  - one module-doc bullet in the `:7-19` list naming the feature.
- Create: `rust/tests/uuid1_interop.rs`, starting with `#![cfg(feature = "uuid1")]`.

**Interfaces:**
- Consumes: `Uuid::from_bytes`, `Uuid::into_bytes` and `Uuid::as_bytes` (const fns in `rust/src/uuid.rs`).
- Produces: the five impls, which Task 3 calls from `examples/no_panic.rs`.

- [ ] **Step 1: Add the dependency, the feature and the lock bump, and nothing else yet.**
  - `rust/Cargo.toml`: the optional `uuid` dependency and `uuid1 = ["dep:uuid"]`;
  - run `cargo update -p uuid --precise 1.29.0`.

  Do not create `uuid1.rs`.
- [ ] **Step 2: Write the failing tests** in `rust/tests/uuid1_interop.rs`. Name them exactly as below:
  - `round_trips_both_ways`: random v4. `Uuid::from(uuid::Uuid::from(id)) == id`, and the reverse from a `uuid::Uuid::new_v4()`.
  - `nil_and_max_cross_both_ways`: for `[0u8; 16]` and `[0xFF; 16]`, convert both directions and check both `==` directions.
  - `borrow_conversion_keeps_the_original`: `let u: uuid::Uuid = (&id).into();`, then use `id` again and check `u == id` and `id == u`.
  - `display_matches_for_every_layout`: a v7, a v5, and the SQL Server form of that v7 (`v7::to_sql_order(&v7)?`). `id.to_string() == uuid::Uuid::from(id).to_string()`.
  - `v7_order_survives_conversion`: take two `v7::now_v7()?` ids in sequence. Both orders, `a < b` and `uuid::Uuid::from(a) < uuid::Uuid::from(b)`, must hold.
  - `inequality_is_detected`: ids differing in the last byte compare `!=` in both directions.
  - `hashes_differ_today`: `DefaultHasher` over each type gives *different* `finish()`, labelled empirical (corrected during implementation).
- [ ] **Step 3: Run, expecting RED on the traits.**
  ```
  cargo test --features uuid1 --test uuid1_interop
  ```
  Expected: the feature resolves, and the compile FAILS with errors E0277 and E0308, naming the missing `From<…>` and `PartialEq<…>` impls between `hyperuuid::Uuid` and `uuid::Uuid`. An "unknown feature" error, or any other, means Step 1 is wrong: fix it and re-run.
- [ ] **Step 3b: Implement `uuid1.rs` and the `lib.rs` lines,** following the Global Constraints.
- [ ] **Step 4: GREEN.** Run:
  - `cargo test --features uuid1`;
  - `cargo test`, which must not build `uuid1_interop`'s body;
  - `cargo clippy --all-targets --features uuid1 -- -D warnings`;
  - `cargo fmt --check`.

  Expected: all PASS.
  - NC: flip one byte inside one `From` body, and `uuid1_interop` FAILS. Revert.
- [ ] **Step 5: The lock.** `git diff rust/Cargo.lock` shows only uuid's version and checksum lines. `cargo test --locked --features uuid1` passes.
- [ ] **Step 6: docs.rs banners.** In a scratch copy, run:
  ```
  RUSTDOCFLAGS='--cfg docsrs' cargo +nightly doc --features uuid1 --no-deps
  ```
  Count "Available on crate feature `uuid1` only" in `target/doc/hyperuuid/struct.Uuid.html`. Expected: 5.
  - NC: without `--cfg docsrs`, 0.

### Task 3: No-panic coverage and CI wiring

**Files:**
- Modify: `rust/examples/no_panic.rs`:
  - a `#[cfg(feature = "uuid1")]` block placed before `let ptr = out.as_mut_ptr();` (line 61), with the spec's five lines verbatim;
  - in the header comment, a second build line with `--features no-panic,uuid1`.
- Modify: `.github/workflows/ci.yml`, in `check-core` `with:` (lines 200-204), after `no_std_rustflags`:
  ```yaml
  extra_features: uuid1
  no_panic_command: cargo build --release --example no_panic --features no-panic,uuid1
  ```

**Interfaces:**
- Consumes: Task 2's five impls.

- [ ] **Step 1: Add the example block and the ci.yml lines.**
- [ ] **Step 2: Build the example locally** (macOS, link only; the proof itself needs Linux):
  ```
  cargo build --release --example no_panic --features no-panic,uuid1
  ```
  Expected: PASS. Run the same without `uuid1`; expected: PASS, since the block compiles out.
- [ ] **Step 3: The proof, on Linux.** From the repo root:
  ```
  docker run --rm -v "$PWD":/w -w /w/rust rust:1.89 cargo build --release --example no_panic --features no-panic,uuid1
  ```
  Expected: PASS.
  - NC: add `if u.as_bytes()[0] == 0xAB { panic!() }` to one `From` body. The same command must FAIL with `ERROR[no-panic]` naming that function. Revert.
- [ ] **Step 4: Contracts.** Run:
  - `python3 -m unittest discover .github/scripts/tests`;
  - `actionlint .github/workflows/ci.yml`.

  Expected: PASS.

### Task 4: README, CHANGELOG and ROADMAP

**Files:**
- Modify: `rust/README.md`:
  - append "Enable the `uuid1` feature to convert both ways." after the `:90` paragraph;
  - add the new subsection "Interoperating with the `uuid` crate" right after "Why not the `uuid` crate?" (`:82-90`), with the spec's text verbatim.
- Modify: `rust/README.md` "Optional native-extension features" (`:248`). After its first paragraph, add one sentence: "One more optional feature, `uuid1`, adds no entry point: it adds the conversions in [Interoperating with the `uuid` crate](#interoperating-with-the-uuid-crate)."
- Modify: `CHANGELOG.md` `## [Unreleased]`: the spec's two bullets, verbatim.
- Modify: `ROADMAP.md:59`: tick the Rust row.

- [ ] **Step 1: Write the four edits.**
- [ ] **Step 2: Verify the README example by hand.** In a scratch crate with `hyperuuid = { path = ".../rust", features = ["uuid1"] }` and `uuid = "1.29"`, wrap the example in `fn main() -> Result<(), hyperuuid::v7::NewV7Error>`. Expected: it compiles and runs, and `assert_eq!` holds.
  - NC: drop `features = ["uuid1"]`, and it must FAIL to compile, with no `From` impl.
- [ ] **Step 3: Check the snippet keeps the placeholder.** Run `grep -n 'hyperuuid = { version = "<version>", features = \["uuid1"\] }' rust/README.md`. Expected: 1 hit.

### Task 5: Isolation, vendoring and semver sweep

**Files:** none. If a check fails, fix it in the owning file and name that file in the report.

- [ ] **Step 1: Isolation.** For `f` in `"" cdylib staticlib python ruby php uuid1`, run:
  ```
  cargo tree --offline -e normal ${f:+--features $f} | grep -c ' uuid v'
  ```
  Expected: 0 for each, and 1 for `uuid1` (the control).
- [ ] **Step 2: no_std, the compile proof.** Run:
  ```
  RUSTFLAGS='--cfg getrandom_backend="custom"' cargo check --lib --no-default-features --features uuid1 --target thumbv7em-none-eabi
  ```
  Then the same for `--target wasm32-unknown-unknown`. Expected: PASS.
  - NC: a scratch copy whose `uuid1.rs` adds `extern crate std;` and one `std::string::String` conversion must FAIL the thumbv7em check.
- [ ] **Step 3: Dependency isolation on wasm.** Run:
  ```
  cargo tree --no-default-features --features uuid1 -e normal --target wasm32-unknown-unknown | grep -c wasm-bindgen
  ```
  Expected: 0.
  - NC: a scratch copy with the dependency's `features = ["js"]` gives a non-zero count.
- [ ] **Step 4: The R tarball.** Run `r/tools/vendor.sh`. `cargo vendor` there runs without `--versioned-dirs`, so a package sits in `vendor/<name>/` or, for a second version, `vendor/<name>-<version>/`. Count package directories, in both naming forms:
  ```
  tar tf r/src/rust/vendor.tar.xz | grep -cE '^vendor/uuid(-[0-9][^/]*)?/Cargo.toml$'
  ```
  This works with bsdtar and GNU tar. A sanity run with `sha1` in place of `uuid` gives 1 on today's archive.
  Expected: 0, and `git diff r/inst/AUTHORS` is empty.
  - NC: a scratch copy with `"uuid1"` added to `r/src/rust/Cargo.toml`'s features, re-vendored, gives exactly 1.
- [ ] **Step 5: Semver.** Run, against 1.0.0:
  - `cargo semver-checks --baseline-version 1.0.0`;
  - the same with `--features uuid1`.

  Expected: "no semver update required" for both.
  - NC: in a scratch copy, remove `Clone` from `Uuid`'s derive, and it FAILS.
- [ ] **Step 6: The MSRV matrix,** on the final manifest and lock, exactly as the Forge runs it. Run:
  ```
  cargo +1.89 check --locked --lib
  cargo +1.89 check --locked --lib --features uuid1
  cargo +1.89 check --locked --lib --no-default-features --features cdylib
  ```
  Expected: all PASS.
- [ ] **Step 7: Every binding against a freshly built core.** Record in the report which freshly built artifact each suite exercised.
  - **Plain library:** run `cargo cdylib`, then the C#, Java, TypeScript (`npm run build && npm test && npm run test:bun`) and R suites, per CLAUDE.md "Local loop". Go, Swift and PHP go through `.github/scripts/local-core.sh`.
  - **Python:** `python_ext.rs` changed in Task 1. In a venv, from `python/`, run `maturin develop --release`. Assert that `hyperuuid._native.__file__` points into the venv and that its mtime postdates this step's start. Then run `pytest`.
  - **Ruby:** from `ruby/`, run `bundle exec rake native:dev`, then `bundle exec rspec`, which must report `HyperUuid::BACKEND == :native`. Then run `HYPERUUID_PURE=1 bundle exec rspec`, which must report `:fiddle`.

  Expected: every suite PASSES on its intended backend.
