# Apple entropy: one user-space call instead of a syscall per id

Written 2026-10-28, as the fifth Phase 1 item. Status: **Draft for owner review.** Ships in
1.1.0, landing ahead of the system-wide M6 receipts pass.

## Goal

On macOS the core gets its random bytes from `getentropy`: one syscall per id, and one per
256 bytes of a batch. On the Apple M6 (Mac18,5, macOS 27.0.1) that costs about 600 ns for the
11 bytes a single v7 takes. That is most of a single id's cost in every binding, and the
Java brief's ~600 ns `newV7` probe was exactly this. Every table moves to the M6 in 1.1.0. If
the call stays, each single-id row there measures macOS's syscall price, not the core, and a
reviewer would rightly call it a macOS artifact. Switching Apple platforms to Apple's own
user-space CSPRNG makes the M6 tables measure the core. Every binding on every Apple platform
(macOS, iOS, Mac Catalyst) gets the speed with no API change.

## Decisions (owner, 2026-10-28)

- **Every table moves to the M6** in one receipts pass, run last in 1.1.0 (decisions.md).
- **Spec this item, landing before that pass** ("Yes, spec it", 2026-10-28).
- **Siblings:** frozen until v1.1 ships.

## Facts (checked 2026-10-28)

**Throwaway C probe** on the M6 (`-O2`, macOS 27.0.1; the probe is not kept):

| Call | 11 bytes, one call | 1 MiB, per 16 bytes |
| --- | --- | --- |
| `getentropy` (256-byte chunks for the 1 MiB) | 583–612 ns | 45.5 ns |
| `CCRandomGenerateBytes` | 58.5 ns | 2.6–2.7 ns |
| `arc4random_buf` | 25.9 ns | 2.7 ns |

**`getrandom` 0.4.3** (in `rust/Cargo.lock`):
- macOS takes its `getentropy` backend (`src/backends.rs:53-59`), which loops in 256-byte
  chunks.
- iOS, tvOS and watchOS, and by `target_os = "ios"` Mac Catalyst too, take `apple_other`,
  which is `CCRandomGenerateBytes`. Its README table says so (:39-40).
- So today **only macOS pays the syscall**. The Apple mobile targets already run the
  user-space generator.

**Rust std 1.99.1** (`library/std/src/sys/random/apple.rs`) uses `CCRandomGenerateBytes` on
every `target_vendor = "apple"`. Its comment says the call is seeded from the kernel's CSPRNG,
that `arc4random_buf` "calls into the same system service anyway", and that
`CCRandomGenerateBytes` "has been proven to be App Store-compatible".

**`arc4random(3)`** on this Mac: a CSPRNG "re-seeded from the kernel random number subsystem
on a regular basis, and also upon fork(2)". The functions "are always successful".

## Design

**One cfg arm in `rust/src/entropy.rs`.** On `target_vendor = "apple"`, `fill_with` calls
`libc::CCRandomGenerateBytes(buf.as_mut_ptr().cast(), buf.len())` once per request, of any
size, and maps any return but `kCCSuccess` to `getrandom::Error::UNEXPECTED`. It never panics
or asserts, unlike std, so the no-panic proof is unaffected. The generic `getrandom::fill` arm
loses `target_vendor = "apple"` from its scope.
- `libc` is already a `cfg(unix)` dependency (`rust/Cargo.toml:32-33`). Nothing is added.
- The call sits in `libSystem`, so no framework is linked: the dylib, the static archives, the
  Swift artifact bundle and xcframework, and the C# Apple `NativeReference`s keep their link
  lines.

**Why `CCRandomGenerateBytes` and not `arc4random_buf`,** although the latter is about 33 ns
faster per single id. It is the call Rust std uses on every Apple platform, and the one
`getrandom` already ships on iOS, tvOS, watchOS and Mac Catalyst. So this change gives macOS
the same generator the rest of Apple already gets through both, and a reviewer meets a
precedent, not a choice. It is also proven App Store-compatible, which matters for the Swift
xcframework and MAUI iOS apps. On batches the two are equal. Open item 1 records the
alternative.

**Behaviour.**
- macOS: a user-space DRBG replaces a syscall per id.
- iOS, tvOS, watchOS and Mac Catalyst: the same function as today, called directly instead of
  through `getrandom`.
- Linux, Android, Windows, WASI and the rest: untouched.
- No corpus change. Nothing persisted is random-dependent in a way bytes can pin; the corpus
  pins layouts and v5.

**Module doc.** The `entropy.rs` header's first paragraph gains "on Apple platforms,
`CCRandomGenerateBytes`". "macOS has not been measured yet" is replaced by the M6 receipt,
taken from Criterion before and after this change (single `new_v4`, `new_v7`, a batch of 1000).

## Contract changes

- **`CHANGELOG.md`, Unreleased, Changed:** "**Faster ids on macOS.** The core draws its random
  bytes from `CCRandomGenerateBytes`, Apple's user-space CSPRNG seeded by the kernel, the
  generator Rust's standard library uses on Apple platforms. It replaces a `getentropy` syscall
  per id: a single v7 on an Apple M6 went from ⟨before⟩ to ⟨after⟩ ns. iOS, tvOS, watchOS and
  Mac Catalyst already used it through `getrandom`. *(core, and every binding on macOS)*"
- **`CLAUDE.md`, "Open follow-ups":** the "Core: the user-space ChaCha20 entropy path …"
  bullet becomes: "Core: the ChaCha20 pool is Linux and Android only. Apple platforms take
  `CCRandomGenerateBytes`, Windows `ProcessPrng`, and the rest `getrandom`." The clause about
  the TypeScript Apple M6 tables goes; the receipts pass re-measures them.
