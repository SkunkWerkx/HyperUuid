# v7 counter: re-seed per millisecond

Brainstormed 2026-10-27 (architectural path) from ROADMAP Phase 0, item 1, and revised the
same day after four review rounds. Changes are marked **[review N]** inline and listed at the
end. This is the design spec; `docs/superpowers/plans/` will hold the implementation plan
derived from it.

## Context

`rust/src/v7.rs` keeps one process-wide `AtomicU32` counter. It is seeded once in [0, 512),
advanced by `fetch_add` and never reset. Subtracting the counters of two v7s from one process
gives the exact number minted between them, however far apart in time they were. RFC 9562
§6.2 Method 1 (Fixed Bit-Length Dedicated Counter) re-initialises the counter when the
timestamp advances. This spec adopts that, and replaces the ordering contract in README
"v7 ordering, precisely".

**No corpus change.** `corpus/v7_layout.json` pins `write_fields(ms, counter26)`, which places
a timestamp and a 26-bit counter, and that placement is untouched. What changes is which
counter values get minted, and the ordering contract around them. Stored rows are not
reordered or re-derived. The roadmap's "update the corpus" is dropped.

## Decisions taken in the brainstorm

| Decision | Choice | Why |
| --- | --- | --- |
| Threat model | Hide counts across ms only (Method 1) | Keeps the batch space; same-ms counting is inherent to a stepped counter and stays documented |
| Older timestamps | Clamp to the stream's ms | Racing clock reads across threads must not break in-process order |
| Clamp reach | Bounded, past side only | The core has no clock, so it can't tell an idle gap from a future-dated call |
| No 64-bit atomics | Fall back to today's counter under `cfg(target_has_atomic = "64")` | No new dependency, and every shipped artifact has 64-bit atomics |
| State layout **[review 1]** | One `AtomicU64` holding a 44-bit ms and a 20-bit step | Full ms to the year 2527, lock-free, no dependency |
| Exhaustion **[review 1]** | New `NewV7Error::CounterExhausted`, C ABI code `6` | Fails loudly instead of misordering. The enum is `#[non_exhaustive]`, so this is not a breaking Rust change |
| Backfill batches **[review 3]** | Size-bounded seed: the batch always fits at its own ms | Never rolls, so it never stamps m + 1 and never hits `MAX_UNIX_MILLIS` |

## The stream word

`STREAM: AtomicU64 = (ms + 1) << 20 | step`, compiled under `cfg(target_has_atomic = "64")`.
**`step` is the last step reserved in `ms`** **[review 3]**.

- **Full ms [review 1].** Every ms in [0, `L_MAX`] is stored exactly, where
  `L_MAX = 2^44 − 2`. An m above `L_MAX` never reaches the stream.
- **Counter mapping.** `counter26 = step << 6 | low6`, and the UUID layout is unchanged.
  - A single call reserves one step and gets 6 random low bits.
  - A batch of n reserves k = ⌈n/64⌉ consecutive steps and fills `low6` sequentially, so its
    items are contiguous and strictly increasing.
  - `MAX_BATCH` stays 2^26 (k = 2^20 steps).
- **Publication [review 1].** The +1 offset means 0 is never a valid state, so 0 means
  "unseeded". First use is the advance transition from state 0. There is no flag. Today's
  `SEED_CLAIMED` add-trick is safe only because seeding adds to a counter, so it stays in the
  fallback alone.
- **CAS loop.** Every transition is a `compare_exchange_weak` on the one word. `Relaxed` is
  enough, because no other memory is published through it. The random draws are `r` for the
  seed and `low6`. They happen once, before the loop, and are reused across retries.

### Seed

For a call that reserves k steps from one u32 `r` **[review 3]**:

```
seed(k) = (u64(r) × (min(2^19 − 1, 2^20 − k) + 1)) >> 32
```

This is a multiply-shift, with no division and no panic path. It gives
`seed ∈ [0, min(2^19 − 1, 2^20 − k)]`. So `seed < 2^19`, `seed + k ≤ 2^20`, and the k steps
`seed … seed + k − 1` always fit in one ms. `MAX_BATCH` forces seed 0.

### Transitions

Let L be the stream's ms, s its last reserved step, m the supplied ms, and `lag = L − m`.
Unseeded counts as `lag < 0`. Each row is one CAS from `(L, s)`, except the last.

