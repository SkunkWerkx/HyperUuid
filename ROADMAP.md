# HyperUuid Roadmap

Oct 27, 2026 · @Buvy

## Where it stands

HyperUuid now ships nine bindings over one Rust core, with TypeScript and R landed as of master commit `12fe74d` (Oct 26, 2026), and C and C++ reach it from the release's C archives (in the tree, Oct 30). Fork safety and the counter re-seed are solved; runtime floors and in-git binaries are the open items.

| Binding | Mechanism | Distribution | Floor | Single-call result |
| --- | --- | --- | --- | --- |
| C# | LibraryImport over the C ABI | NuGet | .NET 10 (desktop, server, browser); .NET 11 (all platforms) | 7.5-7.9x faster than `Guid.NewGuid()` |
| Java | JNI (21–24), FFM (25+) | Maven Central | JDK 21 | 2.8-5.1x faster than `UUID.randomUUID()` |
| Python | PyO3 extension | PyPI, wheels and an sdist | 3.11 | 3.4-4.2x faster than stdlib |
| Ruby | Magnus, Fiddle fallback | RubyGems | 3.3 (on Fiddle) | 2.1-5.5x faster than `SecureRandom.uuid` |
| PHP | ext-ffi | Packagist, binaries in git | 8.3 | 1.2-1.4x faster than a naive inline v4 |
| Swift | static library | git tag; core zips on the GitHub Release | 6.2 | 11-16x faster than `Foundation.UUID()` |
| Go | cgo | git tag, binaries in git | 1.26 | slower than `google/uuid` (control group) |
| TypeScript (new) | napi-rs addon | npm: root package + 8 platform packages | Node 22, Bun | v4 4.1-6.4x slower than `crypto.randomUUID()`; v7 2.3x faster than `uuid` `v7()` on Node, 1.2x slower on Bun |
| R (new) | `.Call` into a vendored static library | CRAN source package | R 4.2 | v4 12.7-29x faster than `uuid`; time-based 1.2-1.3x slower |
| C, C++ (new) | the C ABI: a generated C header, a C++23 header over it | GitHub Release: one archive per platform (CMake, pkg-config) | C99; C++23 | not measured |

What the TypeScript and R landing settled:

- **TypeScript** published its loss up front, as planned. Batches recover it: `newV7Batch(1000)` is 3.5x (Node) to 6.2x (Bun) faster than 1000 single calls, and since the switch to `CCRandomGenerateBytes` a single `newV7()` on Node beats `uuid`'s `v7()` (Apple M6, 2026-10-30). There is no wasm fallback yet, so `--omit=optional` or an unlisted platform gets no addon.
- **R** went straight to CRAN as a source package with vendored crates, rather than r-universe first. It exposes raw-matrix doors (`uuid_v4_raw`, `uuid_v7_raw`, `uuid_to_raw`) and the vectorised SQL-order conversions.
- **Fork safety is done.** The Linux entropy pool lives in a mapping marked `MADV_WIPEONFORK`, refuses to run if that flag is not honoured, and `rust/tests/entropy_pool.rs` proves a forked child draws different bytes. No key outlives one second, which also covers snapshot restores.
- **The v7 counter re-seeds on every new millisecond** (Unreleased; Phase 0 below). Two IDs no longer reveal how many were minted between them across a millisecond.
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
- [x] **Go README:** put the cgo requirement and the `CGO_ENABLED=0` compile error in the opening lines.
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

**PHP: move to an ext-php-rs extension installed by PIE.**

- Composer does not install extensions; PIE, the PHP Foundation's installer, does. It resolves the package on Packagist and downloads the matching prebuilt zip from GitHub release assets on Linux and macOS.
- Set `"type": "php-ext"` in the root `composer.json`, and name assets `php_<ext>-<version>_php<X.Y>-<arch>-<os>-<libc>.zip`.
- Build once per supported minor (the ABI changes per minor, not per patch): 8.3, 8.4, 8.5, NTS only, plus musl for Alpine. About 21 zips per release.
- Add ZTS when someone asks (FrankenPHP).
- Retire the FFI binding, or move it to its own repo. It is the only reason PHP still needs binaries in git, and production PHP-FPM often cannot use FFI.

