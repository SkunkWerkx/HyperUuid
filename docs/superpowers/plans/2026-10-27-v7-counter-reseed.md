# v7 Counter Re-seed Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the never-reset v7 counter with a per-millisecond re-seeded, clamped,
drift-bounded stream (RFC 9562 §6.2 Method 1), and carry its new `CounterExhausted` error and
ordering contract through all nine bindings.

**Architecture:**
- The transition arithmetic is a pure function over a packed `u64` state, in a new private
  module `rust/src/stream.rs`. Deterministic tests drive it directly, with no globals.
- A CAS loop applies the function to an `&AtomicU64`. Tests pass their own atomic, so nothing
  ever touches the process-wide `STREAM`.
- `v7.rs` draws entropy first, reserves, then writes.
- Targets without 64-bit atomics compile today's counter unchanged.

**Tech Stack:** Rust 2024 (`no_std`, `no-panic`), plus each binding's own toolchain.

**Spec:** `docs/superpowers/specs/2026-10-27-v7-counter-reseed-design.md`. Read it first. Every
transition, bound and test value below comes from it.

## Global Constraints

- **No git.** The owner drives git. Wherever a task template would say "commit", leave the work
  in the working tree, verified. Run `git fetch` and compare with origin before the first edit.
- **The corpus is untouched.** No file in `corpus/` changes, and `python3 corpus/oracle.py` must
  still pass as it does now.
- **No new C export, no new dependency.** `check_exports.py` and the export counts stay as they
  are.
- **Constants, in `rust/src/stream.rs`:**

  ```rust
  pub(crate) const STEPS: u32 = 1 << 20;
  pub(crate) const SEED_MAX: u32 = (1 << 19) - 1;
  pub(crate) const STREAM_MAX_MILLIS: u64 = (1 << 44) - 2;
  pub(crate) const CLAMP_WINDOW_MS: u64 = 11_000;
  pub(crate) const DRIFT_LIMIT_MS: u64 = 1_000;
  ```

  W = `CLAMP_WINDOW_MS` and D = `DRIFT_LIMIT_MS`. The guarantee window is G = W − D =
  10,000 ms.
- **Encoding:** state = `(ms + 1) << 20 | last_step`, and 0 means unseeded. Each UUID's
  `counter26 = step << 6 | low6`.
- **Seed:** `seed(k, r) = ((r as u64 * (min(SEED_MAX, STEPS − k) as u64 + 1)) >> 32) as u32`.
- **New error:** `NewV7Error::CounterExhausted`, C ABI return code `6`. Its exact `Display` text,
  which every binding copies verbatim (Go and Swift prefix their usual `hyperuuid: `):
  `version 7 counter exhausted: the stream cannot advance without running more than 1000 ms
  ahead of the supplied timestamp or past its last millisecond`
- **Binding error kinds:** the code 6 error is a state error, not an argument error. Exact types
  are in each binding's task.
- **Wording:** README text describes the current state only, and history goes in the CHANGELOG
  (plain Unreleased entries). The same contract wording goes into all nine bindings.
- **Formatters** cover src, tests, benchmarks and smoke tests in every language. Run each
  binding's formatter or linter over what you touched.

## Review Focus

1. **Out-of-stream callers in a process whose stream was pushed far ahead.** After one call at
   `STREAM_MAX_MILLIS` (or any far-future time), real-time calls are minted out of stream. They
   must still succeed, be valid v7s carrying their own timestamp, and leave the state alone.
   Task 3 pins this with `real_time_calls_after_a_future_dated_call_still_succeed`.
2. **A failure reserves nothing [plan review].**
   - Every failure leaves the stream state word unchanged.
   - Validation failures (input `TimestampOutOfRange`, `BatchTooLarge`, `BufferTooSmall`) also
     leave the caller's buffer untouched.
   - `Random` and `CounterExhausted` may clobber the front of the buffer, because the batch's
     entropy is drawn into `out` before the reservation. Extend the existing "treat the buffer
     as clobbered" sentence on `new_v7_batch` (`v7.rs:167`) to cover `CounterExhausted`.
     Keeping the stronger promise would need a scratch buffer, which this crate deliberately
     has none of.
   - Task 3 pins all of this with `v7_entropy_failure_reserves_nothing` and
     `v7_exhaustion_reserves_nothing`, through the injectable `*_in` paths.
3. **The exhaustion error reaches every caller as itself, never as "random source failure".**
   C#, Java, Go, PHP, Ruby (Fiddle) and Swift all have a default arm that labels any unknown
   code a random-source failure. Each binding task pins code 6 explicitly, at both the single
   and the batch site.
4. **Process-global poisoning in test suites.** A binding test that drives the real stream to
   the ceiling makes later v7 tests in the same process mint out of stream. Every end-to-end
   exhaustion test runs in its own process, as each task says, and core tests never touch
   `STREAM`.
5. **The no-64-bit-atomics fallback still compiles and behaves as today.** A 64-bit host would
   compile it out. So the fallback's `counter()` and its wrap rule are compiled under
   `cfg(any(test, not(target_has_atomic = "64")))`, which keeps their existing wrap tests
   running on the host. The `thumbv7em` check proves the fallback compiles as the real path
   (Task 3).

---

### Task 0: The `CounterExhausted` variant (prerequisite) [plan review]