| Band | Reserves | Installs |
| --- | --- | --- |
| Advance: unseeded, or `lag < 0`, with `m ≤ L_MAX` | steps `seed(k) … seed(k) + k − 1` at m | `(m, seed(k) + k − 1)` |
| Clamp, fits: `0 ≤ lag ≤ W`, `s + k ≤ 2^20 − 1` | steps `s + 1 … s + k` at L | `(L, s + k)` |
| Clamp, splits: `0 ≤ lag ≤ W`, `s + k > 2^20 − 1` | head `s + 1 … 2^20 − 1` at L (empty when `s = 2^20 − 1`), tail `0 … t − 1` at L + 1, where `t = k − (2^20 − 1 − s)` | `(L + 1, t − 1)` |
| Out of stream: `lag > W`, or `m > L_MAX` | steps `seed(k) … seed(k) + k − 1` at m | nothing (no CAS) |

- **Split checks, before any CAS.** A failing call leaves the state bit-identical and
  returns `CounterExhausted`.
  - **Drift [review 1]:** the split requires `(L + 1) − m ≤ D`.
  - **Ceiling [review 2]:** the split requires `L + 1 ≤ L_MAX`. Without this check,
    encoding 2^44 truncates to the unseeded state or to a smaller word.
- **Out-of-stream calls [review 3]** use the same size-bounded seed, so a batch of that kind
  never rolls. A batch's items are strictly increasing, all at m. Calls of this kind are
  unordered against each other and against the stream.
- **`TimestampOutOfRange` [review 3]** on the 64-bit path means only `m > MAX_UNIX_MILLIS`,
  caught at input. Stream states stop at `L_MAX < MAX_UNIX_MILLIS`, and out-of-stream batches
  never roll. The fallback keeps today's overflow check (`v7.rs:231`).
- **Constants.** `W = 11_000` ms and `D = 1_000` ms, as named consts. The guarantee window is
  `G = W − D = 10 s`.

### Invariant

Every successful CAS installs a strictly larger u64, and the ceiling check means the encoding
never wraps. A UUID's (ms, counter26) order is the order of the step it reserved, with `low6`
ordering within a step. Every reserved step sits after the previous state's last step and at
or before the new state's last step. A call's successful CAS is its **reservation**, and its
linearization point **[review 2]**.

Over **stream reservations** (successful calls in the advance or clamp bands):

- Their UUIDs sort in CAS order.
- A reservation completed before another call begins sorts before that call's UUIDs, on any
  thread.
- Overlapping reservations sort in CAS order, which need not be completion order.

**Drift lemma.** Let S be the newest m among stream reservations so far. Then
`L ≤ S + D`:

- an advance sets `L = m`;
- a split requires `L + 1 ≤ m + D`, with `m ≤ S`.

So any call with `m ≥ S − G` has `lag ≤ G + D = W`. If `m ≤ L_MAX`, it takes the advance or
clamp band.

## Contract

This replaces README "v7 ordering, precisely" and every matching doc comment.

- **Scope [review 3].** These guarantees hold on targets with 64-bit atomics, for
  timestamps `m ≤ L_MAX` (before the year 2527). S is the newest timestamp among
  *successful stream reservations* only. Failed calls and out-of-stream calls never change S
  or the stream.
- **Ordering.** A call with `S − G ≤ m ≤ L_MAX` either fails with `CounterExhausted`, or
  reserves from the stream. If it reserves, its UUIDs sort strictly after every UUID from a
  stream reservation that completed before it began.
  - Overlapping reservations, singles and batches alike, are ordered by reservation, not by
    completion.
  - Overflow is included.
  - All three of today's wrap caveats are gone.
- **Embedded timestamp [review 3].** It runs at most 11 s ahead of the supplied one:
  - a clamp that fits stamps L with `L − m ≤ W`;
  - a split's tail stamps L + 1 with `(L + 1) − m ≤ D`;
  - an out-of-stream call stamps m exactly.

  Today the bound is 1 ms.
