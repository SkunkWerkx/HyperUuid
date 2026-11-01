# Lessons learned: what we should have known, binding by binding

HyperUuid is one RFC 9562 engine in Rust reached from twelve languages. It took from 2026-08-26
to 2026-10-31 and thirteen releases to get the bindings to where they are, and most of what we
know now we could have known on day one, had we measured first and believed second. This is the
record of what we believed, what we measured, and what we should have known, one binding at a
time, with the receipt for every number: the machine, the date, and the file in this repository
that holds it. Nothing here is adjusted for story. Where a number is a loss it is printed as a
loss, and where a claim has no receipt it says so.

Two machines did almost all of the measuring. The XPS is a Dell XPS 15 9510 with an
i9-11900H, running Fedora 44 under WSL2 (linux-x64) and Windows 11 natively. The M6 is an Apple
M6 (Mac18,5) under macOS 27. Numbers from the two are never compared with each other. An
earlier arm64 WSL2 box produced the first tables, and it is the subject of the first lesson.

## The four lessons that kept repeating

**1. Make the Rust ABI `no_std`, panic-free and allocation-free before anything else.** The
one decision that never had to be revisited. The crate was `#![no_std]` with no `alloc` from
0.1.1 (2026-08-31; the last allocation went when the batch functions started drawing entropy
through the caller's own buffer), every generation, parsing and conversion function and every C
export carried `#[no_panic]` from 0.5.0, proven at link time by a release binary CI builds
(`examples/no_panic.rs`), and the shared library every binding loads has been `no_std` since
the same release, 19,048 bytes with LTO instead of 426,856 with std's unwinder and backtraces
in it. Everything in this document stands on those three properties. Every buffer is the
binding's, which is what lets R, Go and C# hand the core vectors they allocated and sized, and
what CRAN is told in `r/cran-comments.md`. A panic cannot reach a C caller, so no binding
carries an unwinder for the core's sake, and the one that did swap unwinders (Magnus, 0.2.0)
was tested on hardware rather than argued. And a core with no runtime underneath it is a core
that builds for `wasm32-wasip1` and translates to Go source, links into a Blazor archive built
with `-Zbuild-std`, runs under TinyGo, and compiles as a bare-metal rlib, none of which was
planned in August. The rule for the next engine: these three come first, compiler-enforced
and link-proven, before the first binding is written, because every binding after that
inherits them for free and none can add them later.

**2. The bill was the entropy source, not the crossing.** Every binding was first explained in
terms of its FFI door: `ctypes` costs a microsecond, `runtime.cgocall` costs 40 ns, Fiddle is
"structural". Every one of those explanations was either wrong or beside the point. The cost
that decided single-call speed on every platform was where the random bytes came from and how
often the operating system was asked for them. The core's own entropy note
(`rust/src/entropy.rs`) has the cleanest version: on the M6, one `new_v4` cost 711 ns with
`getentropy` per id, 168 ns with `CCRandomGenerateBytes`, and the Go binding, with a buffered
AES stream in front of the same core, mints one in 37 ns. Same engine, same machine, three
entropy doors, a 19x range. The Go CHANGELOG entry for 0.3.0 said it in September: "entropy,
not the crossing, is what those doors cost". We wrote it down and then re-learned it in
October, twice.

**3. One machine's ratio is one machine's ratio.** The first published tables came from an arm64
WSL2 box whose wall-clock read cost about a microsecond and whose JDK random source ran six
times slower than the XPS's. Both sat on the far side of the comparisons that produced the
largest numbers. The 0.4.0 re-measure on x86-64 shrank the Rust-vs-`uuid`-crate headline from
13–16x to 2x on v5 and v7 and level on v4 and v6, and Swift's batch-over-loop from 42–50x to
6–7.5x (CHANGELOG 0.4.0). Since then every table names its machine and date, tables from
different machines are never merged, and a table that predates a core change says so.

**4. Pick the door for reach, then measure it; never assert it.** The two bindings whose
mechanism was chosen without a benchmark (TypeScript's napi-rs addon, "decided from the start
rather than by measurement", and PHP's extension, taken "for reach, not for the crossing")
are also the two with the least settled numbers. The bindings whose door was chosen by a
measured spike (Python's PyO3, Ruby's Magnus, Go's wasm2go) each have a CHANGELOG line that
names the number that decided it.

On the packaging side the lesson has one sentence: **a binary in git is a mistake you pay for
every release.** Go's thirteen archives, Swift's twenty-two files and PHP's native libraries
were all committed because a registry had no packing step or a build needed them at hand. All
are gone, replaced by attested release assets, a Swift bundle whose checksums CI writes, a PHP
extension PIE installs, and a Go core that is source.

---

## Rust, the core

**Believed.** That a `cdylib` with `lto = true` in its profile was link-time optimised, and that
the first tables were representative.

**Measured.** Cargo only passes `-C lto` for a cdylib built as an invocation's one crate type, so
LTO never applied while the manifest listed `["cdylib", "rlib"]`. Fixing that and building
`no_std` took the linux-x64 library from 426,856 bytes to 19,048 (346,544 with LTO alone), at
the same speed (CHANGELOG 0.5.0); stripped, 16,168 bytes (0.6.0). The 0.4.0 re-measure is
lesson 3 above.

**The entropy work, with its receipts** (`rust/src/entropy.rs` header):

| Change | Where | Before → after |
| --- | --- | --- |
| ChaCha20 pool for single ids, ChaCha20 stretch for batches (0.7.0) | XPS, glibc 2.43 | `new_v4` 38 → 23 ns; batch 10.6 → 4.4 ns per id |
| Same | XPS, musl (`getrandom` is a real syscall) | `new_v4` 228 → 24 ns; batch 12.5 → 3.4 ns per id |
| Kept `getrandom` on Windows | win-x64 | `ProcessPrng` batch 2.4 ns per id against ChaCha20's 3.4: already faster |
| `CCRandomGenerateBytes` instead of `getentropy` (2026-10-28) | M6 | `new_v4` 711 → 168 ns; `new_v7` 594 → 60 ns; batch 18.2 → 1.9 ns per id |

Apple's generator has a step at 12 bytes: 67.5 ns for 11 bytes, 169 ns for 12, in a C probe on
the M6, which is why a v4 (16 bytes) costs more than a v7 (11) there. `arc4random_buf` would
save another 33 ns per id and was rejected on its guarantees
(`docs/superpowers/specs/2026-10-28-apple-entropy-design.md`). The pool is Linux only; Android
takes the stretch for batches and `getrandom` for single ids. The per-millisecond counter
re-seed (2026-10-27) cost at most 0.9% in any Criterion case on the M6 (CHANGELOG, Unreleased).
The WebAssembly batch walk, which builds each item as two little-endian words, cost native
batches 10–13% when tried everywhere, so only WebAssembly compiles it (ROADMAP item 6).

**Should have known.** `getrandom` per id was 83–91% of a batch on Linux before the stretch
(CHANGELOG 0.7.0). One `perf` run on the first day would have put the entropy source first on
the list instead of fourth release.

**Where it stands** (XPS, `rust/README.md`): v4 23.4 ns against the `uuid` crate's 40.6, v5 49.6
against 97.6, v7 30.9 against 72.8 (the crate reads the clock inside the timed region, which
is printed), timestamp extraction 0.66 against 5.94, a batch of 1000 at 4.40 µs against 25.2
for the loop.

**Packaging.** The 0.2.0 crate is permanently unsigned, because a crates.io publish is
irreversible; from 0.2.1 publishing is tokenless OIDC. `default-features = false` failed on
every cdylib-capable target until 0.5.0. The MSRV went 1.85 (0.6.0) to 1.89.

## C#

**Believed.** That .NET 11 could be the floor at 1.0.0, and that the Blazor WebAssembly story
was a packaging detail.

**Measured.** 7.93x over `Guid.NewGuid()` on v4 and 7.53x on v7 (XPS, BenchmarkDotNet,
`csharp/README.md`): 292.84 ns against 36.94 and 38.88. The batch: 37.51 µs for 1000 singles,
6.06 µs for `NewV7Batch`, 5.13 µs filling a `Span<Guid>` and 3.78 µs filling a `Span<byte>`,
and the README says what the 1.4 µs between the last two is: the per-element conversion,
measured. The ratio moved from 5.7–8.1x to 7.5–10x when the core got its pool (0.7.0); the
binding did not change.

**The Blazor arc**, which is the reach lesson in miniature (CHANGELOG 0.4.0 through
Unreleased): a `no_std` archive to stop `duplicate symbol: rust_eh_personality` beside
HyperCast; a `wasm-opt` pass to translate exceptions to `exnref`; a 3 MB archive copied into
every app's output; an archive that shipped unattested; an Emscripten stub for `uuid_variant`
that returned the wrong answer in the browser; two diagnostics (`HYPERUUID001`, since removed,
and `HYPERUUID002`); and finally a Debug build that failed in Chromium because the SDK skips
the post-link `wasm-opt` for unstripped modules. The fix is to build the archive with
`-Zbuild-std` on stable through `RUSTC_BOOTSTRAP=1`, which leaves out the prebuilt sysroot's
legacy exception handling: the archive went from 3.3 MB to 265 KB (CHANGELOG, Unreleased, #50).
An Apple smoke-test failure was blamed on the SDK and was the test's own.

**Should have known.** The floor. .NET 10 was the floor at the start (2026-08-29), .NET 11
became it at 1.0.0, and .NET 10 came back in Unreleased, supported on desktop, server and the
browser, with iOS, Mac Catalyst and Android on 11 and later behind `HYPERUUID003`. A floor
follows upstream support, and upstream still supports 10 until 2028-11-14. We moved it twice
to end where the rule put it.

**Reach.** Ten RIDs plus WebAssembly, iOS, Mac Catalyst and Android; not tvOS, not the iOS
simulator on Intel Macs, not android-arm or android-x86. The C# tables predate the .NET 11
move, because BenchmarkDotNet stopped on net11.0 until its next release.

## Java

**Believed.** Kotlin, for a day (2026-08-26). Replaced by plain Java the next day because
`kotlin-stdlib` would have been a transitive dependency of a UUID library. Then: that a
GraalWasm backend was worth carrying, and that JDK 25 could be the floor.

**Measured.** `Linker.Option.critical(true)` and a per-thread scratch took newV4 from 155 to
102 ns and newV5 from 230 to 102 (0.3.0, JMH). A GraalVM Native Image built from the 0.3.0
jar ran `newV7(long)` in 6.4 µs where the JVM took 36 ns, 180 times slower (0.4.0); fixed to
78 ns then, 57 ns now, and a native image still costs up to about two and a half times the JVM,
which `java/README.md` prints. Today (XPS, JDK 25, FFM): `randomUUID()` 160.6 ns against
newV4 31.5 (5.1x), newV5 57.3, newV7 35.8; a batch 34.9 → 9.3 µs; a `byte[]` fill 3.6 µs. The
`UUID[]` fill at 12.3 µs is slower than the batch at 9.3, and is printed as such.

**Should have known.** The GraalWasm backend "never ran Java in a browser, no consumer asked
for it, and replaying the suite through GraalWasm's interpreter on every CI leg had become the
longest step in each" (CHANGELOG 1.0.0). It was added in 0.3.0 on a reach argument and removed
in 1.0.0 on a measured one. And the floor: JDK 25 at 0.4.0, now JDK 21 because that is what
upstream supports, with a JNI tree for 21–24 and FFM on 25+ in one multi-release jar. It is 21
and not 17 because Temurin builds no JDK 17 for Windows arm64 or Alpine arm64.

**An open debt.** The JNI path's single-call cost was estimated at a 6–10% loss over FFM from a
crossing probe on the M6 (JNI 4.7–4.9 ns, FFM `critical` 2.5 ns, "indicative only, not a README
receipt"), and the brief said a JMH receipt was required. `java/README.md` prints FFM on JDK 25
only. The JNI receipt is not there yet.

**Packaging.** 0.2.0 never reached Maven Central and never will. A shaded jar that drops
`Multi-Release: true` silently runs the JNI tree on 25. No Android: ART has JNI, but it would
need an AAR and a loader. No browser: no Java-to-wasm compiler has FFM or loads a JNI library.

## Python

**Believed.** That `ctypes` was the honest door and its cost was the price of admission. The
README of 2026-08-27 said the crossing "costs real, measurable overhead" and printed the loss:
`new_v4` 1.93 µs against `uuid.uuid4()`'s 0.99, "stdlib wins, ~2x".

**Measured.** PyO3, the next day: v4 647 ns against 1.03 µs, a win (commit 0f49269,
2026-08-28); `ctypes` was dropped the day after. Then 0.4.0 found that "nearly all of a call was
the `uuid.UUID` or `datetime` it returned": `new_v4` 540 → 243 ns, `new_v7(ms)` 726 → 354, and
the ratio over stdlib went from 1.4–2.2x to 3.0–4.2x without touching the native side. A
`bytes` v5 name cost a microsecond more because PyO3 built and discarded a `TypeError` on the
way to the right overload. Today (XPS, CPython 3.14.7, `python/README.md`): `new_v4` 215 ns
against 729, `new_v5` 372 against 1.58 µs, `new_v7` 342 against `uuid.uuid7()`'s 1.29 µs; a
batch 132 µs against 323; `fill_v7` into bytes 3.7 µs against 888 µs for fill-then-construct.

**Should have known.** The object the call returns is the call. Python spends its time
constructing `uuid.UUID`, not crossing into Rust, and the byte-level door exists because of
that.

**Packaging, where Python has the most history.** The osx-x64 wheel of 0.4.0 failed at
release, because wheels were first installed after the other registries had already
published; 0.5.0 installs every wheel in CI. A wasm backend (0.3.0) was dropped in 0.6.0
because it "reached no platform their native backends did not"; the Pyodide wheel, which does
reach one, stayed. Free-threaded CPython came in 0.9.0 as 25 wheels and went back to 17 in
Unreleased: the `cp314t` wheel "was bound to one interpreter version, too narrow an ABI to
carry", and the `abi3t` wheel for 3.15+ carries it instead, beside an sdist proven on glibc 2.17
and musl. At 1.0.0 the PyPI publish failed inside the release workflow (the nested publish
action could not find its own container image), the recovery workflow uploaded the same bytes,
and PyPI now flags 1.0.0's provenance as coming from a different workflow than the next
release's will. Nothing was rebuilt; the bytes are the attested ones.

## Ruby

**Believed.** That Fiddle's cost was "structural, not a bug to fix". The README said so.

**Measured.** Magnus, 2026-08-28: `new_v4` 459 ns, 2.8x faster than `SecureRandom.uuid` where
Fiddle had been 1.43x slower, and the commit message adds that "the old README sentence
'structural, not a bug to fix' was wrong, and the README now prints the correction". On
Windows the same swap was 406 against 2,407 ns (0.2.0). Then 0.4.0: "the timestamp argument
was range-checked against a 64-bit constant, a bignum comparison that cost more than the
mint", `new_v4` 317 → 234 ns. Today (XPS, Ruby 4.0.7): 5.5x over `SecureRandom.uuid` on v4,
3.8–4.0x on v7, 2.1x on v5; the batch only 1.3x over a loop; bytes 4.9 µs against 202 µs.

**Losses printed.** Fiddle is 1.16x slower than `SecureRandom.uuid` on v4 and about 1.3x on v7,
and that is what an Intel Mac gets (2,299 against 416 ns per `new_v4`). On a slow virtualised
clock the current-time rows reach parity, because the clock read costs a microsecond there.

**Should have known.** A fat gem per Ruby minor was always going to be the price of Magnus,
and the Fiddle fallback was always going to be needed for everything outside that grid. The
rule now is written down: Fiddle is never restricted to old Rubies, and 3.3 stays on Fiddle to
its end of life (2027-03-31). What took three releases to learn: on Alpine 0.3.0 loaded the
glibc library (0.4.0 fixed it); 0.6.0 added musl gems and renamed the glibc ones `-linux-gnu`
(Nokogiri's scheme) so every Bundler resolves the right one; 0.6.1 corrected the README's advice
that pinning `fiddle` in `Gemfile.lock` would stop a build (it does not); 0.8.0 fixed a Magnus
segfault under compacting GC. A wasmtime backend came and went (0.3.0 to 0.6.0); the ruby.wasm
gem stayed. Nine gems a release.

**The ABI discussion Python had, and Ruby still owes.** Python's grid was argued out in the
open: the `cp314t` wheel went because it was bound to one interpreter version, `abi3` and
`abi3t` carry across minors, and the sdist is proven at the floor. Ruby got the mechanics
without the argument. MRI has no stable extension ABI, so a Magnus build binds to one Ruby
minor, every platform gem is fat across the minors in `ruby/Rakefile`, and Fiddle takes
everything else. What is not written down anywhere is the rule: which minors the fat gem
carries and why those, what adding a minor costs (one more shared object in every platform
gem and one more CI leg), when a minor leaves (its end of life is in `CLAUDE.md`; the decision
to track that date and nothing else is not), and whether a Fiddle-only interpreter is a
supported one or a fallback with a printed loss. Each of those was decided when it came up.
The same discussion, with the same receipts, is owed here.

## PHP

**Believed.** That the benchmark harness measured the binding. Also that the FFI package was
the shipping shape.

**Measured.** The 0.4.0 CHANGELOG: "the ~0.7 ms it takes to load the native library sat inside
every figure: the README's batch table read 0.64 ms against 0.87 ms, a 1.4x gain, where the
batch is 76 µs against 285 µs for the loop, 3.8x". The same pass put the v4 at level to 1.2x
against a naive inline `random_bytes` v4, where the earlier tables had said 1.9–2x, and the
byte batch at 8x the object batch, not 100x. The 0.1.0 wrapper diet had taken a call from
~570 to ~305 ns "with no change of mechanism". Today's table (XPS, `php/README.md`, and it says
so in its first line) predates the extension and measured 1.0's FFI package: `newV4` 224 ns
against the naive 308, `newV5` 502 ns and 1.6x slower, `timestamp()` 293 ns against
`ramsey/uuid`'s 13.2 µs.

**Should have known.** By default PHP allows its built-in FFI under a web SAPI only from
preloaded code. The FFI package was never going to be the shape a PHP shop deploys, so the
extension (ext-php-rs, installed by PIE, one build per PHP minor) was chosen for reach, and
`composer require` stops working in 1.1 with no deprecated FFI package kept, on the judgment
that the user base is near zero. The spec's reason for not keeping a PHP-level wrapper over a
thin extension, "the ~100 ns PHP-level wrapper the spike measured", has no committed receipt:
the spike's comparison script was never committed. The extension has not been benchmarked on
the XPS yet. Both are debts, named here so they are not forgotten.

**Packaging.** Native libraries were committed to git in August because Packagist has no
packing step; the Composer archive carried every binding until export-ignore took it from 3.5
to 2.6 MB (0.5.0). Now: 18 zips on the release (8.3, 8.4, 8.5; NTS; six platforms), the
Windows build on stable with `RUSTC_BOOTSTRAP=1` for `abi_vectorcall` because dropping Windows
would have been a regression, no source fallback, no win-arm64 (PHP ships none), no ZTS. The
first PIE install from Packagist is proven only at the rehearsal or 1.1.0, and a red
acceptance after publish means a patch release. There was a seventh platform for a week:
Intel macOS, whose leg compiled an x86_64 PHP from php.net's source under Rosetta on every
uncached run because Homebrew bottles none, and which stalled 15 minutes at link on 2026-10-31
(run 41188206120) before the owner cancelled it and the leg. The rule that came out of it is
the shortest in this document: never build a language runtime. A leg takes the runtime its
platform packages, or there is no leg.

**The same ABI discussion, owed twice over.** PHP's extension ABI changes with every minor, so
each minor is its own build and seven more zips, and the choice of 8.3, 8.4 and 8.5 is a list
in the Forge with an end-of-life date beside it, not an argued rule. Unanswered in writing:
when a new minor joins (at its release, or its first release candidate, so that PIE users can
test ahead), whether ZTS is ever built (today a threaded SAPI gets nothing), what a user on an
unlisted minor gets (nothing: there is no source fallback and no FFI package), and what a
Windows user gets if `abi_vectorcall` never stabilizes. Python wrote its answers into the
CHANGELOG and the README as it made them; Ruby and PHP should get the same treatment before
the next minor of either forces the decisions ad hoc again.

## Swift

**Believed.** That a shared library copied "to a fresh temp file" at load was a load story.

**Measured.** Zero mallocs per call in 0.3.0: `newV7Batch(1000)` 86 → 17 µs and 1,002 → 1
malloc. The 0.4.0 re-measure cut the batch-over-loop headline from 42–50x to 6–7.5x. Today
(XPS, Swift 6.3.3, `swift/README.md`): `Foundation.UUID()` 1,325 ns against `newV4` 85 (16x),
`newV5` 122, `newV7` 107; a batch 57 → 3.7 µs, about 15x, with the README's own caveat that
"the multiple is the machine's as much as the binding's". There is no Apple-machine table for
the Swift binding. That is a gap, on the one platform where Swift is native.

**Should have known.** Static linking was the only load story that survives every platform:
Linux in 0.4.0 (musl and WebAssembly with it), macOS and Windows in 0.6.0 with the loader
gone, iOS and Mac Catalyst through an XCFramework in 0.7.0, Android (Swift 6.3+, API 28+) in
0.8.0. The twenty-two committed files under `swift/` went in Unreleased: the core comes from
the GitHub Release as a SwiftPM binary target, and the two zips' checksums in `Package.swift`
are written by CI after it verifies their attestations. Floor 6.2, for SE-0482. Not tvOS,
watchOS, visionOS, or the Intel-Mac simulator and Catalyst. Swift 6.3's `--build-system
swiftbuild` breaks dependents, and the README says so.

## TypeScript

**Believed.** That napi-rs with no wasm fallback was the right door, "decided from the start
rather than by measurement" (the spec says so itself), and the loss was predicted in the same
spec: "`crypto.randomUUID()` pools entropy and will likely be close or ahead on v4; the README
prints it either way".

**Measured** (M6, Node / Bun, from the README's own history):

| Core | `newV4()` | against `crypto.randomUUID()` | `newV7()` | against `uuid`'s `v7()` |
| --- | ---: | --- | ---: | --- |
| `getentropy` per id (2026-10-26) | 781 / 800 ns | 11.9x / 20x slower | 685 / 699 ns | 2.0x / 4.9x slower |
| `CCRandomGenerateBytes` (2026-10-30) | 239 / 258 ns | 4.1x / 6.4x slower | 155 / 166 ns | **2.3x faster** / 1.2x slower |

Nothing in the binding changed between the two rows. The core's entropy door did. A batch of
1000 costs 44.2 / 27.3 µs as strings and 3.62 / 2.93 µs as bytes, and `v7UnixMillis`, a call
that touches no randomness, 115 / 156 ns.

**Should have known.** Lesson 2 again, and it was already in the CHANGELOG. The TypeScript
README's explanation ("the rest is the random source") does not close arithmetically against
the core's own probe numbers, and now says "most of the rest".

**Packaging.** Ten npm packages (the root, eight platform packages and the WebAssembly
fallback), the root's `optionalDependencies` injected at pack time and never committed. The
first publish of each name is the owner's, by hand with a granular token as a `0.0.1`
placeholder, because npm Trusted Publishing attaches only to a package that exists;
`release.yml` is registered after. Node 22 is the floor until 2027-04-30, at 22.13 because the
WebAssembly runtime needs it. The wasm fallback was a 1.2 item and landed 2026-10-31 instead,
pulled forward because `--omit=optional` and an unlisted platform otherwise meant no addon; it
is a tenth package the root depends on outright, 6x to 13x behind the addon, which the README
prints, proven by installing the packed tarballs into fresh consumers rather than by the suite
alone (that is where the bundler finding came from: neither esbuild nor webpack can fold the
loader's platform `require`s into one file, so the working form is `hyperuuid` external; and
the s390x scenario found that the runtime reads its reference counts through host-endian typed
arrays over little-endian wasm memory, so on a big-endian host the process never exits: a
printed loss the scenario now asserts, not a platform). The
addon links Rust's std, so the core's glibc 2.34 floor is not its floor, and its own is not
stated yet.

## R

**Believed.** That extendr was the door. It is not: the R binding is a hand-written C shim
reached through `.Call`, over a staticlib wrapper crate whose only dependency is the published
`hyperuuid` crate at an exact version.

**Measured** (M6, `r/README.md`): `uuid_v4()` went from 1.31 µs against the `uuid` package's
9.3 (7.1x) to 779 ns against 9.92 (12.7x) when the core moved to `CCRandomGenerateBytes`, and
1000 ids 256 µs against 7.41 ms (29x). The time-based call loses: 2.46 µs against 1.89 for one
id (1.3x slower) and 119 against 101 µs for 1000 (1.2x), and `uuid_v7()` allocates 27.2 KB
where `uuid` allocates nothing. The README says why the comparison is uneven: `uuid` makes a
version 1 with a clock sequence and a fixed node, not a version 7.

**Not explained yet.** A single `uuid_v7()` costs about three times a single `uuid_v4()` in R
while the core's v7 is cheaper than its v4. The README explains only the 1000-id ordering. That
is an open question, not a receipt.

**Packaging.** A CRAN source package with the crates vendored into `src/rust/vendor.tar.xz`,
the core among them exactly as `cargo package` writes it at the pinned version, built offline;
the pin is a version file bumped with `DESCRIPTION`, and the vendoring script refuses a pin
that differs from the checkout's crate. Submitted by hand from the attested tarball the tag run
preserved. win-arm64 is experimental and untested; webR not attempted. The surface is kept
minimal on purpose.

## Go

Go is where every lesson landed at once, so it gets the longest section.

**Believed, in order.** August: purego and `dlopen`, then cgo with purego as the fallback. The
0.1.0 CHANGELOG: "Go is a deliberate control group and is *slower* per call than
`google/uuid`". The 1.0.0 root README: "`runtime.cgocall` ... ~40 ns on the box measured here,
structural, and not diet-able. There is no PyO3-for-Go", and "the binding stays in the roster
as a thought experiment and the control baseline". In between, 0.3.0 had already found the
truth and printed it: by-value shims cut allocations to zero and "per-call time on the
generators barely moves, because entropy, not the crossing, is what those doors cost; the
README's 'structural floor' claim is corrected".

**Measured.** The spike of 2026-10-30 (`docs/superpowers/spikes/2026-10-30-go-cgo-free/`)
translated the core's `wasm32-wasip1` build to Go source with wasm2go and put each candidate
random source behind it. On the M6, one instance behind one mutex (results
`macos-arm64-m6-*.txt`):

| ns | `version` (the crossing alone) | v4 | v7 | batch ×1000 | v7 ×12 goroutines | v4 ×12 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| cgo, 1.0's binding | 12.4 | 184 | 72 | 1,835 | 207 | 459 |
| translated, `crypto/rand` per call | 3.1 | 164 | 52 | 2,646 | 151 | |
| translated, AES-256-CTR stream | 3.2 | 30 | 35 | 2,058 | 135 | 121 |
| translated, no entropy at all | | | | 1,492 | | |

Read the first column against the second. The crossing was 9 ns on this machine. The v4 was
184 ns because the native core asks `CCRandomGenerateBytes` for every id on Apple platforms,
exactly as `google/uuid` asks the operating system; the translated core with the same per-call
habit measured 164, and with a buffered stream 30. On the XPS the crossing was larger (about 17
ns in WSL2 and 26 on Windows, from the `version` rows), and the 1.0.0 README's "about 40 ns"
was roughly the whole cgo `version` call there, not the crossing. The author of this document
repeated the 40 ns explanation in the new Go README this week, on the M6, where it is wrong
by a factor of four, and found it only by reading the spike's own result files back. That is
lesson 2 for the third time.

**Where it shipped** (M6, Go 1.27.3, `-count 6` medians, 2026-10-31; the raw output is
committed as `results/macos-arm64-m6-shipped-host.txt`):

| | hyperuuid | `google/uuid` | cgo, 1.0 |
| --- | ---: | ---: | ---: |
| `NewV4` | 37.0 ns, 0 allocs | 167 ns, 1 alloc: **4.5x faster** | 184 |
| `NewV7At` | 37.3 ns | 202 ns: **5.4x faster** (explicit timestamp against a clock read) | 72 |
| batch of 1000 | 2.84 µs | | 1.84 µs: **1.5x slower** |
| `V7Timestamp` | 6.6 ns | `id.Time()` 1.4 ns: **4.8x slower**; use `id.Time()` | |
| v4, 12 goroutines | 56.8 ns ±45% (an earlier run 16.7 ±74%) | | 459 |

Linux arm64 in Docker on the same Mac (`linux-arm64-docker-m6-shipped.txt`): v4 55.1 ns level
with `google/uuid`'s 57.8, v7 54.9 against 98.6, both **1.3–1.4x behind** the cgo binding's own
41 and 40 ns there, because on Linux the native core already drew from its ChaCha20 pool and
there was no per-call syscall for the stream to remove; the batch 3.3 µs against cgo's 3.46,
level in this run and 5.0 in an earlier one; 12 goroutines of v4 at 10.2 ns against cgo's 4.9,
**2x slower** under contention. In the browser and on WASI (`-shipped-js-node.txt`,
`-shipped-tinygo-wasmtime.txt`) the OS source serves every call and `google/uuid` is level or
ahead per single call: its v4 60 ns against 154 under TinyGo on wasip1, 491 against 537 under
stock Go in Node. The batch is the binding's only win there.

**Should have known.** Three things, all in the record before the work started. The 0.3.0
sentence above. The 2026-10-27 TypeScript README, which printed its Apple losses and named the
per-id OS call as the cause. And the prior art: `ncruces/go-sqlite3` and `wasilibs/go-re2`
had both already moved from a wasm runtime to wasm2go, and a study the ROADMAP cites found
18 of 24 cgo-decision issues in the most-starred Go repositories were moves away from cgo.

**Packaging.** Thirteen static archives were committed under `go/staticlib/` so that `go get`
would work, which made every release a commit of binaries, made `CGO_ENABLED=0` a compile
error (0.6.0), and once linked another platform's archive into Android and iOS (0.7.0). The
ROADMAP's plan to keep the archives off master on release-only commits, and before that in a
separate repo behind a vanity import path, was superseded on 2026-10-30 by not having them.
`go/internal/core/core.go` is now the core, regenerated by `go generate` with a pinned rustc
(a reproducibility constraint, not a floor) and diffed in CI; a binary carries it like any
package on every GOOS and GOARCH Go has. The cost accepted for that: the batch, Linux
contention, and a primary instance that holds 1 GiB for the life of the process after a
`MaxV7Batch`.

**What a control found.** The FIPS selection test (a child process under `GODEBUG=fips140=on`
must report the OS source) was written to prove the stream is never chosen there. On its first
run in the Android emulator it failed before our code ran: `fips140: verification mismatch`
from Go's own integrity check. The cause is in the linker, not the module: for external linking
Go reads the FIPS section addresses back out of the linked executable, GNU ld writes
dynamic-relocation addends into the data, and the NDK's lld does not unless given
`--apply-dynamic-relocs`, so Go hashed four empty ranges. Reproduced on `linux/amd64` in Docker
with `-fuse-ld=lld` (Go 1.26), fixed with that flag, control kept. A test that exists only to
pin a negative found a toolchain bug on the one platform that both links externally and
links with lld; the lesson is the one about running every control on every platform, not
just the platform it was written on.

## C and C++

The newest door and the only one without a benchmark: one archive per platform on the release,
a header generated by cbindgen from the Rust source, a C++23 header over it. The hand-written
header it replaced had never mentioned return code 6, and the shared library's install name
was the build machine's path until a fix in Unreleased; the Android and WASI archives are
packed but not run. The ROADMAP's performance column for C says "not measured", which is the
right thing for it to say until it is.

---

## How the measuring is done now

Every table names its machine and date. The XPS tables (linux-x64 and Windows) are the
C#, Java, Python, Ruby, PHP, Swift and Rust rows; the M6 tables are Go, TypeScript and R, and
Go's Linux arm64 in Docker on it. The two are never merged, and the README says so where it
lists both. Every linux-x64 v7 row carries a mark saying it predates the per-millisecond
re-seed of 2026-10-27; the M6 rows post-date it and the Apple entropy switch of 2026-10-28; the
PHP rows predate the extension; the C# rows predate the .NET 11 move; the Python, Ruby and PHP
rows predate the pool's one-second key limit, which adds 3 to 4 ns. Each binding uses its own
ecosystem's harness (Criterion, BenchmarkDotNet, JMH, `testing.B` with benchstat, phpbench
with Xdebug off, package-benchmark, mitata, R's `bench`, and Python's and Ruby's own benchmark
packages) and every case is warmed before it is timed. Go's shipped numbers have their raw output committed
beside the spike's; the others have the harness and the command.

## What the release machinery taught

- **Registry versions can never be replaced.** 0.2.0 is unsigned on crates.io and absent from
  Maven Central, for good. The rule since: never retag, never rebuild; recover by
  re-dispatching from the preserved packages.
- **Install from the real registry before believing the publish.** The osx-x64 wheel of 0.4.0
  failed at install after the other registries had published. Every package is installed into
  a clean project in CI now, and the CDN's bytes are verified against the attested bytes
  (0.2.0), so "the registry stores an upload verbatim" is measured each release rather than
  assumed.
- **Two acts at one tag.** The tag push packs, attests (build provenance plus one release
  attestation naming every shipped file) and preserves without uploading; a dispatch publishes
  from the preserved bytes. The publish dispatch is also the recovery path, so every release
  exercises it.
- **The shared pipeline is called at its head, unpinned**, so a fix reaches every consumer at
  once, and Forge merges are held while a release is in flight. A CI run from before a version
  bump is not reusable after it (0.4.0).
- **Floors follow upstream support, and the lists that move together are written down.** Go
  1.26, Python 3.11, Ruby 3.3 (on Fiddle), PHP 8.3, Node 22, .NET 10, JDK 21, Swift 6.2, R
  4.2, each with the date it moves and every file that moves with it listed in `CLAUDE.md`.
- **Attestations over every file, and a first publish by hand where a registry needs one to
  exist** (npm's ten names) before trusted publishing can attach.

## What the pull-request board taught

The board was one `build-native` matrix until 2026-11-01: every binding on every host on
every push, 84 jobs and 24 minutes for a one-line change. It is now one Forge workflow per
concern, the core on seven hosts first and then one job per host × binding, each binding's
run gated on whether the pull request touched it, with one aggregate job red on any failure
or on any job skipped while its gate said run (run 41255218791, the first; the receipts are
in the spec, `docs/superpowers/specs/2026-10-31-pr-board-design.md`).

- **Measure the barrier, not the leg.** Every suite waits for the slowest core leg, and on the
  first board that was Windows arm64 at 4m17 against Linux arm64 at 1m39, a 2m38 spread the
  spec had guessed at one minute. The lever (one caller per host) is written down and not
  pulled on one run.
- **A lint that is slower than the core is not a gate, it is the critical path.** Nine of ten
  lints finished in 12 to 65 seconds on a single arm64 runner; R's took 5m19, so R's suites
  waited on their lint instead of the core. The rule was written before the number existed
  and the number says to apply it.
- **The thing you called release-only was on every PR all along.** The sdist fall-through
  (13m15) was the longest job on the board, as it had been on the old one; the spec said it
  was release board, the Forge said otherwise in a comment, and the measurement sided with
  the Forge. Read the gate, not the prose.
- **Fail closed, then prove the token.** The classifier returns the whole board on anything it
  does not understand, an empty list, the API's 3,000-file cap, or a malformed line, and the
  job runs under `pipefail` so an API failure is red, not docs-only. The one unproven step
  between a pull request and its board, reading the files endpoint with the default token,
  took seven seconds on the first run.
- **The warnings that survive a rewrite were never the rewrite's.** All three on the first
  board (two from `R CMD build` on a Windows checkout without execute bits, one from an
  unsigned Windows Swift installer) were on the old board's last green run.

## Open debts, so they are not lost

- Java: the JNI path's JMH receipt on JDK 21, promised and not yet measured.
- PHP: the extension on the XPS, and the spike's "~100 ns wrapper" that has no committed
  script.
- Swift: an Apple-machine table.
- Go: the x64 columns and the parallel v4 there, awaiting the XPS; the Alpine run, iOS,
  Android and the cross-compile sweep are proven by CI, not locally.
- TypeScript: its own glibc floor, stated and enforced. The WebAssembly fallback on Bun (1.4.4's
  `node:wasi` has no reactor `initialize`), and in Firefox and Safari: untested, stated so. The
  big-endian exit hang is upstream's (emnapi's byte-order mix); not reported yet.
- Ruby and PHP: the ABI decision written down as Python's is (which interpreter minors the
  native extension carries and why, what a new one costs, when one leaves, and what everyone
  else gets).
- R: why a single v7 costs three times a single v4.
- The core's entropy note gives a whole `new_v7` at 60 ns and its 11-byte draw at 67.5 ns in a
  separate C probe; one of the two measurements is the wrong shape for the comparison.
- The pull-request board: the docs-only and one-binding controls, not yet run; `lint-r`
  beside its suites, or cut below the core legs' time; the sdist fall-through's PR-mode gate,
  a Forge decision; the Windows arm64 core leg's 4m17.
- The obvious next experiment, given lesson 2: a buffered stream on Apple platforms inside the
  core itself, which is what Go did outside it, and which would move the TypeScript and R Apple
  rows. Not started, and not promised here.