[x] **Swift: remote `binaryTarget`.** Done 2026-10-29, in the tree: the root `Package.swift` names the bundle and the XCFramework as two zips on the GitHub Release by URL and checksum, and the binaries left git. A spike proved remote static-library bundles on Swift 6.2.4 and 6.4.1 (glibc, musl, wasm, macOS, iOS, Mac Catalyst); Windows and Android are CI's. Ships in 1.1.0.

**Go: superseded (2026-10-30).** The plan was to keep the archives off master by putting them on release-only commits behind the `go/vX.Y.Z` tags; before that, a separate repo behind a vanity import path. Both are moot: the Go binding moves to the core translated to Go source (Phase 3, item 6), which leaves no Go binaries anywhere.

## Phase 3: New bindings and follow-ups

C is next because it is nearly free and opens the core to every language that speaks the C ABI.

1. [x] **C.** Done 2026-10-29, in the tree: the header is generated by cbindgen and checked by a test, and the release attaches one archive per platform (fifteen, iOS and Mac Catalyst included) with the shared and static libraries, CMake and pkg-config files, built against in CI on the eight desktop RIDs and the four Apple mobile ones. The contract is the crate's semver, read at run time through `hyperuuid_version()`: a minor adds exports, only a major changes or removes one. The shared library names itself (`@rpath/` install name, unversioned soname). Ships in 1.1.0.
2. **TypeScript v1.2: wasm fallback.** A napi-rs WASI build that the root package falls back to automatically, so `--omit=optional` and unlisted platforms still work and browsers get a path. Before it ships, test Docker images that strip optional dependencies, lockfiles made on a Mac and installed on Linux, and serverless bundlers that miss `.node` files.
3. [x] **Entropy on macOS.** Done another way: Apple targets call `CCRandomGenerateBytes` once per request instead of `getentropy` per id (a single v7 on the Apple M6 went from 594 to 60 ns, 2026-10-28). The ChaCha20 pool stays Linux and Android only. In the tree; ships in 1.1.0.
4. **R: hold the API where it is.** CRAN is done; keep HyperUuid's R surface minimal and spend R effort on HyperCast and HyperTabular, the real targets for the actuary.
5. [x] **C++.** Done 2026-10-30, in the tree: `include/hyperuuid.hpp`, header-only over the C header, `std::expected` (C++23) for every generating call and `errc` as a `std::error_code` enum, shipped in every C archive and built against in CI beside the C consumer. `check_exports.py` holds it to every export. Add vcpkg and Conan ports when someone asks.