- **Volume [review 3, review 4].** What leaks is consumed steps, not reservations or UUID
  counts. This applies to stream-generated values only: an out-of-stream value draws its own
  seed and relates to nothing.
  - **Within one ms,** the step difference between two values is exactly the number of steps
    consumed between them. A single consumes one step; a batch of n consumes ⌈n/64⌉.
  - **Raw counters** of adjacent singles differ by 1 to 127, because of `low6`.
  - **Across a split,** from step `s_a` at L to step `s_b` at L + 1, the rollover-adjusted
    difference `(2^20 − 1 − s_a) + (s_b + 1)` is exactly the steps consumed between them. Splits
    happen only while the stream is overloaded and ahead of real time.
  - **Across a real advance,** the re-seed hides the count.
  - **A lone step value** shows at least `step − (2^19 − 1)` steps consumed in its ms.
- **Backfill.** A timestamp more than W behind the stream, or past `L_MAX`, is minted
  at m, outside the stream, and unordered.
- **Future-dated call.** One advances the stream. Until the clock catches up, real-time
  calls fall more than W behind and are minted out of stream, unordered among themselves.
- **Capacity [review 2].**
  - A ms entered by an advance holds at least 2^19 + 1 steps, since the seed is at most
    2^19 − 1.
  - A ms entered by a split holds 2^20 steps.
  - Demand above a ms's capacity splits forward, and drift accumulates up to D.
  - No throughput floor is claimed.
- **Exhaustion.** `CounterExhausted` fires before any state change when a split would put
  the stream more than D ahead of the call's timestamp, or past `L_MAX`.
- **Fallback.** Targets without 64-bit atomics (`thumbv7em-none-eabi` and similar) keep
  today's never-reset counter and its documented wrap behaviour. None of the guarantees above
  apply there.

## Files

- **`rust/src/v7.rs`:**
  - the stream word and the transitions table;
  - `seed(k)`, the checks and the CAS loop;
  - `fill_batch` generalised to `(ms, first_step, count, tail_ms)`, where the tail starts at
    step 0 and appears only when the head runs out. It stays `inline(always)`, so the
    `no-panic` proof survives;
  - the `W`, `D` and `L_MAX` consts;
  - `CounterExhausted` and its `Display`;
  - the `cfg` fallback, which is today's code unchanged;
  - the doc comments.
- **`rust/src/ffi.rs`:** return code `6`, in the module doc and in the v7 exports' mapping.
  Also `rust/src/lib.rs` (the line 27 comment) and the error mapping in
  `rust/src/ruby_ext.rs`, `python_ext.rs` and `php_ext.rs`.
- **All nine bindings:**
  - map code 6 to the binding's own error, in its own spelling;
  - one test per binding for that mapping;
  - doc comments, READMEs and README error tables that mention the wrap, reverse sorting or
    "trusted as given", swept for parity.
- **`README.md`:** "v7 ordering, precisely" and the v7 row of the version table.
- **`CHANGELOG.md`:** plain Unreleased entries for the core and each binding touched.
- **`ROADMAP.md`:** tick the item and drop "update the corpus".
- **HyperCast and HyperTabular:** check for v7 wording.

## Testing

**Core tests** use a test-only hook that sets the stream word and forces `r` and `low6`.

- **Invariant.** Over random sequences of operations, the state word only grows. Every
  stream reservation's UUIDs exceed every earlier one's.
- **Seed [review 3].** `r = u32::MAX` gives seed `2^19 − 1` for k = 1. For a mid-size k and
  for k = 2^20, `seed + k ≤ 2^20` with seed 0 at 2^20. `r = 0` gives seed 0.
- **Advance reserves the batch [review 3].** From unseeded and from an older L:
  - a 65-item batch installs `(m, seed + 1)`;
  - a following single reserves `seed + 2` and sorts after the batch's last UUID.
- **Clamp.** Lags 0, G and W stamp L. Lag `W + 1` is out of stream and leaves the state
  untouched.
- **Review 1 counterexamples.**
  - A stream at `2^38 + 100` given 100, and a stream at 100 given `2^38 + 99`, are both
    classified correctly.
  - Repeated `MAX_BATCH` at one m: each sorts after the previous until `CounterExhausted`,
    and none is ever misordered.
- **Review 2 and 3 split cases.**
  - At `s = 2^20 − 2`, a `MAX_BATCH` reserves head step `2^20 − 1` and tail `0 … 2^20 − 2`
    at L + 1. It ends at `(L + 1, 2^20 − 2)`.
  - At `s = 2^20 − 1` (the ms is used up), the head is empty and the tail is
    `0 … 2^20 − 1`. It ends at `(L + 1, 2^20 − 1)`.
  - At `L = L_MAX`, both a single at `s = 2^20 − 1` and a batch that would split return
    `CounterExhausted`, with the state bit-identical afterwards.
  - A drift-bound failure leaves the state bit-identical too.
