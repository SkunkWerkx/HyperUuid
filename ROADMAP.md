# HyperUuid Roadmap

Oct 27, 2026 · @Buvy

## Where it stands

HyperUuid now ships nine bindings over one Rust core, with TypeScript and R landed as of master commit `12fe74d` (Oct 26, 2026). Fork safety is already solved; the counter, runtime floors and in-git binaries are the open items.

| Binding | Mechanism | Distribution | Floor | Single-call result |
| --- | --- | --- | --- | --- |
| C# | LibraryImport over the C ABI | NuGet | .NET 11 only | 7.5-7.9x faster than `Guid.NewGuid()` |
| Java | FFM | Maven Central | JDK 25 | 2.8-5.1x faster than `UUID.randomUUID()` |
| Python | PyO3 extension | PyPI, wheels only (no sdist) | 3.11 | 3.4-4.2x faster than stdlib |
| Ruby | Magnus, Fiddle fallback | RubyGems | 3.3 (on Fiddle) | 2.1-5.5x faster than `SecureRandom.uuid` |
| PHP | ext-ffi | Packagist, binaries in git | 8.2 | 1.2-1.4x faster than a naive inline v4 |
| Swift | static library | git tag, binaries in git | 6.2 | 11-16x faster than `Foundation.UUID()` |
| Go | cgo | git tag, binaries in git | 1.26 | slower than `google/uuid` (control group) |
| TypeScript (new) | napi-rs addon | npm: root package + 8 platform packages | Node 22, Bun | 2.0-20x slower than `crypto.randomUUID()` / `uuid` `v7()` |
| R (new) | `.Call` into a vendored static library | CRAN source package | R 4.2 | v4 7.1-8.8x faster than `uuid`; time-based 1.6x slower |

What the TypeScript and R landing settled:

- **TypeScript** published its loss up front, as planned. Batches recover it: `newV7Batch(1000)` is 11x (Node) to 16x (Bun) faster than 1000 single calls. There is no wasm fallback yet, so `--omit=optional` or an unlisted platform gets no addon.
- **R** went straight to CRAN as a source package with vendored crates, rather than r-universe first. It exposes raw-matrix doors (`uuid_v4_raw`, `uuid_v7_raw`, `uuid_to_raw`) and the vectorised SQL-order conversions.
- **Fork safety is done.** The Linux entropy pool lives in a mapping marked `MADV_WIPEONFORK`, refuses to run if that flag is not honoured, and `rust/tests/entropy_pool.rs` proves a forked child draws different bytes. No key outlives one second, which also covers snapshot restores.
- **The v7 counter is unchanged.** It is still seeded once per process and never resets.
- **The About text could not be re-checked from here.** The public page last showed "v4/v5/v6/v7" and seven languages; confirm it now names nine and drops v6.

## Standing policies

Four rules decide every item below.

1. **Supported runtimes only.** A binding's floor follows upstream support; a version past end of life is never a floor.
2. **Production-reality review before release.** Ask how teams actually deploy the language and where the install breaks: a practitioner, or an adversarial Forseti pass.
3. **Losses print next to wins,** as Go, TypeScript and R already do.
4. **Binaries ship as attested release assets, not git commits,** except where the ecosystem's tooling leaves no other path.

## Phase 0: Correctness and credibility

The counter re-seed is the one change that touches persisted behaviour; the rest is wording.

- [ ] **Re-seed the v7 counter on each new millisecond.** Re-initialise it randomly when the timestamp advances (RFC 9562 §6.2 Method 1; check §6.9). Today two IDs reveal how many were minted between them. Rework the batch roll-forward and the wrap rules in "v7 ordering, precisely", and update the corpus.
- [ ] **Fix the About text:** drop v6, name all nine languages.
- [ ] **Reposition the README.** Lead with SQL Server ordering, v7 ordering guarantees and byte-identical v5. Speed supports; it does not lead. Cut to about a third and push depth into per-binding READMEs.
- [ ] **Go README:** put the cgo requirement and the `CGO_ENABLED=0` compile error in the opening lines.
- [x] **Fork safety:** done (`MADV_WIPEONFORK` pool plus a fork test). Add one README sentence so reviewers find it.