6. **Go: one backend, the core translated to Go source with wasm2go (decided 2026-10-30; ships in 1.1).** The Go binding needs cgo, which most Go code never has: 11.3% of the 920 most-starred Go repositories import `C`, and 18 of the 24 cgo-decision issues in those repositories were moves away from it ([Chen et al., arXiv 2508.09875](https://arxiv.org/html/2508.09875v1)). The paper measures no call overhead, so every number below is this spike's.

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

   The same harness on x64, 2026-10-30: the Dell XPS 15 9510 (i9-11900H), Go 1.27.3, rustc 1.99.1, Linux amd64 in WSL2 (Fedora 44, kernel 6.18) and Windows 11 natively (MinGW-w64 gcc 16.2 for cgo); `-count 6`, benchstat medians, within ±8% except Windows' `version` cells (up to ±12%), WSL2's cgo `new_v7` (±17%) and the starred cell (±39%). Both cgo controls link the current core: on Windows the archive the Forge's `build-static-libs.sh` trims for Go, built from this checkout, not the committed 1.0.0 one. The wasm and the translated Go are byte-identical on both. wazero was not re-run. Zero allocations in every row.

   | ns/op, Linux amd64 (WSL2) / Windows amd64 | `version` | `new_v4` | `new_v7` | `v7_unix_millis` | batch ×1000 |
   | --- | --- | --- | --- | --- | --- |
   | cgo (today) | 27.9 / 38.3 | 64 / 97 | 69 / 96 | 29.4 / 38.9 | 4,840 / 3,140 |
   | wasm2go, `crypto/rand` | 12.1 / 11.9 | 78 / 127 | 69 / 127 | 14.3 / 13.7 | 16,350 / 7,700 |
   | wasm2go, Go-side ChaCha20 (`x/crypto`) | 10.8 / 11.8 | 82 / 62 | 81 / 62 | 13.1 / 13.8 | 16,090 / 16,140 |
   | wasm2go, Go-side ChaCha8 (`math/rand/v2`) | 10.9 / 12.6 | 61 / 40 | 67 / 42 | 14.0 / 14.0 | 10,710 / 10,720 |
   | wasm2go, the core's ChaCha20 pool in the guest | 10.8 / 12.1 | 118 / 84 | 110 / 74 | 13.5 / 16.0* | 24,790 / 28,630 |

   - **Calls cost nothing to cross.** The translated core is ordinary Go code, so `version` and `v7_unix_millis` run 3–4x faster than cgo on arm64 and 2–3x on x64, where a cgo crossing costs 28 ns (WSL2) to 38 ns (Windows).
   - **Generation is decided by the random source.** On Linux about 45% of a `crypto/rand` v7 is `vgetrandom` and its locking (CPU profile). A Go-side stream keyed from `crypto/rand` and rekeyed after 1 MiB or one second (the native pool's rule) brings Linux arm64 to cgo's level with ChaCha8, or 1.4x behind with ChaCha20. Linux amd64 is at cgo's level with ChaCha8 and 1.2–1.3x behind with ChaCha20. On Windows every Go-side stream beats cgo, whose v4 and v7 cost 96 ns there: ChaCha8 by 2.3x, ChaCha20 by 1.6x. `crypto/rand` per call is 1.3x behind there. `x/crypto/chacha20` has assembly for arm64, loong64, ppc64x and s390x but not amd64, where it runs generic Go; `math/rand/v2`'s ChaCha8 has amd64 assembly. Go programs cannot fork without exec, so a Go-side stream needs no fork guard; the one-second rule covers snapshot restores, as it does natively. The core's own pool inside the guest keeps entropy policy in the core but is scalar code: slowest batch.
   - **The batch is the one loss, and it is larger on x64.** With the Go-side streams it is 1.3x behind cgo on Linux arm64 and 2.7x on macOS. On Linux amd64 it is 2.2x behind with ChaCha8 and 3.3x with ChaCha20; on Windows, 3.4x and 5.1x. There, `crypto/rand` per call has the fastest translated batch (2.5x behind).
   - **Persisted bytes hold.** 194 corpus vectors (v5, SQL order both ways, refusals, timestamps, inspection) are byte-identical through the translated core natively on arm64 (macOS, Linux) and amd64 (Linux, Windows) and, under emulation, on 386, riscv64, ppc64le and big-endian s390x. A corrupted v5 vector and a corrupted SQL-order vector each fail (negative control). `v7_layout.json` needs injected entropy and was not run.
   - **Every target.** The package cross-compiles for linux/{amd64, 386, arm, riscv64, ppc64le, s390x, loong64, mips64}, windows/{amd64, arm64}, freebsd, openbsd, plan9, aix, illumos, android and ios (an iOS executable needs external linking under any backend; that is Go's rule).
   - **The browser, without TinyGo-only code.** Translated Go compiles under stock Go and TinyGo alike. Under node (V8), on the M6 (macOS) and the i9-11900H (Linux amd64 in WSL2: node 24, TinyGo 0.44.0, `-count 3`):

     | ns/op, macOS / Linux amd64 | `new_v4` | `new_v7` | `v7_unix_millis` | batch ×1000 |
     | --- | --- | --- | --- | --- |
     | TinyGo today (the core's archive linked in) | 480 / — | 445 / — | 4.9 / — | 2,100 / — |
     | TinyGo, wasm2go, `crypto/rand` | 99 / 264 | 89 / 254 | 24 / 52 | 18,400 / 42,700 |
     | stock Go `GOOS=js`, wasm2go, ChaCha8 | 88 / 196 | 103 / 247 | 14 / 31 | 11,540 / 26,700 |
     | stock Go `GOOS=js`, wazero (interpreter) | ~6,000 / — | ~7,800 / — | ~920 / — | 1,530,000 / — |

     Single calls are 4–5x faster than today's TinyGo path on macOS; the batch is 5–9x slower. Today's TinyGo path was not measured on x64. Stock Go `GOOS=js` with `crypto/rand` per call costs 1,160 ns a v4 on x64, so a browser build wants a Go-side stream.
   - **Reproducible.** The wasm is byte-identical built on macOS, on Linux arm64 and amd64, and on Windows (rustc 1.99.1), and wasm2go's output is deterministic, so the generated file can be committed and checked the way `include/hyperuuid.h` is: CI regenerates and diffs. wasm2go pins through `go.mod`'s `tool` directive, as `go-sqlite3-wasm` does.
   - **The sandbox is real** (wasm2go's SECURITY.md): code lives outside linear memory, the call stack is Go's and can't be addressed from it, and every access is bounds-checked, so a core bug panics instead of corrupting the Go heap. Its stated threat model assumes a trusted module, which ours is.
   - **Ruled out:** wazero (the table above; its defaults are also a deterministic random source and a fake wall clock, and it interprets on any GOARCH other than amd64 and arm64 and inside wasm), purego (`SyscallN` 107–125 ns on Linux, 1–2 allocs) and goffi (113–124 ns, an allocation per call in `layoutArgs`, contrary to its README). purego and goffi also each ship a `fakecgo`, so they can't link into one binary, and both need a shared library extracted at run time.

   Decided (owner, 2026-10-30): one Go backend, the translated core, committed as generated source with a regenerate-and-diff check. cgo, the thirteen archives, `backend_tinygo.go` and its archive, the core freshness check and the release-only-commit plan (Phase 2) all go. Go binds the same way on every target, and `go get` is the whole install, with or without a C toolchain. The batch loss is accepted for that reach and for one way of binding. Still open:
   - The random source: Go-side ChaCha20 (`x/crypto`, the same construction as the native pool; recommended), Go-side ChaCha8 (fastest, standard library, but 8 rounds is a nitpick waiting to happen), or the core's own pool in the guest (entropy policy stays in the core; slowest). On amd64 `x/crypto/chacha20` has no assembly, so its batch is 1.5x ChaCha8's there (3.3x behind cgo on Linux, 5.1x on Windows), against 1.0–1.15x on arm64.
   - wasm2go is pre-1.0 (one maintainer, two production users of size).
   - Optional: shrink the guest's 1 MiB default stack (`-z stack-size`) to cut each instance's 1.06 MB of linear memory.

   The harness, the raw results and how to run them on another machine: `docs/superpowers/spikes/2026-10-30-go-cgo-free/`.
7. **Go: run the whole test suite under TinyGo in CI.** Today CI runs only a smoke program (`go/internal/tinygosmoke`), so the suite doesn't even compile there, and nothing would catch a TinyGo-only regression. Found 2026-10-30, TinyGo 0.44.0, `-target=wasip1` under wasmtime, in a scratch copy:
   - `go/inspect_test.go:88` fails to compile: `7 + 1<<32` overflows TinyGo's 32-bit `int` on wasm. That case only exists where `int` is 64 bits, so add it only when `strconv.IntSize == 64`. A runtime conversion compiles but truncates to 7, and the test then fails on a value that can't occur.
   - The corpus tests and `TestNativeVersionMatchesTheCrate` find `corpus/` and `rust/Cargo.toml` by walking up from `os.Getwd()`, which is always `/` under TinyGo on wasip1 (it ignores `PWD`). They need a root they can be told (an environment variable), and the runner has to mount the checkout (`wasmtime run --dir=<repo>::/repo`). The default `tinygo test` emulator mounts only a temp directory, so CI builds with `tinygo test -c` and runs wasmtime itself. Negative control: without the mount, the corpus tests fail with "not found".
   - `TestAV7BatchOfExactlyTheCounterSpaceIsStrictlyIncreasing` (1 GiB) runs out of TinyGo's wasm heap; run with `-test.short`, which already skips it.
   - With the mount and `-short`, 64 tests pass. The only failure was the truncated `1<<32` case, which the `IntSize` check removes. Still to try: the browser target (`-target=wasm` under node).

## Deferred

**BEAM (Erlang, Elixir, Gleam): wait until someone asks.** Those communities already use tagged-tuple and `Result` errors, so they need the least convincing. If demand appears, HyperCast is the better first binding: a Rustler wrapper over the C ABI, with dirty schedulers for large batches.
