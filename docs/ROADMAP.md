# HyperUuid Roadmap

Oct 31, 2026 · @Buvy

## Where it stands

HyperUuid now ships nine bindings over one Rust core, with TypeScript and R landed as of master commit `12fe74d` (Oct 26, 2026), and C and C++ reach it from the release's C archives (in the tree, Oct 30). Fork safety and the counter re-seed are solved, and no binary is committed anywhere (Phase 2, and Go's by Phase 3 item 6); runtime floors are tracked below.

| Binding | Mechanism | Distribution | Floor | Single-call result |
| --- | --- | --- | --- | --- |
| C# | LibraryImport over the C ABI | NuGet | .NET 10 (desktop, server, browser); .NET 11 (all platforms) | 7.5-7.9x faster than `Guid.NewGuid()` |
| Java | JNI (21–24), FFM (25+) | Maven Central | JDK 21 | 2.8-5.1x faster than `UUID.randomUUID()` |
| Python | PyO3 extension | PyPI, wheels and an sdist | 3.11 | 3.4-4.2x faster than stdlib |
| Ruby | Magnus, Fiddle fallback | RubyGems | 3.3 (on Fiddle) | 2.1-5.5x faster than `SecureRandom.uuid` |
| PHP | ext-php-rs extension | PIE: Packagist, 18 zips on the GitHub Release | 8.3 | 1.2-1.4x faster than a naive inline v4 (1.0's FFI package; the extension not yet measured) |
| Swift | static library | git tag; core zips on the GitHub Release | 6.2 | 11-16x faster than `Foundation.UUID()` |
| Go | the core as Go source (wasm2go) | git tag, no binaries | 1.26 | 4.5-5.4x faster than `google/uuid`; batch 1.5x slower than the cgo binding it replaced |
| TypeScript (new) | napi-rs addon | npm: root package + 8 platform packages + the WebAssembly fallback package | Node 22.13, Bun | v4 4.1-6.4x slower than `crypto.randomUUID()`; v7 2.3x faster than `uuid` `v7()` on Node, 1.2x slower on Bun |
| R (new) | `.Call` into a vendored static library | CRAN source package | R 4.2 | v4 12.7-29x faster than `uuid`; time-based 1.2-1.3x slower |
| C, C++ (new) | the C ABI: a generated C header, a C++23 header over it | GitHub Release: one archive per platform (CMake, pkg-config) | C99; C++23 | not measured |

What the TypeScript and R landing settled:

- **TypeScript** published its loss up front, as planned. Batches recover it: `newV7Batch(1000)` is 3.5x (Node) to 6.2x (Bun) faster than 1000 single calls, and since the switch to `CCRandomGenerateBytes` a single `newV7()` on Node beats `uuid`'s `v7()` (Apple M6, 2026-10-30). Where no addon fits (`--omit=optional`, an unlisted platform) the root falls back to the WebAssembly package, at about 1.5 µs an id (2026-10-31).
- **R** went straight to CRAN as a source package with vendored crates, rather than r-universe first. It exposes raw-matrix doors (`uuid_v4_raw`, `uuid_v7_raw`, `uuid_to_raw`) and the vectorised SQL-order conversions.
- **Fork safety is done.** The Linux entropy pool lives in a mapping marked `MADV_WIPEONFORK`, refuses to run if that flag is not honoured, and `rust/tests/entropy_pool.rs` proves a forked child draws different bytes. No key outlives one second, which also covers snapshot restores.
- **The v7 counter re-seeds on every new millisecond** (1.1.0; Phase 0 below). Two IDs no longer reveal how many were minted between them across a millisecond.
- **The About text is fixed** (Oct 29): it names all nine languages and v4, v5, v7 and the v8 SQL Server form, with no v6. C and C++ join it when the owner says go.

## Standing policies

Four rules decide every item below.

1. **Supported runtimes only.** A binding's floor follows upstream support; a version past end of life is never a floor.
2. **Production-reality review before release.** Ask how teams actually deploy the language and where the install breaks: a practitioner, or an adversarial Forseti pass.
3. **Losses print next to wins,** as Go, TypeScript and R already do.
4. **Binaries ship as attested release assets, not git commits,** except where the ecosystem's tooling leaves no other path.

## Phase 0: Correctness and credibility

The counter re-seed is the one change that touches persisted behaviour; the rest is wording.

- [x] **Re-seed the v7 counter on each new millisecond.** Re-initialise it randomly when the timestamp advances (RFC 9562 §6.2 Method 1; check §6.9). Two IDs used to reveal how many were minted between them. Rework the batch roll-forward and the wrap rules in "v7 ordering, precisely"; no corpus change: the layout is untouched.
- [x] **Fix the About text:** drop v6, name all nine languages. Done Oct 29.
- [x] **Reposition the README.** Lead with SQL Server ordering, v7 ordering guarantees and byte-identical v5. Speed supports; it does not lead. Done Oct 30, in the tree: the three lead, and benchmarks, linking and publishing, WebAssembly and provenance detail moved verbatim to `docs/`. 73 KB to 42 KB, not a third: the rest is the SQL Server and v7 depth the lead points at, and Key features.
- [x] **Go README:** put the cgo requirement and the `CGO_ENABLED=0` compile error in the opening lines. Moot since Phase 3 item 6: there is no cgo requirement.
- [x] **Fork safety:** done (`MADV_WIPEONFORK` pool plus a fork test), and the README's key features say so.

## Phase 1: Supported-runtime floors

C# and Java are the floors that lock out the financial and insurance buyers; the rest are small.

| Binding | Change | Why |
| --- | --- | --- |
| [x] C# | Target `net10.0` and `net11.0` | .NET 8 ends support Nov 10, 2026; 10 is the LTS conservative shops run |
| [x] Java | JNI path for JDK 21 in a multi-release JAR's base, FFM under `META-INF/versions/25`; floor 21, no JDK 17 | FFM is only a preview in 21 (final in 22), so 21 needs JNI. In the tree; ships in 1.1.0 |
| [x] Ruby | Not done, by decision (2026-10-30): 3.3 stays on Fiddle until its end of life (2027-03-31) | A third Magnus ABI widens every platform gem and the Forge's ABI lists for five months of support; 3.3 runs correctly on Fiddle today, only slower |
| [x] Python | Publish an sdist via maturin | FreeBSD, ppc64le and s390x (Linux on IBM Z at banks) can build from source |
| [x] PHP | Floor moves to 8.3 | 8.2 reaches end of life Dec 31, 2026 |
| [x] Rust | `uuid` crate `From`/`Into` behind a feature flag | Most Rust code already holds `uuid::Uuid` |

## Phase 2: Binaries out of git

The end state is a main repo of source only, with every binary an attested release asset.

[x] **PHP: an ext-php-rs extension installed by PIE.** Done 2026-10-30, in the tree: the whole API is a Zend extension built from the core, the root `composer.json` is a `php-ext` package, and PIE downloads the prebuilt zip for the platform and minor from the GitHub Release: 8.3, 8.4 and 8.5, NTS only, on six platforms (Linux x64 and arm64 on glibc and on musl, macOS arm64 and Windows x64; Windows on stable Rust with `RUSTC_BOOTSTRAP=1` on that one build step), 18 zips a release, each attested and named in the release attestation. The FFI package is retired, not kept beside it, and `php/src/` with its committed libraries is gone. `release.yml` ends with PIE installing the release on every platform and minor; the first install from Packagist is proved at the rehearsal or 1.1.0. ZTS waits until someone asks (FrankenPHP). Ships in 1.1.0.

[x] **Swift: remote `binaryTarget`.** Done 2026-10-29, in the tree: the root `Package.swift` names the bundle and the XCFramework as two zips on the GitHub Release by URL and checksum, and the binaries left git. A spike proved remote static-library bundles on Swift 6.2.4 and 6.4.1 (glibc, musl, wasm, macOS, iOS, Mac Catalyst); Windows and Android are CI's. Ships in 1.1.0.

**Go: superseded (2026-10-30).** The plan was to keep the archives off master by putting them on release-only commits behind the `go/vX.Y.Z` tags; before that, a separate repo behind a vanity import path. Both are moot: the Go binding moves to the core translated to Go source (Phase 3, item 6), which leaves no Go binaries anywhere.

## Phase 3: New bindings and follow-ups

C is next because it is nearly free and opens the core to every language that speaks the C ABI.

1. [x] **C.** Done 2026-10-29, in the tree: the header is generated by cbindgen and checked by a test, and the release attaches one archive per platform (fifteen, iOS and Mac Catalyst included) with the shared and static libraries, CMake and pkg-config files, built against in CI on the eight desktop RIDs and the four Apple mobile ones. The contract is the crate's semver, read at run time through `hyperuuid_version()`: a minor adds exports, only a major changes or removes one. The shared library names itself (`@rpath/` install name, unversioned soname). Ships in 1.1.0.
2. [x] **TypeScript: wasm fallback.** Done 2026-10-31, in the tree, pulled forward from 1.2: a tenth npm package, `@skunkwerkx/hyperuuid-wasm32-wasip1`, the core built for `wasm32-wasip1` by napi-rs and a plain dependency of the root, loaded where no platform package fits; proven by the whole suite under the forced fallback on three OSes and by consumer tests of the packed tarballs (`--omit=optional`, a Mac lockfile on Linux, the exact Node floor, esbuild and webpack, whose working form is `hyperuuid` external) and in Chromium; s390x under emulation found the big-endian loss (the runtime's reference counts read host-endian, so the process never exits) and records it. Design: `docs/superpowers/specs/2026-10-31-typescript-wasi-fallback-design.md`.
3. [x] **Entropy on macOS.** Done another way: Apple targets call `CCRandomGenerateBytes` once per request instead of `getentropy` per id (a single v7 on the Apple M6 went from 594 to 60 ns, 2026-10-28). The ChaCha20 pool stays Linux only (Android keys a ChaCha20 stretch for batches and takes `getrandom` for single ids). In the tree; ships in 1.1.0.
4. **R: hold the API where it is.** CRAN is a 1.1 first (the tag run attests the tarball and the owner submits it by hand); keep HyperUuid's R surface minimal and spend R effort on HyperCast and HyperTabular, the real targets for the actuary.
5. [x] **C++.** Done 2026-10-30, in the tree: `include/hyperuuid.hpp`, header-only over the C header, `std::expected` (C++23) for every generating call and `errc` as a `std::error_code` enum, shipped in every C archive and built against in CI beside the C consumer. `check_exports.py` holds it to every export. Add vcpkg and Conan ports when someone asks.

6. [x] **Go: one backend, the core translated to Go source with wasm2go.** Done 2026-10-31, in the tree: `go/internal/core/core.go` is wasm2go's translation of the core's `wasm32-wasip1` build, regenerated by `go generate` with a pinned rustc (1.99.1) and a 64 KiB guest stack (256x the smallest stack that passes the whole suite), diffed in CI on every push and regenerated by release preparation at the version. cgo, the thirteen archives, the TinyGo archive, the freshness check and the `hyperuuid_local` tag are gone; `go get` is the whole install on every GOOS and GOARCH Go has, with `CGO_ENABLED=0`. The random source is a hardened AES-256-CTR stream where `golang.org/x/sys/cpu` reports the features Go's own AES assembly requires (amd64, arm64, s390x; never under `purego`, TinyGo or FIPS 140 mode), rekeyed after 1 MiB or one second with the buffered keystream discarded, and `crypto/rand` per checked read everywhere else. v7 runs on one primary instance behind its mutex; v4, v5 and inspection take a pooled instance when the primary is busy. `NewV5` has a stated limit, `MaxV5Name` (2^31 − 1 on a 64-bit host, the guest's slice limit), and `ErrNameTooLong`; a streaming v5 through the C ABI would restore the old range and is not planned. The shipped binding on the Apple M6 (Go 1.27.3, `-count 6` medians): `NewV4` 37.0 ns (`google/uuid` 167; cgo 1.0 on the same machine 184), `NewV7At` 37.3 ns (`google/uuid`'s `NewV7()` 202; cgo 72), batch ×1000 2.84 µs (cgo 1.84: the batch loss, 1.5x), `V7Timestamp` 6.6 ns (`id.Time()` 1.4), 12 goroutines of v4 56.8 ns a call aggregate, ±45%, an earlier run 16.7 ±74% (cgo 459); the raw output is committed beside the spike's results (`macos-arm64-m6-shipped-*.txt`). Linux arm64 in Docker on the same Mac: v4 55.1 ns level with `google/uuid` (57.8) and v7 54.9 against 98.6, both 1.3–1.4x behind the cgo binding's own 41 and 40 ns there; batch 3.3 µs against cgo's 3.46 (5.0 in an earlier run); parallel v4 10.2 ns against cgo's 4.9. In the browser and on WASI the OS source serves every call: 537 ns a v4 under stock Go `js/wasm` in Node, 154 ns under TinyGo on wasip1 under wasmtime, and `google/uuid` is level or ahead per single call on both (its v4 60 ns under TinyGo), the batch being the binding's win there; the whole suite runs on all three in CI (item 7). The x64 columns await the XPS run. Design and plan: `docs/superpowers/specs/2026-10-31-go-wasm2go-design.md`. The spike that decided it follows.

   The Go binding needed cgo, which most Go code never has: 11.3% of the 920 most-starred Go repositories import `C`, and 18 of the 24 cgo-decision issues in those repositories were moves away from it ([Chen et al., arXiv 2508.09875](https://arxiv.org/html/2508.09875v1)). The paper measures no call overhead, so every number below is this spike's.

   [wasm2go](https://github.com/ncruces/wasm2go) (v0.4.19, MIT) translates a wasm module into one Go source file with no dependencies beyond the standard library. The core built for `wasm32-wasip1` (8.7 KB) becomes 45 KB of Go whose only import is WASI `random_get`, and the Go compiler compiles it like any other package. It is the prior art's current answer: `ncruces/go-sqlite3` and `wasilibs/go-re2` both moved from wazero to wasm2go (go-re2 keeps cgo as an opt-in `re2_cgo` tag), and `pgaskin/go-lightningcss` does the same for a Rust crate. The engine stays one: Rust core, then wasm, then Go, all mechanical.

   Spike, all `CGO_ENABLED=0` except cgo: one module instance behind a mutex (the v7 counter lives in its memory). The wasm2go rows differ only in where `random_get` gets its bytes. Go 1.27.3, Apple M6 (Mac18,5) native, Linux arm64 in Docker on the same machine; `-count 6`, benchstat medians; the Linux legs vary by up to ±26% and the starred cell by ±53%. Zero allocations in every row.

   | ns/op, macOS / Linux arm64 | `version` | `new_v4` | `new_v7` | `v7_unix_millis` | batch ×1000 |
   | --- | --- | --- | --- | --- | --- |
   | cgo (today) | 12.6 / 18 | 188 / 38 | 74 / 40 | 14.1 / 13.7 | 1,860 / 3,480 |
   | wazero v1.12.0, compiled | 10.9 / 16 | 215 / 99 | 97 / 90 | 12.7 / 12.9 | 3,690 / 9,500 |
   | wasm2go, `crypto/rand` | 3.9 / 5.2 | 174 / 84 | 55 / 75 | 4.1 / 7.4 | 3,670 / 11,400* |
   | wasm2go, Go-side ChaCha20 (`x/crypto`) | 3.2 / 4.7 | 28 / 56 | 26 / 53 | 3.8 / 6.8 | 5,210 / 5,250 |
   | wasm2go, Go-side ChaCha8 (`math/rand/v2`) | 3.4 / 3.5 | 24 / 41 | 24 / 41 | 4.2 / 4.1 | 5,130 / 4,570 |
   | wasm2go, the core's ChaCha20 pool in the guest | 3.2 / 5.1 | 56 / 86 | 50 / 78 | 3.8 / 7.0 | 11,000 / 10,960 |

   The arm64 table predates the core's WebAssembly batch walk and AES-CTR (below); its translated batch figures are the old walk's. The re-run on the M6 is next.

   The same harness on x64, 2026-10-30: the Dell XPS 15 9510 (i9-11900H), Go 1.27.3, rustc 1.99.1, Linux amd64 in WSL2 (Fedora 44, kernel 6.18) and Windows 11 natively (MinGW-w64 gcc 16.2 for cgo), on the core with the WebAssembly batch walk; `-count 6`, benchstat medians, within ±6% except Windows' `version` cells (up to ±11%). Both cgo controls link the current core: on Windows the archive the Forge's `build-static-libs.sh` trims for Go, built from this checkout, not the committed 1.0.0 one. The wasm and the translated Go are byte-identical on both. wazero was not re-run. Zero allocations in every row.

   | ns/op, Linux amd64 (WSL2) / Windows amd64 | `version` | `new_v4` | `new_v7` | `v7_unix_millis` | batch ×1000 |
   | --- | --- | --- | --- | --- | --- |
   | cgo (today) | 27.7 / 37.2 | 60 / 92 | 66 / 95 | 28.4 / 37.7 | 4,330 / 3,100 |
   | wasm2go, `crypto/rand` | 10.8 / 11.8 | 74 / 118 | 70 / 118 | 13.1 / 12.4 | 11,990 / 5,370 |
   | wasm2go, Go-side ChaCha20 (`x/crypto`) | 10.6 / 11.5 | 80 / 58 | 79 / 57 | 12.9 / 12.4 | 13,010 / 12,500 |
   | wasm2go, Go-side ChaCha8 (`math/rand/v2`) | 10.5 / 11.7 | 57 / 37 | 60 / 41 | 12.8 / 12.4 | 8,020 / 7,580 |
   | wasm2go, Go-side AES-256-CTR (`crypto/aes`) | 10.5 / 11.4 | 64 / 48 | 84 / 60 | 12.8 / 12.7 | 5,270 / 5,090 |
   | wasm2go, the core's ChaCha20 pool in the guest | 10.5 / 11.6 | 111 / 81 | 103 / 73 | 12.7 / 12.3 | 22,120 / 21,750 |
   | wasm2go, no entropy at all (the walk's floor) | | | | | 4,330 / 4,240 |

   - **Calls cost nothing to cross.** The translated core is ordinary Go code, so `version` and `v7_unix_millis` run 3–4x faster than cgo on arm64 and 2–3x on x64, where cgo's `version` call costs 27.7 ns (WSL2) and 37.2 ns (Windows) against 10.5–10.8 and 11.4–11.8 translated: a crossing of about 17 and 26 ns.
   - **Generation is decided by the random source.** On Linux about 45% of a `crypto/rand` v7 is `vgetrandom` and its locking (CPU profile). A Go-side stream keyed from `crypto/rand` and rekeyed after 1 MiB or one second (the native pool's rule) brings Linux arm64 to cgo's level with ChaCha8, or 1.4x behind with ChaCha20. Linux amd64 is at cgo's level with ChaCha8, 1.1–1.3x behind with AES-CTR (its v7, an 11-byte draw, costs more than its v4) and 1.2–1.3x with ChaCha20. On Windows every Go-side stream beats cgo, whose v4 and v7 cost 92–95 ns there: ChaCha8 by 2.3–2.5x, AES-CTR by 1.6–1.9x, ChaCha20 by 1.6–1.7x; `crypto/rand` per call is 1.2–1.3x behind. `x/crypto/chacha20` has assembly for arm64, loong64, ppc64x and s390x but not amd64, where it runs generic Go; `math/rand/v2`'s ChaCha8 has amd64 assembly, and `crypto/aes` runs on AES-NI. Go programs cannot fork without exec, so a Go-side stream needs no fork guard; the one-second rule covers snapshot restores, as it does natively. The core's own pool inside the guest keeps entropy policy in the core but is scalar code: slowest batch.
   - **The batch is the one loss, and AES-CTR with the WebAssembly walk closes most of it.** A batch of 1000 draws 6 KB in one `random_get`, then walks the items. On x64 the draw costs under 1 µs with AES-CTR, against 3–9 µs with the ChaCha streams, and the walk is the rest: no entropy at all costs 4.3 µs on WSL2 (cgo's whole batch) and 4.2 µs on Windows. With AES-CTR the batch is 1.2x behind cgo on Linux amd64 and 1.6x on Windows (cgo's Windows batch is unusually fast: `ProcessPrng`); ChaCha8 is 1.9x and 2.4x behind, ChaCha20 3.0x and 4.0x. Before the WebAssembly walk the same batches were 30% slower: on x64 they were 2.2–5.1x behind cgo with the Go-side streams. On arm64 (old walk) the Go-side streams were 1.3x behind cgo on Linux and 2.7x on macOS.
   - **The WebAssembly batch walk.** WebAssembly has no byte-swap instruction, so the core's big-endian stores became chains of shifts in the translated Go. Only WebAssembly builds now compile a walk that builds each item as two little-endian words (`rust/src/v7.rs`, `fill_batch_words`); every native target keeps its walk, which measured faster natively (the word walk cost native batches 10–13% when tried everywhere). `walk-ab.sh` in the spike measures the two against each other, alternating: translated, 7.65 → 5.39 µs (−30%) with AES-CTR and 6.73 → 4.50 µs (−33%) with no entropy on WSL2, and 7.52 → 5.56 µs (−26%) and 6.36 → 4.19 µs (−34%) on Windows; native, unchanged (5.17–5.23 against 5.07–5.14 µs on WSL2, 3.52–3.63 against 3.39–3.70 µs on Windows). A test holds the two walks to each other byte for byte, on s390x too, and the no-panic proof links the WebAssembly one on the host (`--cfg hyperuuid_words_walk`): `no-panic` cannot see a panic that aborts, as every WebAssembly build's does.
   - **Persisted bytes hold.** 194 corpus vectors (v5, SQL order both ways, refusals, timestamps, inspection) are byte-identical through the translated core natively on arm64 (macOS, Linux) and amd64 (Linux, Windows) and, under emulation, on 386, riscv64, ppc64le and big-endian s390x. A corrupted v5 vector and a corrupted SQL-order vector each fail (negative control). `v7_layout.json` needs injected entropy and was not run.
   - **Every target.** The package cross-compiles for linux/{amd64, 386, arm, riscv64, ppc64le, s390x, loong64, mips64}, windows/{amd64, arm64}, freebsd, openbsd, plan9, aix, illumos, android and ios (an iOS executable needs external linking under any backend; that is Go's rule).
   - **The browser, without TinyGo-only code.** Translated Go compiles under stock Go and TinyGo alike. Under node (V8), on the M6 (macOS) and the i9-11900H (Linux amd64 in WSL2: node 24, TinyGo 0.44.0, `-count 3`):

     | ns/op, macOS / Linux amd64 | `new_v4` | `new_v7` | `v7_unix_millis` | batch ×1000 |
     | --- | --- | --- | --- | --- |
     | TinyGo today (the core's archive linked in) | 480 / — | 445 / — | 4.9 / — | 2,100 / — |
     | TinyGo, wasm2go, `crypto/rand` | 99 / 234 | 89 / 238 | 24 / 49 | 18,400 / 29,300 |
     | stock Go `GOOS=js`, wasm2go, ChaCha8 | 88 / 194 | 103 / 247 | 14 / 30 | 11,540 / 31,700 |
     | stock Go `GOOS=js`, wazero (interpreter) | ~6,000 / — | ~7,800 / — | ~920 / — | 1,530,000 / — |

     Single calls are 4–5x faster than today's TinyGo path on macOS; the batch is 5–9x slower. Today's TinyGo path was not measured on x64. The x64 column is the core with the WebAssembly batch walk. Against the old walk's run, TinyGo's batch went from 42,700 to 29,300 ns, but both stock Go `GOOS=js` batches went the other way (ChaCha8 26,700 to 31,700, `crypto/rand` 16,900 to 20,800). Those were 3-run medians from separate sessions; the interleaved A/B on the M6 (2026-10-31, `walk-ab.sh` with `WALK_WASM=1` and `WALK_BROWSER=1`) confirms the direction. The raw module gains: in V8 (Node 24) its batch went from 2.65 to 2.11 µs (−20%), under wasmtime 50 from 13.7 to 13.0 µs (−5%). The translated core compiled to WebAssembly a second time, by Go itself (`GOOS=js`), loses: 36.0 to 41.5 µs with the AES stream (+15%), 5.8 to 10.6 µs with no entropy (+84%). That is the one target where the word walk costs, the binding takes the OS source there (its batch is 13.0 µs), and the walk stays for every other target. Stock Go `GOOS=js` with `crypto/rand` per call costs about 1,080 ns a v4 on x64, so a browser build wants a Go-side stream.
   - **Reproducible.** The wasm is byte-identical built on macOS, on Linux arm64 and amd64, and on Windows (rustc 1.99.1), and wasm2go's output is deterministic, so the generated file can be committed and checked the way `include/hyperuuid.h` is: CI regenerates and diffs. wasm2go pins through `go.mod`'s `tool` directive, as `go-sqlite3-wasm` does.
   - **The sandbox is real** (wasm2go's SECURITY.md): code lives outside linear memory, the call stack is Go's and can't be addressed from it, and every access is bounds-checked, so a core bug panics instead of corrupting the Go heap. Its stated threat model assumes a trusted module, which ours is.
   - **Ruled out:** wazero (the table above; its defaults are also a deterministic random source and a fake wall clock, and it interprets on any GOARCH other than amd64 and arm64 and inside wasm), purego (`SyscallN` 107–125 ns on Linux, 1–2 allocs) and goffi (113–124 ns, an allocation per call in `layoutArgs`, contrary to its README). purego and goffi also each ship a `fakecgo`, so they can't link into one binary, and both need a shared library extracted at run time.

   Decided (owner, 2026-10-30): one Go backend, the translated core, committed as generated source with a regenerate-and-diff check. cgo, the thirteen archives, `backend_tinygo.go` and its archive, the core freshness check and the release-only-commit plan (Phase 2) all go. Go binds the same way on every target, and `go get` is the whole install, with or without a C toolchain. The batch loss is accepted for that reach and for one way of binding. The direction for it (owner, 2026-10-30): close it inside the translated path with no native regression, through Go-side AES-256-CTR and the WebAssembly batch walk, both above. What was open then, and how it closed (2026-10-31):
   - arm64: the M6 tables in the spike's README are the AES-CTR row and the walk's A/B (−29% with AES, −36% with no entropy on macOS arm64; −28% and −35% on Linux arm64), and the shipped binding's numbers are above.
   - The browser's batch walk, measured: the raw module's batch gains 20% in V8 and 5% under wasmtime from the word walk; the translated core compiled to WebAssembly a second time by Go (`GOOS=js`) loses 15% with the AES stream and 84% with no entropy (the table's note above). The walk stays: it is one committed translation for every target, and the browser takes the OS source.
   - The random source: a buffered AES-256-CTR stream (512-byte keystream buffer, direct generation for larger draws), selected by Go's own CPU-feature rule and never under `purego`, TinyGo or FIPS 140 mode; the OS source elsewhere, including where Go's AES would be software (riscv64, 386, the browser). ChaCha8 and ChaCha20 and the core's pool in the guest were not taken.
   - wasm2go pre-1.0: accepted, pinned in a tool module of its own (`go/internal/wasm2go/`) so it never enters a consumer's graph; because the translation is committed, a dead wasm2go costs regeneration, not the binding.
   - The stack: 64 KiB, measured as 256x the smallest size that passes the whole suite (256 bytes; 128 traps), so an instance is small enough to pool.

   The harness, the raw results and how to run them on another machine: `docs/superpowers/spikes/2026-10-30-go-cgo-free/`.
7. [x] **Go: run the whole test suite under TinyGo in CI.** Done 2026-10-31 with item 6: the Forge's wasm job compiles the suite with `tinygo test -target=wasip1 -c` and runs it under wasmtime with the checkout mounted and the corpus and crate version passed through `HYPERUUID_CORPUS` and `HYPERUUID_CRATE_VERSION` (a negative control without the mount fails on the corpus), runs the exhaustion package the same way, and runs the suite under stock Go on `js/wasm` (Node) and `wasip1/wasm` (wasmtime) too; the 1 GiB batch test skips on 32-bit and wasm hosts. Before it, CI ran only a smoke program (`go/internal/tinygosmoke`), so the suite did not compile there. Found 2026-10-30, TinyGo 0.44.0, `-target=wasip1` under wasmtime, in a scratch copy:
   - `go/inspect_test.go:88` fails to compile: `7 + 1<<32` overflows TinyGo's 32-bit `int` on wasm. That case only exists where `int` is 64 bits, so add it only when `strconv.IntSize == 64`. A runtime conversion compiles but truncates to 7, and the test then fails on a value that can't occur.
   - The corpus tests and `TestNativeVersionMatchesTheCrate` find `corpus/` and `rust/Cargo.toml` by walking up from `os.Getwd()`, which is always `/` under TinyGo on wasip1 (it ignores `PWD`). They need a root they can be told (an environment variable), and the runner has to mount the checkout (`wasmtime run --dir=<repo>::/repo`). The default `tinygo test` emulator mounts only a temp directory, so CI builds with `tinygo test -c` and runs wasmtime itself. Negative control: without the mount, the corpus tests fail with "not found".
   - `TestAV7BatchOfExactlyTheCounterSpaceIsStrictlyIncreasing` (1 GiB) runs out of TinyGo's wasm heap; run with `-test.short`, which already skips it.
   - With the mount and `-short`, 64 tests pass. The only failure was the truncated `1<<32` case, which the `IntSize` check removes. Still to try: the browser target (`-target=wasm` under node).

## Deferred

**BEAM (Erlang, Elixir, Gleam): wait until someone asks.** Those communities already use tagged-tuple and `Result` errors, so they need the least convincing. If demand appears, HyperCast is the better first binding: a Rustler wrapper over the C ABI, with dirty schedulers for large batches.