The variant has to exist before Task 1's arithmetic can return it. Inside the defining crate,
`#[non_exhaustive]` doesn't relax exhaustive matches, so every in-crate match on `NewV7Error`
gains its arm here, in one step.

**Files:**
- `rust/src/v7.rs`: add `CounterExhausted` (documented) and its `Display` text from Global
  Constraints.
- `rust/src/ffi.rs`: `CounterExhausted => 6`, and the module doc's code list.
- `rust/src/python_ext.rs`: `PyRuntimeError::new_err(e.to_string())`.
- `rust/src/php_ext.rs`: `PhpException::from_message(e.to_string())`, the same generic
  exception its other v7 arms throw (`php_ext.rs:56–62`) [plan review 2]. The extension door
  raises no exception classes for any v7 error today. The `CounterExhaustedException` class is
  thrown by the FFI `Runtime.php` path (Task 7), matching how `TimestampOutOfRangeException` is
  split between the two doors.
- `rust/src/ruby_ext.rs`: `HyperUuid::CounterExhaustedError`, through `const_get` as at
  line 235.
- `ruby/lib/hyperuuid/errors.rb`: `class CounterExhaustedError < StandardError; end`.
- `php/src/CounterExhaustedException.php`: `final class … extends \RuntimeException`.

TypeScript's crate is outside the core, so `#[non_exhaustive]` applies there and its `_` arm
still compiles. Task 8 adds its explicit arm.

- [ ] **Step 1:** Add `counter_exhausted_display_text` to `rust/src/tests.rs`. It asserts
  `NewV7Error::CounterExhausted.to_string()` equals the exact message.
- [ ] **Step 2:** Run `cargo test --lib counter_exhausted_display_text`. Expected: FAIL (no such
  variant).
- [ ] **Step 3:** Add the variant and every arm listed above.
- [ ] **Step 4:** Run `cargo test --lib counter_exhausted_display_text`, then `cargo check`
  plus each feature build CLAUDE.md names: the `ruby-ext` and `php-ext` aliases, and maturin in
  `python/`. Each must compile.

### Task 1: Pure stream arithmetic (`rust/src/stream.rs`)

**Files:**
- Create: `rust/src/stream.rs` (module doc, consts, functions, and an inline
  `#[cfg(test)] mod tests` in the `entropy.rs` style)
- Modify: `rust/src/lib.rs`, adding `#[cfg(target_has_atomic = "64")] mod stream;` beside
  `mod entropy;`

**Interfaces:**
- Produces:

  ```rust
  pub(crate) fn encode(ms: u64, last_step: u32) -> u64;           // (ms + 1) << 20 | last_step
  pub(crate) fn decode(state: u64) -> Option<(u64, u32)>;         // None for 0
  pub(crate) fn seed(k: u32, r: u32) -> u32;
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub(crate) struct Reservation { pub ms: u64, pub first_step: u32, pub next: Option<u64> }
  pub(crate) fn plan(state: u64, m: u64, k: u32, r: u32) -> Result<Reservation, NewV7Error>;
  ```

  `k` is the number of steps, 1 ≤ k ≤ `STEPS`. Step j of the reservation (0 ≤ j < k) is
  `first_step + j` at `ms` while that is below `STEPS`. Past that it is `first_step + j − STEPS`
  at `ms + 1`: the split tail. `next` is `None` for an out-of-stream call (no CAS).

- [ ] **Step 1: Write the failing tests** in `stream.rs`'s test module, one `#[test]` each, with
  the spec's values.
  - `seed_bounds`:
    - `seed(1, u32::MAX) == SEED_MAX`
    - `seed(1, 0) == 0`
    - `seed(STEPS, u32::MAX) == 0`
    - `seed(k, u32::MAX) + k <= STEPS` for k ∈ {1, 2, 1000, STEPS / 2 + 1, STEPS − 1, STEPS}
  - `encode_decode_round_trip`: (0, 0) and (`STREAM_MAX_MILLIS`, `STEPS − 1`) round-trip;
    `decode(0) == None`; `encode(0, 0) != 0`.
  - `advance_from_unseeded_reserves_the_whole_batch`: `plan(0, m, 2, r)` with `seed(2, r) == s`
    gives `Reservation { ms: m, first_step: s, next: Some(encode(m, s + 1)) }`. This is the
    65-item case.
  - `advance_from_an_older_ms`: state `encode(m − 5, 7)`, k = 1, gives first step `seed(1, r)`
    at m.
  - `clamp_fits_at_lags_0_g_and_w`: for lag ∈ {0, 10_000, 11_000}, state `encode(L, 9)` with
    m = L − lag and k = 3 gives `{ ms: L, first_step: 10, next: Some(encode(L, 12)) }`.
  - `lag_past_w_is_out_of_stream`: lag 11_001 gives `ms == m`, `first_step == seed(k, r)`,
    `next == None`.
  - `past_stream_max_is_out_of_stream`: m ∈ {`STREAM_MAX_MILLIS + 1`, `MAX_UNIX_MILLIS`} with
    k = `STEPS` gives first step 0 and `next == None`, from both unseeded and seeded states.
  - `residue_counterexamples_classify_correctly` (review 1):
    - state `encode((1 << 38) + 100, 0)` with m = 100 is out of stream;
    - state `encode(100, 0)` with m = `(1 << 38) + 99` is an advance.
  - `split_from_step_2_pow_20_minus_2`: state `encode(L, STEPS − 2)` with m = L and k = `STEPS`
    gives `first_step == STEPS − 1` at L, and `next == Some(encode(L + 1, STEPS − 2))`.
  - `split_from_an_exhausted_ms`: state `encode(L, STEPS − 1)` with k = `STEPS` gives
    `first_step == STEPS`, so the head is empty, and `next == Some(encode(L + 1, STEPS − 1))`.
  - `drift_bound`:
    - state `encode(m + DRIFT_LIMIT_MS − 1, STEPS − 1)` with k = 1 at m succeeds and lands at
      `ms + 1 == m + DRIFT_LIMIT_MS`;
    - state `encode(m + DRIFT_LIMIT_MS, STEPS − 1)` gives `Err(CounterExhausted)`.
  - `ceiling`: state `encode(STREAM_MAX_MILLIS, STEPS − 1)` with k = 1, and state
    `encode(STREAM_MAX_MILLIS, 5)` with k = `STEPS − 5`, both give `Err(CounterExhausted)`.
  - `repeated_max_batch_at_one_ms_is_ordered_until_exhausted` (review 1): from unseeded, apply
    `plan` repeatedly at a fixed m with k = `STEPS`, feeding `next` back in. Every reservation's
    (ms, first step) is strictly after the previous one's last step. The loop ends in
    `CounterExhausted` after exactly `DRIFT_LIMIT_MS` splits plus the first advance.
  - `capacity_counts_the_seeding_call`: from unseeded with `r = u32::MAX`, `SEED_MAX + 2` calls
    (that is, 2^19 + 1) with k = 1 at m end at `encode(m, STEPS − 1)`. The next call splits.
    Then the same from `encode(m, SEED_MAX)`, where 2^19 calls end there.
  - `state_only_grows`: a seeded xorshift drives 100,000 random (m, k, r) operations around a
    moving clock with all three bands. Each `next` is strictly greater than the state it came
    from. Each `Err` leaves the loop's state unchanged.