- **Out of stream [review 3].**
  - Two sequential calls at `2^44 − 1` leave the state untouched. No ordering is asserted
    between them.
  - A `MAX_BATCH` at `MAX_UNIX_MILLIS` succeeds with seed 0, every item at m, strictly
    increasing.
  - A backfill batch with forced `r = u32::MAX` fits at m.
- **Capacity [review 4].** Two equivalent forms of the same boundary:
  - From the unseeded state, with `r = u32::MAX` so that the seed is `2^19 − 1`: `2^19 + 1`
    singles at one m, counting the advancing call that reserves the seed, end at
    `(m, 2^20 − 1)`. The next single splits.
  - With the hook installing `(m, 2^19 − 1)` as the last reserved step: `2^19` more singles
    end at `(m, 2^20 − 1)`. The next single splits.
- **Concurrency [review 3]**, checked by reservation:
  - Each thread's own results strictly increase.
  - A mutex-guarded value holds the **maximum** completed UUID. A batch contributes its last
    UUID. Each thread reads it before a call, and after the call updates it with `max`. Every
    result must exceed the value read before its call.
  - All results are distinct.
  - Timestamps are jittered within **G** of the shared clock. Lags in `(G, W]` and above W are
    tested separately, single-threaded.
  - Nothing asserts completion order.

**Negative controls:**

- The ordering tests fail against today's `v7.rs`: singles across the wrap, and a call after
  a batch's rolled tail.
- The review 1 tests fail against a model of the 38-bit draft.
- The ceiling and tail tests fail against the review 2 draft's rules.
- The advance-reserves-the-batch test fails against the review 2 draft's
  "install (m, seed)" rule.
- The concurrency oracle [review 4] is checked against an injected violation. Completed
  results arrive as 200, then 100, then a later call returns 150. The maximum-completed oracle
  rejects that sequence. A "latest returned" oracle accepts it, and a test asserts that it
  does, which proves the weaker oracle is the one that misses it.

**Commands:**

- `cargo test` in `rust/`;
- the `no_std` check on `thumbv7em-none-eabi`, which exercises the fallback;
- the wasm32 builds;
- the no-panic proofs, with `hyper-check-core.yml`'s flags;
- `cargo cdylib`, then every binding's suite:
  - Go, Swift and PHP through `.github/scripts/local-core.sh`;
  - R after `r/tools/vendor.sh`;
  - TypeScript `npm run build && npm test && npm run test:bun`.

**Performance:**

- Criterion v7 single and batch, before and after the change, uncontended and with 8
  threads. The CAS loop replaces `fetch_add`, and any loss is reported.
- Tables not re-measured are marked as predating this core change.

## Review changes

- **[review 1]**
  - The 38-bit ms residue became a full 44-bit ms with a 20-bit step.
  - Drift is bounded by D, separately from the clamp window W.
  - `CounterExhausted` was added.
  - The volume claim was narrowed.
  - First use became a CAS from the never-valid 0.
- **[review 2]**
  - A ceiling error, raised before any CAS.
  - The advance seed is bounded by k, and splits start their tail at step 0.
  - Capacity is stated per seed.
  - Ordering is defined by reservation, not completion.
- **[review 3]**
  - `step` is the last reserved step, and every transition is a table row: an advance
    installs `seed + k − 1`.
  - The guarantee is scoped to stream reservations on 64-bit atomics with `m ≤ L_MAX`. S
    counts only those.
  - Backfill uses the size-bounded seed and never rolls, so `TimestampOutOfRange` is input-only
    on the stream path.
  - The seed formula is capped at `2^19 − 1`.
  - The tail test endpoint is corrected, and a test for a used-up ms is added.
  - The concurrency test jitters within G and tracks the maximum completed UUID.
  - The timestamp lead is 11 s, not 12.
  - Volume is stated as step leakage, not UUID counts.
- **[review 4]**
  - The oracle's negative control injects a 200 / 100 / 150 violation instead of relying on
    the generator.
  - The capacity test says whether the seeding call is counted.
  - Volume is stated as consumed steps, for stream values only, with the rollover-adjusted
    difference across a split.