- **READMEs:** no performance wording changes in this item. Every number moves in the receipts
  pass. A README sentence that blames macOS's per-id `getentropy` (TypeScript's and R's M6
  notes, and the Java spec's planned sentence) is dropped in that pass, not reworded here.

## Files

- **[own]:** `rust/src/entropy.rs` (the new arm, the narrowed generic arm, the module doc, and
  tests); `rust/examples/apple_fork.rs` and its integration test (new); `rust/benches/` only if a single-id bench is missing.
- **[shared]:**
  - `CHANGELOG.md`: one bullet;
  - `CLAUDE.md`: the one follow-up bullet;
  - `docs/superpowers/specs/2026-10-28-java21-jni-design.md`: its "macOS `getentropy`
    dominates" sentence and audit row are struck by the integrator.
- **Forge:** none.

## Ordering constraints

1. After the v7 re-seed has merged; it touches `entropy.rs`'s WASI arm.
2. Before the system-wide receipts pass, so every M6 table measures this core.
3. Independent of the other four Phase 1 items.

## Verification

**Local (this M6),** each check with its negative control:
1. **The symbol the core imports.** After `cargo cdylib`, `nm -u target/release/libhyperuuid.dylib`
   lists `_CCRandomGenerateBytes` and not `_getentropy`.
   - NC: the same command on the parent commit lists `_getentropy`.
   - The same check runs on the Swift artifact bundle's macOS archive, built through
     `.github/scripts/local-core.sh`.
2. **It is the fast path.** Criterion's `new_v4` and `new_v7` singles fall from about 600 ns to
   under 100 ns, and a batch of 1000 falls too.
   - NC: the baseline run on the parent commit, taken in the same session.
3. **Correct bytes.**
   - `cargo test` passes, including the v7 stream tests, the corpus, and the ignored
     `MAX_BATCH` test run with `--ignored`: one 402 MB request in a single call.
   - A new test checks that two 64-byte draws differ and are not all zero.
     NC: a scratch arm that returns zeros fails it.
4. **Fork, with inherited state (amended after review, 2026-10-28).** The test runs in an isolated
   helper process (an example binary, `examples/apple_fork.rs`, spawned by an integration test),
   so a hang or crash cannot take the test runner with it. The helper:
   - initializes and advances the generator first, with several `fill` draws of varying size in
     the parent, so any state the generator keeps exists before the fork;
   - then forks;
   - then draws 64 bytes in both parent and child; the child writes its bytes to a pipe.

   The draws must differ. The parent `poll`s the pipe and reaps the child with a 5 s bound. The supervising
   integration test bounds the helper too: it spawns it, waits up to 15 s with `try_wait`, and
   kills it and fails on expiry, so a helper stuck in a parent-side draw before the pipe exists
   still fails. Either bound expiring is a failure, and
   failing to finish in time is a failure.
   - **NC:** a scratch arm with a deliberately inherited, unreseeded generator (a static ChaCha20
     keyed once on first use and never re-keyed on fork) must fail the comparison. A generator
     that seeds independently on first use would not show it, which is why the parent draws
     before forking.
5. **Every binding.** `cargo cdylib`, then every binding's suite on this Mac, with Go, Swift and
   PHP through `local-core.sh`, R after `r/tools/vendor.sh`, and TypeScript `npm run build &&
   npm test && npm run test:bun`.
6. **no_std and proofs.**
   - `cargo check --no-default-features` for `aarch64-apple-darwin` and `aarch64-apple-ios`.
   - The `thumbv7em-none-eabi` check, unchanged.
   - The no-panic proof is Linux-only and does not cover this arm. That is labelled, below.

**CI-only:**
- the osx-arm64 `build-native` leg runs every suite;
- `test-apple-mobile` runs the iOS, iOS-simulator and Mac Catalyst builds, which pass through
  the new arm instead of `getrandom`'s identical call;
- the osx-x64 leg runs in release mode only.

**Release-time:** none new. No artifact changes shape.

## Nitpick audit

| Hole | Closed or labelled |
| --- | --- |
| "A user-space DRBG is weaker than the kernel's" | It is Apple's CSPRNG, seeded by the kernel: the generator Rust std uses on all Apple platforms and `getrandom` ships on iOS. Cited, not argued |
| Fork safety | Check 4 advances the generator before forking, then compares parent and child, with an inherited-state negative control. `arc4random(3)` documents a re-seed on fork; std's note says both calls reach the same service |
| VM snapshot restore (Linux's one-second key rule) | Labelled: Apple documents a regular re-seed with no interval. A restored macOS VM is out of scope, as it is for std |
| App Store | `CCRandomGenerateBytes` is the call std calls proven App Store-compatible |
| No-panic | The arm returns an error code and never asserts. The link-time proof runs on Linux only, so this arm is checked by reading, as every non-Linux arm already is |
| Numbers | M6 Criterion before and after, dated. They go only in the CHANGELOG and module doc; the tables wait for the receipts pass |
| `arc4random_buf` would be faster | Recorded in Facts and Open item 1, not hidden |

## Out of scope / Later

- Porting the Linux ChaCha20 pool to Apple. Its fork safety rests on `MADV_WIPEONFORK`, which
  macOS lacks, and Apple's own DRBG is already at the pool's speed on batches.
- The BSDs. They already take `getrandom`'s backends, and no BSD is shipped.
- HyperCast and HyperTabular get their own roadmaps after v1.1.

## Open items

1. **`CCRandomGenerateBytes` or `arc4random_buf`?** Recommended: `CCRandomGenerateBytes`, for
   the std and `getrandom` precedent and the App Store record. `arc4random_buf` saves about
   33 ns per single id on the M6. It is equal on batches, and it is the one Apple's man page
   documents (fork re-seed, "always successful").