- [ ] **Step 2: Run the tests and confirm they fail.**
  Run `cargo test --lib stream::` in `rust/`. Expected: compile errors (functions not defined).

- [ ] **Step 3: Implement** the consts, `encode`, `decode`, `seed` and `plan` exactly as the
  spec's transitions table gives them.
  - The split checks (drift, then ceiling) happen before building `next`.
  - Use checked or saturating arithmetic only where an input could overflow. `m` is at most
    `MAX_UNIX_MILLIS` (callers validate it first), so `m + DRIFT_LIMIT_MS` cannot overflow.
  - Mark everything `#[inline]`. Nothing here may contain a panic path: no indexing, no `/`,
    no `unwrap`.

- [ ] **Step 4: Run the tests and confirm they pass.** `cargo test --lib stream::`: all pass.

- [ ] **Step 5: Run the negative controls.**
  - Temporarily change `encode`/`decode` to keep only the low 38 bits of ms: the
    residue-counterexample test fails.
  - Temporarily make an advance install `encode(m, first_step)`: the advance test fails.
  - Temporarily drop the ceiling check: `ceiling` fails.
  - Temporarily give split tails a seed: `split_from_step_2_pow_20_minus_2` fails.
  - Revert each change.

### Task 2: CAS reservation and the concurrency oracle

**Files:**
- Modify: `rust/src/stream.rs`
- Test: `rust/src/stream.rs` test module

**Interfaces:**
- Consumes: `plan`, `Reservation` (Task 1).
- Produces:

  ```rust
  pub(crate) fn reserve(stream: &AtomicU64, m: u64, k: u32, r: u32) -> Result<Reservation, NewV7Error>;
  ```

  The loop is: `load(Relaxed)`, then `plan`, and on `next == Some(n)` a
  `compare_exchange_weak(state, n, Relaxed, Relaxed)`, retrying on failure with the same `r`.

- [ ] **Step 1: Write the failing tests.**
  - `reserve_returns_errors_without_touching_the_atomic`: seed an `AtomicU64` at the ceiling
    state. `reserve` returns `CounterExhausted`, and `load()` is unchanged.
  - `max_completed_oracle` and `latest_returned_oracle_misses_the_injected_violation` (review 4):
    a small `struct MaxOracle(Mutex<u128>)` with `before() -> u128` and `after(last: u128)`,
    where `after` stores `max`.
    - Feed completions 200 and 100, then call `before()`, then a result of 150. `MaxOracle`
      rejects it, because 150 is not greater than 200.
    - A `LatestOracle`, which stores the last value instead, accepts it.
  - `concurrent_reservations_are_ordered_by_reservation`, `#[cfg(not(target_os = "wasi"))]`:
    - one local `AtomicU64`;
    - 8 threads running 2,000 **coordinated rounds** [plan review];
    - round t has a fixed clock T = T0 + t that never moves during the round, so every m stays
      within G of S: m ∈ [T − 10_000, T] and S ≤ T;
    - a `Barrier` starts each round and another ends it, and the clock advances only between
      rounds;
    - each thread does 10 reservations per round;
    - each reservation has k ∈ {1, 2, 64}, m = T − jitter with jitter ∈ [0, 10_000], and r
      from a per-thread xorshift;
    - each reservation's sort keys are its first and last step as u128s, built as
      `(ms as u128) << 20 | step`, with the tail rule applied;
    - **the order of each call [plan review]:**
      `let floor = oracle.before();`, then `reserve(...)`, then
      `assert!(first_key > floor)`, then `oracle.after(last_key)`. The floor is captured
      *before* `reserve`, never after;
    - each thread asserts its own keys strictly increase;
    - finally every key is distinct;
    - completion order is never asserted.
  - `out_of_window_lags_single_threaded`: lags in (G, W] are clamped and lags above W are out of
    stream, against a local atomic.

