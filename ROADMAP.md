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

**Go: separate repo.** Move the binding to `SkunkWerkx/hyperuuid-go`, optionally behind a vanity import path. Its committed archives leave the main history, and it releases on its own cadence.

## Phase 3: New bindings and follow-ups

C is next because it is nearly free and opens the core to every language that speaks the C ABI.

1. [x] **C.** Done 2026-10-29, in the tree: the header is generated by cbindgen and checked by a test, and the release attaches one archive per platform (fifteen, iOS and Mac Catalyst included) with the shared and static libraries, CMake and pkg-config files, built against in CI on the eight desktop RIDs and the four Apple mobile ones. The contract is the crate's semver, read at run time through `hyperuuid_version()`: a minor adds exports, only a major changes or removes one. The shared library names itself (`@rpath/` install name, unversioned soname). Ships in 1.1.0.
2. **TypeScript v1.2: wasm fallback.** A napi-rs WASI build that the root package falls back to automatically, so `--omit=optional` and unlisted platforms still work and browsers get a path. Before it ships, test Docker images that strip optional dependencies, lockfiles made on a Mac and installed on Linux, and serverless bundlers that miss `.node` files.
3. [x] **Entropy on macOS.** Done another way: Apple targets call `CCRandomGenerateBytes` once per request instead of `getentropy` per id (a single v7 on the Apple M6 went from 594 to 60 ns, 2026-10-28). The ChaCha20 pool stays Linux and Android only. In the tree; ships in 1.1.0.
4. **R: hold the API where it is.** CRAN is done; keep HyperUuid's R surface minimal and spend R effort on HyperCast and HyperTabular, the real targets for the actuary.
5. [x] **C++.** Done 2026-10-30, in the tree: `include/hyperuuid.hpp`, header-only over the C header, `std::expected` (C++23) for every generating call and `errc` as a `std::error_code` enum, shipped in every C archive and built against in CI beside the C consumer. `check_exports.py` holds it to every export. Add vcpkg and Conan ports when someone asks.

## Deferred

**BEAM (Erlang, Elixir, Gleam): wait until someone asks.** Those communities already use tagged-tuple and `Result` errors, so they need the least convincing. If demand appears, HyperCast is the better first binding: a Rustler wrapper over the C ABI, with dirty schedulers for large batches.
