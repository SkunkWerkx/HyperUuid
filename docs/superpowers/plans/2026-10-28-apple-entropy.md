# Apple entropy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On every Apple target the core draws its random bytes from one `CCRandomGenerateBytes` call per request, replacing macOS's `getentropy` syscall per id.

**Architecture:**
- **The arm:** one new `#[cfg(target_vendor = "apple")]` `fill_with` in `rust/src/entropy.rs`; the generic `getrandom::fill` arm loses Apple from its scope.
- **The fork proof:** an example binary, `rust/examples/apple_fork.rs`, forks after advancing the generator, and an integration test, `rust/tests/apple_fork.rs`, supervises it with its own timeout.
- **Receipts:** Criterion before (Task 1, ahead of any edit) and after (Task 4), quoted in the module doc and the CHANGELOG only.

**Tech Stack:** Rust (edition 2024, MSRV 1.89), `libc` 0.2.191, Criterion 0.8.

**Spec:** `docs/superpowers/specs/2026-10-28-apple-entropy-design.md`. It is plan 5 of 5 for Phase 1. It lands after the v7 re-seed (merged, PR #47) and before the system-wide M6 receipts pass (`.superpowers/brainstorm/2026-10-28-phase1/integration.md`, "Order" 4).

## Global Constraints

- **The call:** `libc::CCRandomGenerateBytes(buf.as_mut_ptr().cast(), buf.len())`, once per request of any size. Any return but `libc::kCCSuccess` maps to `getrandom::Error::UNEXPECTED`. No panic, no `assert`, no chunking.
- **The scope:** `target_vendor = "apple"`, which covers macOS, iOS, Mac Catalyst (`target_os = "ios"`), tvOS, watchOS and visionOS. Linux, Android, Windows, WASI and the rest are untouched.
- **Dependencies:** none added. `libc` is already a `cfg(unix)` dependency (`rust/Cargo.toml`, `[target.'cfg(unix)'.dependencies]`). No framework is linked.
- **Persisted bytes:** no corpus change.
- **Docs:** numbers go only in the `entropy.rs` module doc and the CHANGELOG. No README, benchmark table or performance sentence changes here; the receipts pass owns those.
- **Siblings:** none. HyperCast and HyperTabular are frozen until 1.1 ships.
- **Forge:** none.
- **Git:** the owner drives it (CLAUDE.md). Implementers stage nothing and commit nothing; the work stays in the working tree. Before editing, `git fetch` and compare with origin.
- **Checks:** every check runs with its negative control. Copy the flags from CI or the repo aliases (`cargo cdylib`, `cargo test --release`).
- **Formatting:** `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` stay clean in `rust/`, examples and tests included.

## Spec corrections found while planning

1. **The fork helper cannot call `fill`.** `entropy::fill` is `pub(crate)` (`rust/src/entropy.rs:33`), and an example links only the public API. The helper advances the generator with `v4::new_v4()` (16 bytes a draw) and `v7::new_v7_batch` of varying counts (6 bytes an id, one `fill_at` per batch, `v7.rs:466`). Its post-fork draw is four `new_v4()` ids a side: 64 bytes of output, 488 of them random bits. The spec's own 64-byte `fill` test stays in `entropy.rs`'s unit tests (Task 2).
2. **The no-panic proof runs on Linux and Windows, not "Linux-only".** The Forge's `no_panic_runners` default is `["ubuntu-26.04", "windows-latest"]` (`../HyperForge/.github/workflows/hyper-check-core.yml:38-42`), and `ci.yml` doesn't override it. On this Mac the example cannot prove anything: a probe that put `panic!()` in the arm macOS takes still linked cleanly. So the Apple arm is checked by reading, as the spec says.
3. **The negative controls run against the tree as it stands before this plan, not "the parent commit".** Plan 1's work is uncommitted in the tree. The baseline is that tree before Task 2's first edit (Task 1).
4. **Less is CI-only than the spec says.** Two suites run here, so the plan runs them locally (Task 5):
   - Mac Catalyst: `cargo test --target aarch64-apple-ios-macabi --lib` executes natively (probed: the `entropy` tests pass).
   - osx-x64 under Rosetta: this is the same command the Forge's osx-x64 leg runs (`hyper-build-native.yml:1380`).

   Only the iOS simulator and device builds of `test-apple-mobile` stay CI-only.
5. **`cargo test --test apple_fork` alone does not build the example.** Cargo builds examples only for an unfiltered `cargo test` (probed in a scratch crate). The supervising test fails when the helper binary is missing, and never skips. The documented commands are either `cargo test`, or `cargo build --example apple_fork && cargo test --test apple_fork`. CI's `cargo test --release` (`hyper-build-native.yml:511`) and `cargo test --release --target x86_64-apple-darwin` (:1380) are unfiltered, so CI builds it.

## Review Focus

1. **Fork after use.** A child must never repeat its parent's next bytes, even when the parent drew before forking. Pinned by Task 3, with an inherited-state negative control.
2. **One huge request.** A `MAX_BATCH` v7 batch is 67,108,864 × 6 = 402,653,184 random bytes in one call, with no chunking. Pinned by Task 5's `--ignored` run of `a_batch_of_the_whole_counter_space_is_strictly_increasing` (`v7.rs:751`). The existing `a_draw_past_the_hosts_limit_is_filled` (64 MiB + 1) runs on every `cargo test`.
3. **The empty request.** `new_v7_batch(ms, 0, &mut [])` reaches `fill` with a zero-length slice. It must return `Ok`. Pinned by Task 2's `an_empty_draw_succeeds`.
4. **Concurrent threads.** The Linux contention test is `#![cfg(target_os = "linux")]` (`tests/entropy_pool.rs:6`), so nothing covers Apple. Pinned by Task 2's `concurrent_draws_never_repeat`, which is gated off WASI and proved there by Task 5's WASI suite.
5. **x86_64 Macs and Mac Catalyst.** The arm must behave the same under Rosetta and as an iOS-ABI process. Pinned by Task 5's Rosetta and Catalyst runs.

---

### Task 1: The baseline, before any edit

**Files:** none in the repo. Everything goes to the implementer's scratch directory, `$S` below.

**Interfaces:**
- Produces:
  - the Criterion baseline `apple-before` under `rust/target/criterion/`;
  - `$S/before.nm` and `$S/before.otool`;
  - the three "before" point estimates, with machine, OS, rustc and date. Task 4 quotes them.

- [ ] **Step 1: Check the tree.** Run `git fetch && git status -sb`. Expected: `rust/src/entropy.rs` is unmodified (`git diff --quiet rust/src/entropy.rs`). Record `sw_vers -productVersion`, `sysctl -n hw.model` (expect `Mac18,5`), `rustc --version` and the date.
- [ ] **Step 2: Record the symbols.** In `rust/`, run:
  ```
  cargo cdylib
  nm -u target/release/libhyperuuid.dylib > $S/before.nm
  otool -L target/release/libhyperuuid.dylib | tail -n +3 > $S/before.otool
  ```
  Expected: `before.nm` contains `_getentropy` and not `_CCRandomGenerateBytes`. This was checked during planning: the imports were `___error`, `_getentropy` and `_memcpy`. `tail` drops the file's own path and install-name lines.
- [ ] **Step 3: The Criterion baseline.** Run this on mains power, with nothing else heavy running:
  ```
  cargo bench --bench uuid_benchmarks -- --save-baseline apple-before 'single_item/(v4|v7)$|batch_1000/v7_batch$'
  ```
  Expected: exactly three benchmarks run, `single_item/v4`, `single_item/v7` and `batch_1000/v7_batch`; `-- --list` with the same filter shows those three. Record each `time:` middle value. The two singles should be near 600 ns. The batch is per 1000; divide it by 1000 for ns per UUID.

### Task 2: The Apple arm

**Files:**
- Modify: `rust/src/entropy.rs`:
  - the new arm, placed after the generic arm (anchor: the `fn fill_with` whose body is `getrandom::fill(buf)`);
  - that generic arm's `#[cfg(not(any(…)))]`, which gains `target_vendor = "apple"`;
  - two new tests in the crate-level `mod tests`.
- Modify: `rust/Cargo.toml`, the comment above `[target.'cfg(unix)'.dependencies]`. Add that `entropy.rs` calls `CCRandomGenerateBytes` through `libc` on Apple targets. The comment is otherwise stale.

**Interfaces:**
- Consumes: Task 1's `$S/before.nm` and `$S/before.otool`.
- Produces: `fn fill_with(buf: &mut [u8], _unix_millis: Option<u64>) -> Result<(), getrandom::Error>` under `#[cfg(target_vendor = "apple")]`, `#[inline(always)]`, like its siblings.

- [ ] **Step 1: Write the tests** in `mod tests`. They pass on today's backend too; the red for the arm itself is Step 2.
  - `two_64_byte_draws_differ_and_are_not_zero`: two `[0u8; 64]` filled by `fill`. Assert `a != b`, `a != [0; 64]` and `b != [0; 64]`.
  - `an_empty_draw_succeeds`: `fill(&mut [])` is `Ok(())`, and `fill_at(&mut [], 0)` is too. Import `fill_at` beside `fill`.
  - `concurrent_draws_never_repeat`: 8 `std::thread::scope` threads each take 1,000 16-byte draws. The union in a `HashSet<[u8; 16]>` has 8,000 entries. Gate it `#[cfg(not(target_os = "wasi"))]`, the repo's gate for threaded tests (`stream.rs:520`, `:579`, `:686`). wasip1 has no threads, and the Forge leaves that gate to the consumer (`hyper-check-core.yml:405-406`).
- [ ] **Step 2: RED.** `$S/before.nm` lists `_getentropy`; that is the failing state. Run `cargo test --lib entropy::tests`. Expected: PASS on the old backend, which proves the tests compile.
- [ ] **Step 3: Implement the arm.** Its doc comment says, in 3–5 lines:
  - it is the call Rust std uses on every Apple target, and the one `getrandom` already takes on iOS, tvOS, watchOS and Mac Catalyst;
  - Apple seeds it from the kernel;
  - it is in `libSystem`, so nothing new is linked;
  - it is called once per request, because it has no host cap to chunk around.

  The `unsafe` block carries a `// SAFETY:` line: the pointer and length come from one live `&mut [u8]`.
- [ ] **Step 4: GREEN on the symbol.** Run `cargo cdylib`, then `nm -u target/release/libhyperuuid.dylib`. Expected:
  - it lists `_CCRandomGenerateBytes`, and no `_getentropy`;
  - `diff $S/before.otool <(otool -L target/release/libhyperuuid.dylib | tail -n +3)` is empty: the link line is unchanged.
  - NC: `$S/before.nm` (Task 1) lists `_getentropy`.
- [ ] **Step 5: GREEN on the tests.** Run, in `rust/`:
  - `cargo test`;
  - `cargo test --release`, which is CI's form;
  - `cargo clippy --all-targets -- -D warnings`;
  - `cargo fmt --check`.

  Expected: all PASS, with the corpus and the v7 stream tests unchanged.
- [ ] **Step 6: NC for the 64-byte test.** In a scratch copy (`rsync -a --exclude target rust corpus $S/nc-zero/`), make the Apple arm `buf.fill(0); Ok(())`. Then run, from there, `cargo test --lib two_64_byte_draws_differ_and_are_not_zero`. Expected: FAIL. The repo is untouched.

### Task 3: The fork helper and its supervisor

**Files:**
- Create: `rust/examples/apple_fork.rs`.
- Create: `rust/tests/apple_fork.rs`, starting with `#![cfg(target_vendor = "apple")]`.

**Interfaces:**
- Consumes: the public `hyperuuid::v4::new_v4`, `hyperuuid::v7::new_v7_batch` and `libc` (`pipe`, `fork`, `poll`, `read`, `write`, `waitpid` with `WNOHANG`, `kill`, `killpg`, `_exit`).
- Produces: the helper's exit codes, which the supervisor reads:
  - 0: the draws differ;
  - 1: the draws are equal;
  - 2: a 5 s bound expired;
  - 3: any other error.

  Each non-zero exit prints one line to stderr.

- [ ] **Step 1: Write the supervisor test,** `a_forked_child_never_repeats_its_parent`.
  - It finds the helper at `current_exe()`'s grandparent `/examples/apple_fork`. If the helper is missing, it panics with a message naming `cargo build --example apple_fork`.
  - It spawns the helper with stderr piped.
  - It spawns the helper in its own process group, with `std::os::unix::process::CommandExt::process_group(0)`, so the group id is the helper's pid.
  - It polls `try_wait` every 50 ms for at most 15 s. On expiry it calls `libc::killpg(pgid, SIGKILL)`, which takes the forked child down with the helper, then reaps the helper and panics with "helper did not finish in 15 s". `Child::kill()` alone would leave the child alive, still holding the inherited stderr pipe; a scratch probe on this Mac showed that, and showed `killpg` removes both.
  - It asserts exit code 0, printing the helper's stderr on failure.

  The header comment states why the test runs in a separate process: a hang or a crash takes the helper down, not the test runner.
- [ ] **Step 2: Run it, expecting RED.** Run `cargo test --test apple_fork`. Expected: FAIL with the missing-helper message.
- [ ] **Step 3: Write the helper.**
  - On a non-Apple target, `main` prints that the helper is Apple-only and exits 0. It still compiles under `cargo test` everywhere, wasm32-wasip1 included.
  - The Apple `main` first advances the generator in the parent:
    - three `new_v4()` calls;
    - then `new_v7_batch` with counts 1, 37 and 1000, into one 16,000-byte buffer at a fixed `unix_millis` of `1_700_000_000_000`.
  - Then it creates the pipe and forks.
  - The child takes four `new_v4()` ids, writes their 64 bytes to the pipe, and calls `_exit(0)`, or `_exit(3)` on error.
  - The parent takes its own four ids. Then it `poll`s the read end for at most 5,000 ms, and reads exactly 64 bytes.
  - It reaps the child with `waitpid(WNOHANG)` in a loop, within the same 5 s budget. When the budget runs out, whether in the `poll` or in the reap, it sends the child `SIGKILL`, reaps it with a blocking `waitpid`, and only then exits 2.
  - It exits 1 if the 64 bytes are equal, and 0 otherwise.
- [ ] **Step 4: GREEN.** Run, in `rust/`:
  - `cargo build --example apple_fork && cargo test --test apple_fork`;
  - `cargo test`;
  - `cargo test --release`.

  Expected: PASS, in well under a second.
- [ ] **Step 5: NC for the comparison: an inherited, unreseeded generator.** In a scratch copy, `$S/nc-fork/` (`rsync -a --exclude target rust corpus`):
  - add `target_vendor = "apple"` to the `chacha20` target table's `cfg(any(…))` in `Cargo.toml`;
  - make the Apple arm a `static mut` `Option<chacha20::ChaCha20>`, keyed once from `CCRandomGenerateBytes` on first use and never re-keyed. It zeroes `buf` and applies the keystream.

  Then run `cargo build --example apple_fork && cargo test --test apple_fork`. Expected:
  - FAIL with exit code 1: the child replays the parent's next 64 bytes;
  - the same copy with the parent's pre-fork draws deleted still PASSES. This shows why the helper advances before forking.
- [ ] **Step 6: NC for the helper's bound.** In the same scratch copy, with the arm restored, add `std::thread::sleep(10 s)` in the child before its write. Expected: FAIL with exit code 2 after about 5 s, and no `apple_fork` process is left (`pgrep apple_fork` is empty).
- [ ] **Step 7: NC for the supervisor's bound.** In the same scratch copy, add `std::thread::sleep(30 s)` at the top of the Apple `main`. Expected: FAIL with "helper did not finish in 15 s" after about 15 s, and `pgrep apple_fork` finds nothing afterwards.
- [ ] **Step 7b: NC for a hang after the fork.** In the same scratch copy, with Step 7's sleep removed, make two edits. Add `std::thread::sleep(60 s)` in the parent right after `fork` returns, before its own draws and the `poll`. Add `std::thread::sleep(60 s)` in the child before its write. Have the helper print both pids to stderr first. Only a parent-side hang reaches the supervisor with the child still alive; a child-only hang is Step 6, where the helper's own bound kills the child. Expected:
  - FAIL with "helper did not finish in 15 s" after about 15 s;
  - afterwards, `kill(pid, 0)` returns `ESRCH` for both pids, and `pgrep -g <helper pid>` is empty.
  - The control's own control: with the supervisor's `killpg` replaced by `Child::kill()`, the child's pid is still alive afterwards. That is the leak this step exists to catch.

### Task 4: The receipt, the module doc and the shared docs

**Files:**
- Modify: `rust/src/entropy.rs`, the module header.
- Modify: `CHANGELOG.md`, `## [Unreleased]` → `### Changed`. The new bullet goes at the end of that list, after the bullet that ends `need \`rustc\` 1.89. *(Rust, R)*` and before `### Fixed`.
- Modify: `CLAUDE.md`, "Open follow-ups": the bullet that begins "- Core: the user-space ChaCha20 entropy path".
- Modify: `docs/superpowers/specs/2026-10-28-java21-jni-design.md`. This is the integrator's strike, named in the spec's Files. Two edits:
  - drop the sub-bullet "adds one sentence: on macOS, the core calls `getentropy` for every id, which dominates single v4 and v7 calls;";
  - drop the audit row that begins "| M6 calls may lose to `UUID.randomUUID()` |". Its "removes the per-id `getentropy`" claim is now done, not pending.

**Interfaces:**
- Consumes: Task 1's baseline and "before" values; Tasks 2–3 complete, so the measured code is final.

- [ ] **Step 1: The "after" run,** in the same session and conditions as Task 1:
  ```
  cargo bench --bench uuid_benchmarks -- --baseline apple-before 'single_item/(v4|v7)$|batch_1000/v7_batch$'
  ```
  Expected:
  - both singles are under 100 ns, and Criterion reports "Performance has improved";
  - the batch falls too.

  If a single is not under 100 ns, stop and report; do not write numbers. The NC is Task 1's baseline.
- [ ] **Step 2: The module header.**
  - In the first sentence, insert "on Apple platforms, where it is `CCRandomGenerateBytes` called directly," before "on Windows".
  - Replace "macOS has not been measured yet." with one sentence giving machine, OS, date, and the before → after values for single `new_v4`, single `new_v7`, and `new_v7_batch` of 1000 in ns per UUID. Start it with "On an Apple M6 (Mac18,5, macOS 27.0.1, ⟨date⟩), `CCRandomGenerateBytes` against `getentropy`: …".
  - Check that "The other targets keep `getrandom`" no longer reads as covering Apple. If it does, change it to "Windows and WASI keep `getrandom`".
- [ ] **Step 3: The CHANGELOG bullet.** Use the spec's text verbatim. Fill ⟨before⟩ and ⟨after⟩ with Task 1's and Step 1's `single_item/v7`, rounded to whole ns.
- [ ] **Step 4: The CLAUDE.md bullet.** Replace the whole three-line bullet with the spec's text: "Core: the ChaCha20 pool is Linux and Android only. Apple platforms take `CCRandomGenerateBytes`, Windows `ProcessPrng`, and the rest `getrandom`."
- [ ] **Step 5: The Java spec strike,** the two edits under Files.
- [ ] **Step 6: Check the docs.** Run these greps:
  - `grep -n 'not been measured' rust/src/entropy.rs`: expected, no hits;
  - `grep -n 'getentropy per id' CLAUDE.md`: no hits;
  - `grep -n 'dominates single' docs/superpowers/specs/2026-10-28-java21-jni-design.md`: no hits.

  Leave `README.md:78`, `README.md:365` and `typescript/README.md:310-311` as they are. Their per-id `getentropy` sentences belong to the receipts pass (spec, "Contract changes"). The report lists them as stale until then.

### Task 5: The platform sweep and every binding

**Files:** none. If a check fails, fix it in the owning file and name that file in the report.

- [ ] **Step 1: The big request.** Run `cargo test --release -- --ignored a_batch_of_the_whole_counter_space_is_strictly_increasing`. Expected: PASS, with one 402,653,184-byte call.
- [ ] **Step 2: no_std.** Run:
  ```
  cargo check --lib --no-default-features --target aarch64-apple-darwin
  cargo check --lib --no-default-features --target aarch64-apple-ios
  cargo check --lib --no-default-features --target aarch64-apple-ios-macabi
  RUSTFLAGS='--cfg getrandom_backend="custom"' cargo check --lib --no-default-features --target thumbv7em-none-eabi
  ```
  Expected: all PASS. These targets are installed on this Mac.
  - NC: a scratch copy whose Apple arm names `std::vec::Vec` must FAIL the darwin check.
- [ ] **Step 2b: The WASI suite,** as the Forge's `test-wasi` runs it (`hyper-check-core.yml:429-436`, on by default through `wasi_tests`, :73-77; it runs on ubuntu-26.04 with wasmtime 50.0.0). Here use the same wasmtime 50.0.0 (`~/.wasmtime/bin`). In `rust/`, run:
  ```
  target=$(cargo metadata --no-deps --format-version 1 | jq -r .target_directory)
  CARGO_TARGET_WASM32_WASIP1_RUNNER="wasmtime run --dir $(git rev-parse --show-toplevel) --dir $target" cargo test --target wasm32-wasip1
  ```
  Expected: PASS, with `concurrent_draws_never_repeat` compiled out. `apple_fork`'s stub `main` builds, and its test is compiled out.
  - NC: in a scratch copy with that test's gate removed, the same command must abort with "failed to spawn thread" ("not supported").
- [ ] **Step 3: Rosetta and Catalyst.** Run:
  - `cargo test --release --target x86_64-apple-darwin`, the Forge osx-x64 leg's command;
  - `cargo test --target aarch64-apple-ios-macabi`.

  Expected: both PASS, including `apple_fork`. Both commands are unfiltered, so they build the example for their own target.
- [ ] **Step 4: The Swift archive.** Run `.github/scripts/local-core.sh`, then:
  ```
  nm -u rust/target/local-core/swift/HyperUuidCore.artifactbundle/arm64-apple-macosx/libhyperuuid.a | grep -E '_getentropy|_CCRandomGenerateBytes'
  ```
  Expected: only `_CCRandomGenerateBytes`.
  - NC: the same `nm` on the committed `swift/HyperUuidCore.artifactbundle/arm64-apple-macosx/libhyperuuid.a` lists `_getentropy`; this was checked during planning.
- [ ] **Step 5: Every binding against a freshly built core.** Record in the report which freshly built artifact each suite exercised.
  - **Plain library:** run `cargo cdylib`, then the C#, Java, TypeScript (`npm ci && npm run build && npm test && npm run test:bun`) and R suites, per CLAUDE.md "Local loop". R needs `r/tools/vendor.sh` first; `git diff r/inst/AUTHORS` must be empty. Go (`go test -count=1 -tags hyperuuid_local ./...`), Swift (`HYPERUUID_LOCAL_CORE=1 swift test`) and PHP (`HYPERUUID_NATIVE_LIBRARY`) run on Step 4's local core.
  - **Python:** in a venv, from `python/`, run `maturin develop --release`. Assert that `hyperuuid._native.__file__` points into the venv and that its mtime postdates this step's start. Then run `pytest`.
  - **Ruby:** from `ruby/`, run `bundle exec rake native:dev`, then `bundle exec rspec`, which must report `HyperUuid::BACKEND == :native`. Then run `HYPERUUID_PURE=1 bundle exec rspec`, which must report `:fiddle`.

  Expected: every suite PASSES on its intended backend.
- [ ] **Step 6: What stays CI-only.** The report names:
  - `test-apple-mobile`: the iOS simulator and device builds, on the xcframework the Forge's `build-static` makes;
  - the osx-arm64 and osx-x64 `build-native` legs, as the confirming runs;
  - the no-panic proof on Linux and Windows, which does not reach this arm (correction 2).