- [ ] **Step 2: Run them and confirm they fail.** `cargo test --lib stream::`. Expected:
  `reserve` is not defined.

- [ ] **Step 3: Implement `reserve`.**

- [ ] **Step 4: Run them and confirm they pass.** `cargo test --lib stream::`.

### Task 3: Wire the stream into `v7.rs`, `ffi.rs` and the core tests

**Files:**
- Modify: `rust/src/v7.rs`:
  - `static STREAM: AtomicU64` and the singles and batch paths, under
    `cfg(target_has_atomic = "64")`;
  - `counter()`, `COUNTER`, `SEED_CLAIMED` and today's wrap rule, unchanged. They are used
    under `cfg(not(target_has_atomic = "64"))` and compiled under
    `cfg(any(test, not(target_has_atomic = "64")))`, so their existing tests still run on the
    host;
  - `fill_batch` generalised;
  - the doc comments around lines 57–90, 175–188, and on `MAX_BATCH`.
- Modify: `rust/src/ffi.rs`: the comment near line 225. Code `6` itself is mapped in Task 0.
- Modify: `rust/src/lib.rs`, the line 27–28 comment.
- Modify: `rust/src/tests.rs`: rewrite `v7_batch_continues_the_same_counter_sequence_as_individual_calls`
  and any test that assumes +1 counters between calls; add the Review Focus tests.
- Modify: `rust/tests/v7_counter_race.rs`. Its module doc and assertions describe the seed-add
  race. Keep it as the first-callers, process-global test of the *stream*: 8 threads at one
  fixed ms, all results distinct, and per-thread strictly increasing.
- Modify: `.github/scripts/cdylib_smoke.py`, adding an exhaustion check. It runs in its own
  process.
- Modify: `rust/examples/no_panic.rs`, only if its call list needs the new path. Read it first.

**Interfaces:**
- Consumes: `reserve`, `seed`, `STEPS` (Tasks 1–2); `CounterExhausted` and code 6 (Task 0).
- Produces:
  - `NewV7Error::CounterExhausted`;
  - C ABI code `6` from `uuid_new_v7` and `uuid_new_v7_batch`;
  - the generalised
    `fn fill_batch(m: u64, count: u32, out: &mut [u8], item: impl Fn(u32) -> (u64 /* ms */, u32 /* counter26 */)) -> Result<(), NewV7Error>`.
    It stays `#[inline(always)]` and keeps the existing reverse in-place walk. The entropy
    draw moves out to the caller, because the stream path needs `r` before it reserves. The stream path maps
    item i to step `first_step + i / 64`, with the tail rule from Task 1, and
    `low6 = i % 64`. The fallback path passes today's wrap rule.

**Injectable paths [plan review].** The public functions become thin wrappers over:

```rust
pub(crate) fn new_v7_in(stream: &AtomicU64, m: u64,
    fill: impl FnOnce(&mut [u8], u64) -> Result<(), getrandom::Error>) -> Result<Uuid, NewV7Error>;
pub(crate) fn new_v7_batch_in(stream: &AtomicU64, m: u64, count: u32, out: &mut [u8],
    fill: impl FnOnce(&mut [u8], u64) -> Result<(), getrandom::Error>) -> Result<(), NewV7Error>;
```

`new_v7` and `new_v7_batch` call them with `&STREAM` and `crate::entropy::fill_at`. Unit tests
pass a local atomic and a fake `fill`, so no unit test ever touches `STREAM`.

**Ordering inside `new_v7_in` and `new_v7_batch_in`:**
1. Validate `unix_millis` (`TimestampOutOfRange`), `count` and the buffer, as today.
2. Draw entropy **through the supplied `fill` parameter** [plan review 2]. Never call
   `entropy::fill_at` directly here.
   - single: `fill(&mut bytes[..], m)` fills all 16 bytes. `r` comes from `bytes[0..4]` and
     `low6` from `bytes[4] & 0x3F`, before `write_fields` overwrites octets 0–9.
   - batch: `fill(&mut out[..count * 6 + 4], m)`. Read `r` from the 4 bytes after the packed
     entropy *before* the reverse walk.
3. `reserve(stream, m, k, r)` on the **supplied `stream` parameter**, never `&STREAM`, with
   k = 1 or ⌈count / 64⌉. Only the public wrappers name `STREAM` and `fill_at`.
4. Write.