## Phase 1: Supported-runtime floors

C# and Java are the floors that lock out the financial and insurance buyers; the rest are small.

| Binding | Change | Why |
| --- | --- | --- |
| C# | Target `net10.0` and `net11.0` | .NET 8 ends support Nov 10, 2026; 10 is the LTS conservative shops run |
| Java | JNI path for JDK 21 in a multi-release JAR's base, FFM under `META-INF/versions/25`; floor 21, no JDK 17 | FFM is only a preview in 21 (final in 22), so 21 needs JNI |
| Ruby | Add 3.3 to the Magnus fat-gem grid until its end of life | The supported floor currently lands on slow Fiddle |
| Python | Publish an sdist via maturin | FreeBSD, ppc64le and s390x (Linux on IBM Z at banks) can build from source |
| PHP | Floor moves to 8.3 | 8.2 reaches end of life Dec 31, 2026 |
| Rust | `uuid` crate `From`/`Into` behind a feature flag | Most Rust code already holds `uuid::Uuid` |

## Phase 2: Binaries out of git

The end state is a main repo of source only, with every binary an attested release asset.

**PHP: move to an ext-php-rs extension installed by PIE.**

- Composer does not install extensions; PIE, the PHP Foundation's installer, does. It resolves the package on Packagist and downloads the matching prebuilt zip from GitHub release assets on Linux and macOS.
- Set `"type": "php-ext"` in the root `composer.json`, and name assets `php_<ext>-<version>_php<X.Y>-<arch>-<os>-<libc>.zip`.
- Build once per supported minor (the ABI changes per minor, not per patch): 8.3, 8.4, 8.5, NTS only, plus musl for Alpine. About 21 zips per release.
- Add ZTS when someone asks (FrankenPHP).
- Retire the FFI binding, or move it to its own repo. It is the only reason PHP still needs binaries in git, and production PHP-FPM often cannot use FFI.

**Swift: remote `binaryTarget`.** A URL plus checksum on GitHub Releases. Verify first that remote artifact bundles carry static libraries on Swift 6.2.

**Go: separate repo.** Move the binding to `SkunkWerkx/hyperuuid-go`, optionally behind a vanity import path. Its committed archives leave the main history, and it releases on its own cadence.

## Phase 3: New bindings and follow-ups

C is next because it is nearly free and opens the core to every language that speaks the C ABI.

1. **C.** Ship a cbindgen header, pkg-config and CMake config files, and the prebuilt libraries already built. Formalise the C ABI as a versioned, stable contract, since nine bindings now depend on it.
2. **TypeScript v1.2: wasm fallback.** A napi-rs WASI build that the root package falls back to automatically, so `--omit=optional` and unlisted platforms still work and browsers get a path. Before it ships, test Docker images that strip optional dependencies, lockfiles made on a Mac and installed on Linux, and serverless bundlers that miss `.node` files.
3. **Entropy on macOS.** TypeScript's single-call loss is mostly one `getentropy` syscall per ID; the ChaCha20 pool runs on Linux only. Measure the pool on macOS before extending it there.
4. **R: hold the API where it is.** CRAN is done; keep HyperUuid's R surface minimal and spend R effort on HyperCast and HyperTabular, the real targets for the actuary.
5. **C++.** A thin header over the C header using `std::expected` (C++23). Add vcpkg and Conan ports when someone asks.

## Deferred

**BEAM (Erlang, Elixir, Gleam): wait until someone asks.** Those communities already use tagged-tuple and `Result` errors, so they need the least convincing. If demand appears, HyperCast is the better first binding: a Rustler wrapper over the C ABI, with dirty schedulers for large batches.