- [ ] **Step 1: Write the failing tests** in `rust/src/tests.rs`. All of them use `new_v7_in` and
  `new_v7_batch_in` with a local `AtomicU64` and a fake `fill`.
  - `v7_single_after_a_batch_sorts_after_its_last_item`: a batch of 65 at m, then a single at m,
    which must be greater than `batch[64]`. This test targets the *incomplete-reservation
    mutation*: an advance that installs `encode(m, first_step)`. It is not an old-code control.
  - **Old-code wrap pair [plan review]**, deterministic:
    - `fallback_wrap_rule_tail_runs_ahead_of_the_next_call`: the fallback batch rule (compiled
      under `cfg(any(test, …))`) is handed a block straddling the 26-bit wrap at m, followed by
      a single from the counter just past it. It asserts the single sorts *before* the batch's
      rolled tail, the documented old caveat, so it passes on today's rule.
    - `stream_tail_never_runs_ahead_of_the_next_call`: the same shape through the stream. A
      local atomic at `encode(m, STEPS − 2)`, then a `MAX_BATCH`, then a single at m. The single
      sorts after the batch's last item.

    Together they show the old rule reverses deterministically and the new one doesn't.
  - `v7_entropy_failure_reserves_nothing`: a local atomic at `encode(m, 9)`, and a `fill` that
    returns `Err(getrandom::Error::UNSUPPORTED)`, or the crate's equivalent constant. Both the
    single and the batch return `Random`. The atomic still reads `encode(m, 9)`.
  - `v7_exhaustion_reserves_nothing`: a local atomic at `encode(STREAM_MAX_MILLIS, STEPS − 1)`,
    for a single and for a 65-item batch at `STREAM_MAX_MILLIS`. Each returns
    `CounterExhausted` and the atomic is unchanged. The buffer is not asserted, because it may
    hold entropy.
  - `v7_validation_failures_leave_the_buffer_untouched`: `BufferTooSmall`, `BatchTooLarge` and
    an input `TimestampOutOfRange` each leave the buffer and the atomic unchanged. The fake
    `fill` panics if it is called.
  - `real_time_calls_after_a_future_dated_call_still_succeed`, in a new integration test file
    `rust/tests/v7_far_future.rs`, gated with `#![cfg(not(target_arch = "wasm32"))]` to match
    `now_v7`'s own gate (`v7.rs:288`) **[plan review]**. Without the gate the Forge's
    `wasm32-wasip1` test run fails to compile. Its own process: one `new_v7((1 << 44) - 2)`,
    then 1,000 `now_v7()` calls. Every one is `Ok`, version 7, with `v7::unix_millis` within
    1 s of the wall clock.
  - `v7_counter_exhaustion_through_the_public_api`, in `rust/tests/v7_exhaustion.rs` (its own
    process):
    - `new_v7((1 << 44) - 2)`;
    - then `new_v7_batch((1 << 44) - 2, 65_536, buf)` in a loop of at most 1,025 calls;
    - the first `Err` is `CounterExhausted`, and its `to_string()` equals the Global Constraints
      text;
    - every successful batch's first item is greater than the previous batch's last item.
  - Point the existing fallback wrap tests (the ones that hand `fill_batch` a block straddling
    the wrap) at the fallback's wrap rule. Thanks to the `cfg(any(test, …))` gate they run on
    the host and pass unchanged.

- [ ] **Step 2: Run them and confirm they fail.**
  `cargo test` in `rust/`. Expected: compile errors, because `new_v7_in` and
  `new_v7_batch_in` don't exist yet. `fallback_wrap_rule_tail_runs_ahead_of_the_next_call` is
  the one test that already describes today's behaviour, so it passes as soon as it compiles.

- [ ] **Step 3: Implement.**
  - **`cdylib_smoke.py`:** after the existing checks, call `uuid_new_v7(2**44 - 2, out)`, then
    loop `uuid_new_v7_batch(2**44 - 2, 65536, big)` until rc != 0, at most 1,025 times. Assert
    rc == 6 with `check(...)`. The script runs as one process, and it already runs last against
    that library.

- [ ] **Step 4: Run them and confirm they pass**, using the Forge's own flags:

  ```bash
  cd rust
  cargo test
  cargo cdylib && python3 ../.github/scripts/cdylib_smoke.py   # match the Forge's invocation; read hyper-check-core.yml for the argument form
  RUSTFLAGS='--cfg getrandom_backend="custom"' cargo check --no-default-features --target thumbv7em-none-eabi
  cargo build --release --example no_panic --features no-panic
  cargo clippy --all-targets -- -D warnings && cargo fmt --check
  ```

  Then run the wasm32 steps `hyper-check-core.yml` runs: the host and `wasm32-unknown-unknown`
  consumer builds, and the `wasm32-wasip1` test run under wasmtime if it is installed under
  `~`. If `no-panic` can't link on macOS (the Forge keeps it on Linux runners), run it in a
  Linux container or say it is CI-only in the report.
- [ ] **Step 5: Run the negative controls**, then revert each one:
  - Make the advance install `encode(m, first_step)`:
    `v7_single_after_a_batch_sorts_after_its_last_item` fails.
  - Swap the reserve and the entropy draw: `v7_entropy_failure_reserves_nothing` fails.
  - Route `new_v7_batch_in` through the fallback rule: `stream_tail_never_runs_ahead_of_the_next_call`
    fails.
  - Temporarily map `CounterExhausted => 1` (the existing random-source code) in `ffi.rs`,
    then `cargo cdylib` and run `cdylib_smoke.py`. Its rc == 6 assertion must fail at run time.
    Removing the arm would only stop the compile, because the match is exhaustive inside the
    crate [plan review 2].

### Task 4: Core docs

**Files:**
- Modify: `README.md`:
  - "v7 ordering, precisely" (lines 119–128): replace it with the spec's Contract section,
    written as current-state prose;
  - the v7 row (line 110): "26-bit monotonic counter" becomes "a counter re-seeded each
    millisecond";
  - the error and return-code mentions.
- Modify: `rust/README.md`: v7 and error sections, and the `no_std` note that the
  no-64-bit-atomics targets keep the never-reset counter.

- [ ] **Step 1:** Grep for `wrap`, `never resets`, `reverse`, `trusted as given`, `67,108,864`,
  `a millisecond ahead` and `counter space` in `README.md` and `rust/README.md`. Rewrite every
  hit to the new contract (spec, Contract section, including Scope, Embedded timestamp at 11 s,
  Volume as consumed steps, Capacity and Exhaustion).
- [ ] **Step 2:** Check every README example you touch by hand, per CLAUDE.md.

### Tasks 5–13: Bindings

Each binding task has the same shape:
- map code 6, or the Rust `CounterExhausted`, at **both** the single and the batch site, ahead
  of any default arm;
- add the error to the binding's error docs and README error table;
- rewrite every v7 doc comment or README line about the wrap, reverse sorting, "trusted as
  given" or the old 1 ms lead (grep as in Task 4);
- add one test, run in its own process where the task says so;
- run the binding's suite, linter and formatter.

The **exhaustion script** used by the subprocess tests is the same everywhere **[plan review]**.
M = `17592186044414` (2^44 − 2).
1. One single v7 at M succeeds.
2. Batches of 65,536 at M until the first error, at most 1,025 of them. That error is the
   binding's exhaustion error with the exact message: **the batch site**.
3. Singles at M until the first error, at most 1,024. A failed batch can leave up to 1,023
   steps. That error is asserted separately, by type and message: **the single site**.
4. Now the stream sits at `(L_MAX, STEPS − 1)`, so every in-stream call at M fails at once. Make
   one call through **every other v7 door** the binding has, each asserting the exhaustion
   error:
   - Go: `FillV7At`, `FillV7BytesAt`, `NewV7BatchAt`;
   - **C# and Swift are exceptions** [plan review 2]. They run no exhaustion script: Tasks 12
     and 13 test only the code 6 mapping, and the C ABI end to end is covered by Task 3's
     `cdylib_smoke.py`. Their other public doors (C#'s `Span` fill and `TryNewV7`; Swift's
     `fillV7`) are covered by a step in their task that reads each door's call site and
     confirms it goes through the tested helper;
   - R: `uuid_v7` with a time vector of length 2, which goes through the single site;
   - Python, Ruby, PHP and TypeScript: their raw and bytes doors, where they exist.

**Negative control for every binding:** break the single-site mapping and the batch-site mapping
*one at a time*, rebuild, and confirm the runtime assertion for that step fails. How to break
it depends on the language [plan review 2]:
- **A default arm keeps it compiling** (C#, Java, Go, Swift, PHP `Runtime.php`, Ruby
  `runtime.rb`, R's C code, and TypeScript's `v7_error`, which has `_`): remove the new arm.
- **The Rust match is exhaustive** (`python_ext.rs`, `ruby_ext.rs`, `php_ext.rs`): temporarily
  map `CounterExhausted` to the random-source error those files already build. Removing the
  arm would only stop the compile.

#### Task 5: Python
- **Files:** `python/src/hyperuuid/__init__.py` (the `python_ext.rs` arm is in Task 0) (docstrings that describe the
  error), `python/README.md`, and a new `python/tests/test_counter_exhaustion.py`.
- **Error:** `RuntimeError(<message>)`. Python's other state failure, the random source, is a
  `RuntimeError` too.
- **Test:** `test_counter_exhaustion_in_a_subprocess` runs the exhaustion script with
  `subprocess.run([sys.executable, "-c", SCRIPT], check=False)`. It asserts exit 0, with the
  script itself asserting the `RuntimeError` and its message.
- **Run:** maturin develop in `python/`, the `pytest` command from CI, and `ruff`/`mypy` as CI
  runs them.

#### Task 6: Ruby
- **Files:**
  - the error class and the `ruby_ext.rs` arm are in Task 0; this task gives the class its
    one-line doc in the style of its neighbours;
  - `ruby/lib/hyperuuid/runtime.rb`: `when 6 then raise CounterExhaustedError, …` in both
    `new_v7` and `new_v7_batch`;
  - `ruby/README.md`;
  - `ruby/spec/counter_exhaustion_spec.rb`.
- **Test:** run the script in a subprocess for **each backend**, Magnus and Fiddle. Use the
  subprocess helper pattern in `ruby/spec/native_backend_spec.rb:8–21`, which already selects
  Fiddle in a child. Each expects `HyperUuid::CounterExhaustedError` with the message.
- **Run:** the `ruby-ext` alias build, `bundle exec rspec`, and `rubocop` as CI runs them.

#### Task 7: PHP
- **Files:**
  - the exception class and the `php_ext.rs` arm are in Task 0; this task gives the class its
    docblock in the style of `TimestampOutOfRangeException.php`;
  - `php/src/Runtime.php`: `if ($rc === 6)` before the `!== 0` fallback in `newV7` (line ~91)
    and `newV7Batch` (line ~167);
  - `php/README.md`;
  - a test in `php/tests/HyperUuidTest.php`.
- **Test:** `testCounterExhaustion`, marked `#[RunInSeparateProcess]`. Expect
  `CounterExhaustedException` with the message from the FFI door. If the suite already exercises
  the ext-php-rs door, run the script there too, expecting the generic exception with the same
  message, which matches how that door reports every v7 error.
- **Run:** `local-core.sh`, then the suite with `HYPERUUID_NATIVE_LIBRARY`, plus the PHP linter
  and formatter from CI.

#### Task 8: TypeScript
- **Files:**
  - `typescript/src/lib.rs`: `ErrorCode::CounterExhausted` → `"ERR_HYPERUUID_COUNTER_EXHAUSTED"`,
    and an arm in `v7_error` before `_`;
  - `typescript/index.d.ts` and the README's list of error codes;
  - `typescript/tests/errors.test.ts`.
- **Test:** spawn `process.execPath` with the script inlined, so it works under both Node and
  Bun. Assert `code === "ERR_HYPERUUID_COUNTER_EXHAUSTED"` and the message.
- **Run:** `npm run build && npm test && npm run test:bun && npm run lint`. Confirm
  `git diff --stat typescript/index.js typescript/binding.*` is empty apart from intended edits.

#### Task 9: R
- **Files:**
  - `r/src/hyperuuid.c`: `if (rc == 6) Rf_error("<message>")` at every v7 call site (around
    lines 161 and 170);
  - roxygen docs for `uuid_v7`/`uuid_v7_raw` in `r/R/`, then re-document with `devtools`;
  - `r/README.md`;
  - `r/tests/testthat/test-generate.R`.
- **Test:** `test_that("counter exhaustion errors", …)` runs the script through
  `system2(file.path(R.home("bin"), "Rscript"), …)` with `.libPaths()` passed in. It uses
  `skip_if(!nzchar(system.file(package = "hyperuuid", lib.loc = .libPaths())))`, so it runs
  under `R CMD check` and skips under `load_all`. The script calls
  `uuid_v7_raw(65536, time = .POSIXct(17592186044.414, tz = "UTC"))`.
  - R's time is seconds, so the ms can round down by one. Either way the stream fills and
    exhausts at the ceiling within 2,050 batches, so the loop bound is 2,050 here.
- **Run:** `r/tools/vendor.sh`, `devtools::test("r")`, `R CMD build r` and
  `R CMD check --as-cran --no-manual`, `clang-format --dry-run -Werror r/src/*.c r/src/*.h`, and
  lintr against a scratch install.

#### Task 10: Go
- **Files:**
  - `go/errors.go`: `ErrCounterExhausted = errors.New("hyperuuid: <message>")`, documented like
    its neighbours;
  - `go/uuidgen.go`: `case 6` in `NewV7At` (line ~93) and in `errBatch` (line ~257);
  - `go/README.md`, the error table at line 274 and the `MaxV7Batch` paragraph at line 313;
  - a new `go/internal/exhaustiontest/exhaustion_test.go`. A separate package is a separate
    test binary, so it is its own process.
- **Test:** `TestCounterExhaustion` runs the script and asserts
  `errors.Is(err, hyperuuid.ErrCounterExhausted)`.
- **Run:** `local-core.sh`, then `go test -count=1 -tags hyperuuid_local ./...`, `gofmt -l`
  and `go vet`.

#### Task 11: Java
- **Files:** `UuidGenerator.java`:
  - `case 6 -> new IllegalStateException(<message>)` in `batchFailure` (line ~969);
  - the same arm in the single-call mapping;
  - Javadoc on the v7 methods;
  - `java/README.md`;
  - a test in `UuidGeneratorTest.java`.
- **Test:** `counterExhaustionInAChildJvm`. Reuse the `ProcessBuilder` pattern at
  `UuidGeneratorTest.java:525–540`: same java command, classpath and native-access flags. A
  nested `ExhaustionMain` class runs the script and exits 0 only on `IllegalStateException`
  with the message.
- **Run:** the Maven goal CI runs, plus its formatter check.

#### Task 12: C#
- **Files:**
  - `csharp/HyperUuid/UuidGenerator.cs`: add `6 => new InvalidOperationException(<message>)`
    in `ThrowOnBatchFailure` (line ~214) and in the single-call switch (line ~426);
  - move the single-call switch into `internal static Exception V7Failure(int rc, string entryPoint)`;
  - change `ThrowOnBatchFailure` from implicitly private to `internal` **[plan review]**.
    `InternalsVisibleTo` does not expose private members;
  - `csharp/HyperUuid/HyperUuid.csproj`: `<InternalsVisibleTo Include="HyperUuid.Tests" />`
    (the assembly is unsigned);
  - `csharp/README.md`;
  - a test in `csharp/HyperUuid.Tests/`.
- **Test:** `CodeSixIsCounterExhaustion` asserts that `V7Failure(6, "uuid_new_v7")` and
  `ThrowOnBatchFailure(6, …)` give `InvalidOperationException` with the message.
  - C# has no cheap in-suite process isolation, which is why this is a mapping test and not an
    end-to-end one. The C ABI end-to-end case is covered by Task 3's `cdylib_smoke.py`. Say so
    in the report.
  - **Door check [plan review 2]:** read each public v7 door (`NewV7`, `TryNewV7`, the batch
    and `Span` fill overloads) and confirm it reaches `V7Failure` or `ThrowOnBatchFailure`.
    `TryNewV7` returns `false` on any nonzero code. List the doors in the report.
- **Run:** `dotnet test` and `dotnet format --verify-no-changes`, as CI runs them.

#### Task 13: Swift
- **Files:** `swift/Sources/HyperUuid/UuidGenerator.swift`:
  - add `case counterExhausted` to `Error`, with a `description` carrying the message;
  - map rc 6 at the v7 single site (line ~222) and in the batch and fill sites, ahead of
    `randomSourceFailure`;
  - factor the v7 mapping into `static func v7Error(_ rc: Int32, count: Int) -> Error`;
  - `swift/README.md` (line 273 error list, line 188 paragraph);
  - `swift/Tests/HyperUuidTests/`.
- **Test:** `testCodeSixIsCounterExhausted`, through `@testable import`, asserts
  `v7Error(6, count: 1)` is `.counterExhausted`. The reason it is a mapping test is the same as
  C#'s.
  - **Door check [plan review 2]:** read each public v7 door (`newV7`, `newV7Batch`, `fillV7`
    with both destinations) and confirm it goes through `v7Error`. List the doors in the
    report.
- **Run:** `local-core.sh`, then `HYPERUUID_LOCAL_CORE=1 swift test` and swift-format as CI
  runs it.

### Task 14: Changelog, roadmap, siblings, performance, final verification

**Files:** `CHANGELOG.md`, `r/NEWS.md` (if R has an Unreleased section), `ROADMAP.md`, and the
benchmark tables in each binding's README if they are re-measured.

- [ ] **Step 1: CHANGELOG.** Under `## [Unreleased]`, add plain entries for:
  - the core: the re-seed, the new contract and `CounterExhausted` with code 6;
  - each binding's new error type.

  No semver callout.
- [ ] **Step 2: ROADMAP.** Tick the counter item and replace "update the corpus" with "no corpus
  change: the layout is untouched".
- [ ] **Step 3: Sibling repos.** Grep `../HyperCast` and `../HyperTabular` for `v7`, `counter`
  and `wrap`, and report what turns up. Edit only wording that describes HyperUuid's v7.
- [ ] **Step 4: Performance [plan review].** The stream is process-global, and Criterion runs
  every bench in one process, so the timestamps have to be planned.
  - **The problem:** today `v7_current_timestamp` pulls the stream to the present, so the later
    fixed-`RFC_TEST_VECTOR_MS` benches fall more than W behind and silently measure the backfill
    path. A fixed timestamp held long enough can also split about 1,000 times and reach
    `CounterExhausted`, which `.unwrap()` turns into a panic.
  - **The fix:** one timeline per process [plan review 2]. In
    `rust/benches/uuid_benchmarks.rs`, add a synthetic clock: `static BENCH_MS: AtomicU64`,
    started at `BENCH_BASE_MS = 1_700_000_000_000` (any value of at least 60,000 works).
    - Every synthetic-timeline bench takes `m = BENCH_MS.load()` and advances it by 1 every
      1,000 iterations. That gives an advance, and so a re-seed, every 1,000 calls, and keeps
      the bench in stream.
    - The synthetic benches are `v7`, `hyperuuid_v7`, `v7_batch`, `v7_individual_x1000`, and
      new `v7_contended_8t` and `v7_batch_contended_8t`. The contended pair uses `iter_custom`
      with `std::thread::scope`: 8 threads, singles and `batch_1000` respectively.
    - `v7_backfill` [plan review 3] first seeds the stream with one `new_v7(BENCH_BASE_MS)`
      *outside* the timed loop. A fresh process starts unseeded, so without that call the first
      timed call would seed the stream at the old timestamp and never take the backfill path.
      The loop then times `new_v7(BENCH_BASE_MS − 60_000)`, which is 60 s behind and so out of
      stream.
    - `v7_current_timestamp` reads the real clock and is the only real-time bench.
  - **Separate processes:** never mix the two timelines in one process. Each v7 case runs in
    its own `cargo bench` invocation, which is a fresh process with an unseeded stream.
  - **Filters [plan review 3]:** Criterion matches the filter as an unanchored regex against the
    full benchmark ID (`group/name`), so a bare `v7` would select every v7 case. Every
    invocation uses an anchored full ID, for example:

    ```bash
    cargo bench --bench uuid_benchmarks -- '^single_item/v7$'
    cargo bench --bench uuid_benchmarks -- '^single_item/v7_current_timestamp$'
    ```

    The same pattern applies to `single_item/v7_backfill`, `vs_uuid_crate/hyperuuid_v7`,
    `batch_1000/v7_batch`, `batch_1000/v7_individual_x1000` and the two contended IDs, in
    whatever groups they land in.
  - **Check each filter first:** before timing, run each filter with `--list`, as in
    `cargo bench --bench uuid_benchmarks -- --list '^single_item/v7$'`. It must print exactly
    one benchmark. Record the ID list in the report.
  - **Before and after: the same harness against both cores.** Copy the revised bench file into
    a worktree at HEAD. It uses only public API (`new_v7`, `new_v7_batch`, `now_v7`), so it
    builds there unchanged. Run the identical per-process invocations against the HEAD core and
    the new core, so every case, singles, batches, backfill, real-time and contended alike, is
    compared like for like. Remove the worktree afterwards.
  - Report each delta, losses included, and label stream-path and backfill numbers
    separately.
  - Do not edit any README table without re-running its harness. Mark the v7 rows of tables
    not re-measured "measured before the per-millisecond re-seed (core change, 2026-10-27)".
- [ ] **Step 5: Full verification.** Re-run every command in Tasks 3 and 5–13 from a clean
  `cargo cdylib`, run `python3 corpus/oracle.py` (unchanged), and check `git status` against
  the expected file list.
  - The report lists what ran locally and what only CI can prove: the musl legs, Windows,
    Android/iOS, and no-panic if it was not runnable here.
