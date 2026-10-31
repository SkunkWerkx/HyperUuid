# The PR board: core first, host × binding, only what changed

Written 2026-10-31 from the owner's CI conversation of the same day; revised the same day
after a review (eight blockers, five coverage findings, five evidence corrections and eight
hazards, each verified against the workflows; see Revision log). **Draft for owner review.**
Spans this repo's `ci.yml` and the Forge (`SkunkWerkx/.github`), which gains new workflows
beside the ones it has. HyperCast and HyperTabular take the consumer side when they thaw (the
freeze stands: memory `prove-in-hyperuuid-then-cascade`). The release train (preparation
that packs and tags on green, isolated and idempotent publishers, the GitHub Release as a
publisher, where build provenance is made) is **spec two**, written after this one has run
on real pull requests, because it stands on the workflows this spec creates and wants this
spec's measured numbers. `docs/superpowers/plans/` will hold the implementation plan.

## Goal

The pull-request board is the run operators feel on every commit, so it is the one to
optimize. It proves what the pull request could have broken, on the hosts where that could
break differently, and nothing else: the Rust core on all seven hosts always, each binding
only when it or the core changed, each lint once, and nothing the release board exists for.
Compute is spent as if it were paid for, because being open source makes it free to us, not
free.

## Decisions (owner, 2026-10-31)

- **The PR cycle is the priority.** "It happens every commit after open so it's the one
  operators (you and I currently) feel the most." The release train "is expected to be
  brutal because we want to build *everything*", and is spec two.
- **Core first, then host × binding.** "We need to fork off the rust core in a matrix then
  chain the binding work after." The fan-out is "runner (os & arch) × binding", not per
  check: "we do go berserk fanning out linux jobs, I definitely would like to reel them in."
- **A binding runs only when it changed.** "If we can only conditionally invoke them when
  the binding changes then we don't pay the runner time of setting up PHP/Go/C# etc more than
  once per os and architecture host."
- **C and C++ ride with the core.** They consume the core's own outputs, so their smokes are
  steps of the core legs; "that knocks us down to 10 languages".
- **Lint once, and fail fast.** A lint result is host-independent, so each binding lints
  once, on linux-arm64 ("it finishes first almost every time"). "There is no need to pay the
  CI minutes if the lint check was going to fail": a red lint starts no suite anywhere.
- **No new Docker, no self-hosted runner.** Docker covers only Linux, which is not the slow
  leg, and the queue is per job, not per setup; the musl legs' Alpine containers, the C
  smoke's Alpine run and the npm harness's QEMU stay as they are. A self-hosted runner on a
  public repository would run a fork's pull request on the owner's machine; refused on
  security grounds before upkeep is weighed.
- **Two specs, in sequence.** The release train is a second spec, implemented after this
  one, because the Forge is called at master and two restructurings of one workflow in
  flight is the one state where no consumer can release.
- **Runner labels are explicit.** The macOS legs run on `xcode-27`, named, not `macos-latest`
  ("I want to be explicit and name the tag as xcode-27"), and Windows on
  `windows-2025-vs2026` and `windows-11-vs2026-arm`; the same rule for every label.
- **Take the image's toolchain where it is the pinned version.** A toolchain the runner
  image ships at the version we pin is asserted, not installed; only what the image lacks is
  set up.

## Evidence (measured 2026-10-31)

The last green PR board (run 41230480668, branch `typescript_wasm`):

| | |
|---|---|
| Wall clock | 24 min |
| Jobs | 13 top-level, expanding to 84 (79 succeeded, 5 skipped) |
| Rust core compile, per native leg | 6 to 17 s |
| Slowest native leg (win-x64, win-arm64, linux-musl-x64) | 13 min |
| Second-longest job: the Python sdist fall-through on PyPy | 12 min |
| Longest check-core job: lint-go | 4 min |
| macOS jobs on the board | 9 |
| Windows jobs on the board | 13 |

Inside the osx-arm64 leg every step over 20 s is a binding toolchain or suite (Swift setup
54 s, R setup 80 s, Python setup 35 s, Ruby's Magnus suite 35 s, Java's JNI suites 52 s). The
core builds in 6 s. The leg's minutes are the bindings, run one after another in one job, and
`test-c`, which needs only the core, waits for all of them.

Queue wait (job created to job started) across the last six PR boards:

| Runner label | jobs | mean | max | over 2 min |
|---|---|---|---|---|
| macos-latest | 38 | 24 s | 103 s | 0 |
| windows-latest | 42 | 59 s | 281 s | 9 |
| windows-11-arm | 24 | 19 s | 53 s | 0 |
| ubuntu-26.04 | 292 | 82 s | 315 s | 83 |
| ubuntu-26.04-arm | 60 | 46 s | 208 s | 9 |

The ten longest waits, 295 to 315 s each, are all `check-core`'s lint and format jobs on
ubuntu-26.04: under a minute of work each, queued behind the fat jobs. That is the free
tier's cap of 20 concurrent jobs (5 on macOS), not runner capacity. A board of 84 jobs floods
it. The owner's heavy fan-out of the early days was folded back for this reason; the
mechanism is confirmed, and where it bites has moved from macOS to Linux x64.

The last full board (scheduled, 2026-10-26, before Go's translated core, TypeScript, R and
the PHP extension): 28 min, long pole `test-apple-mobile` at 18 min, the two ruby.wasm legs
at 10 and 11 min. Two iOS simulators boot on a full board today: one in the Forge's
`hyper-test-c.yml` (also on PR boards) and one in this repo's `test-apple-mobile`.

## Design

### The board, in dependency order

1. **`changes`.** Always runs, seconds. Classifies the pull request's files (rules below)
   and emits one boolean output per class: `core`, `ci`, `docs_only`, and one per binding.
   Every `if:` below reads these outputs. A docs-only pull request runs this and the
   aggregate (item 12) and nothing else.
2. **`core`.** One call of the Forge's new core workflow, whose matrix is the seven host
   legs, unless `docs_only`: build the cdylib, run the core's suite and the corpus, check the
   cdylib's exports (today's `check-cdylib`, per RID, moves here), build this leg's own static
   library for its smokes, assemble this RID's C archive with `c_package.py assemble` and run
   the C and C++ smokes against it, upload `native-<rid>`. About two minutes each. They never
   skip on a code change, since a binding change can touch nothing here and still needs a
   core to test against.

   One call is one caller job, so every consumer of `core` waits for all seven legs. Today
   that barrier would be 13 minutes; after this split it is the spread between core-only
   legs, expected about a minute (the Windows and musl legs, with their setup and container
   start). The plan measures it. If a leg stays well above the others, the escape hatch is
   one caller per host in `ci.yml` with each binding's test callers per host too, which
   GitHub needs for per-host chaining since `needs` takes no expression; that is about
   seventy caller jobs and is taken only if the measurement says so.
3. **`build-static`** and **`build-android`.** As today, when `core` or `ci`: the one job
   that builds the cross-target bundle (Apple, Android, the archives the NuGet targets link)
   and uploads `static-libs` and `swift-release-assets`, and the two Android legs that upload
   `native-android-*`. `static-libs` has one producer; it cannot come from seven legs.
4. **`core-checks`.** When `core` or `ci`: today's `hyper-check-core.yml` minus the binding
   lints and the per-RID cdylib checks, re-cut as a small matrix with `fail-fast: false`
   over `ubuntu-26.04`, `ubuntu-26.04-arm` and `windows-2025-vs2026`, so the arm64 core check
   and the Windows no-panic check keep their hosts and a red check stops no other. Each
   matrix job runs its host's checks in the order that fails soonest: rustfmt, clippy (with
   the Android target), exports, corpus, semver, MSRV, no_std, no_panic, big-endian, WASI,
   the workflow lint and shape tests, and the C and C++ formatter and shellcheck from
   `hyper-test-c.yml` (its lines 28 to 34), which otherwise lose their home. A sibling of the
   legs, not a step of one: every binding job waits on the legs. The plan measures the
   longest matrix job; today's check-core jobs sum to about ten minutes, so the cut across
   three hosts has to keep each under the core legs' time or split once more.
5. **`lint-<binding>`.** One caller per touched binding, no `needs`, calling that binding's
   lint workflow on `ubuntu-26.04-arm`: the formatter and linter, and whatever else today's
   lint job carries (R's also runs `cargo fmt --check` on `r/src/rust` and clang-format on
   `r/src/*.c` and `*.h`; those move with it). It needs no core, so it runs beside the legs.
   A lint that is slow or has no linux-arm64 build moves, with the reason commented at the
   pin: today lint-go is 4 min, longer than a core leg, so Go's gate is gofmt and go vet, and
   golangci-lint runs beside Go's suites rather than ahead of them, unless its cache brings
   it under the legs' time (measured in the plan).
6. **`test-<binding>`.** One caller per touched binding, needing `lint-<binding>` and `core`
   (Swift's Linux legs and the NuGet consumer test also need `build-static`), calling that
   binding's test workflow, whose matrix is the hosts it ships on. One toolchain setup,
   download `native-<rid>`, build what the binding builds itself (Python's PyO3 extension,
   Ruby's Magnus extension, the PHP extension, TypeScript's addon, Java's JNI library), run
   the suite, upload what the release side consumes (`jni-<rid>`, `node-<rid>`,
   `ruby-native-<rid>-<abi>`, `php-ext-<rid>-<minor>`, `aot-report-<rid>`). Fail fast is
   free: on a green push the suites start when the slower parent finishes, which is `core`
   either way; on a red lint, no suite starts anywhere.

   Go ships on every host and has host-tagged tests (`receipts_test.go` on Linux and macOS,
   `random_aes_test.go` per architecture, `regen_test.go` off wasm), so its matrix is the
   seven hosts like any binding; it downloads no core, so its callers need only its lint.
   Its trigger is `go/**` or `core`, since a core change without a regenerated translation
   must turn `TestCoreIsCurrent` red.
7. **`pack-c`.** When `core` or `ci`, needing `core` and `build-static`: `c_package.py pack`
   over every RID and the layout check, uploading `c-packages-ci`, as `hyper-test-c.yml`'s
   `pack` job does today. The per-RID smokes moved into the legs; this is the whole-set
   layout proof, kept on the PR board because it is the only one before release.
8. **`build-wasm`.** As today, with its per-binding inputs fed from `changes`: the archive and
   its Blazor check when `core`, `ci` or `csharp`; the Rust browser test when `core`; the
   WASI module (`node-wasm32-wasip1`) when `core` or `typescript`; the Go wasm and TinyGo legs
   when `core` or `go`. The ruby.wasm legs stay release-only.
9. **`build-wheels`.** The sdist build and the Pyodide wheel when `core`, `ci` or `python`,
   as today in PR mode. The sdist fall-through (12 min) and the per-RID wheels are release
   board.
10. **`test-nuget-package`** when `csharp`, `core` or `ci`, needing `core` and
    `build-static`; **`test-npm-package`** when `typescript`, `core` or `ci`, needing `core`
    and `build-wasm`. Both as today, the consumer tests of a packed package.
11. **`test-apple-mobile`** and **`test-android`**: release board only, as today. The C
    smoke's simulator boot in `hyper-test-c.yml` moves into `test-apple-mobile`, so a full
    board boots one simulator and every binding that reaches iOS runs in it.
12. **`pr-board`.** `if: always()`, needing every job above: fails if any needed job failed
    or was cancelled, or was skipped while its `changes` output said it should run. This is
    the one status to read. Master is not branch-protected and a skipped job satisfies a
    required check, so without this job a too-narrow classifier would be invisible.

### Job disposition

Every job on today's boards, and where it goes. "PR" is gated as the column says; "release"
means dispatch and schedule only.

| Today | Where | Tomorrow | Gate on PR |
|---|---|---|---|
| `build-native` legs: Rust steps | Forge native workflow | `core` legs | not `docs_only` |
| `build-native` legs: binding steps | same | `test-<binding>` legs | binding, `core` or `ci` |
| `build-native-osx-x64` | same | `test-<binding>` and `core` in release mode | release |
| `build-native-musl` | same, Alpine containers | `core` and `test-<binding>` musl legs, containers kept | as above |
| `build-php-extension` | Forge PHP workflow | `test-php` legs | `php`, `core` or `ci`: linux-x64 on 8.5 (the image's) and win-x64 on 8.3 (the other build path, the toolset install); release keeps six RIDs by three minors |
| `build-static` | ci.yml | unchanged | `core` or `ci` |
| `build-android` | ci.yml | unchanged | `core` or `ci` |
| `test-swift-static` (2 jobs) | ci.yml | Swift's Linux test legs | `swift`, `core` or `ci` |
| `test-c`: format and shellcheck | Forge test-c | `core-checks` | `core` or `ci` |
| `test-c`: `pack` and `c-packages-ci` | Forge test-c | `pack-c` | `core` or `ci` |
| `test-c`: `smoke` ×5 and the Alpine smoke | Forge test-c | steps of the `core` legs (the musl legs are the Alpine run) | with `core` |
| `test-c`: `smoke-apple-mobile` | Forge test-c | `test-apple-mobile` | release |
| `check-core`: core checks (incl. Windows no-panic, arm64 core) | Forge check-core | `core-checks` matrix | `core` or `ci` |
| `check-core`: `check-cdylib` ×7 | same | steps of the `core` legs | with `core` |
| `check-core`: `lint-*` and `format-*` per binding | same | `lint-<binding>` | binding, `core` or `ci` |
| `check-core`: `lint-rust`, `lint-workflows` | same | `core-checks` | `core` or `ci` |
| `build-wasm`: archive, Blazor, Rust browser, WASI, Go wasm | Forge wasm | unchanged, inputs from `changes` | per item 8 |
| `build-wasm`: ruby.wasm ×2 | same | unchanged | release |
| `build-wheels`: sdist, Pyodide | Forge wheels | unchanged | `python`, `core` or `ci` |
| `build-wheels`: per-RID wheels, sdist fall-through | same | unchanged | release |
| `test-nuget-package` | ci.yml | unchanged | `csharp`, `core` or `ci` |
| `test-npm-package` ×3 | ci.yml | unchanged | `typescript`, `core` or `ci` |
| `test-apple-mobile`, `test-android` | ci.yml | unchanged, plus the C simulator smoke | release |
| `request-staging` | ci.yml | `needs` rewritten to the new job names | release |

Nothing is removed from the release board; items move between jobs. The expected PR board
for a one-binding change: `changes`, `core`, `lint-<binding>`, `test-<binding>` (its hosts),
and `pr-board`, plus the binding's own extras (`build-static` and the NuGet test for C#,
`build-wasm` and the npm tests for TypeScript, the wheels pair for Python, Swift's
`build-static`).

### Change detection

`changes` reads the pull request's file list from the API (`pulls/<n>/files`, paginated),
which carries `filename`, `previous_filename` and `status`, so a rename classifies both
sides and no checkout depth is involved. Classes, by path; a file belongs to the first class
that matches, and a rename belongs to every class either of its paths matches:

| Class | Paths |
|---|---|
| ci | `.github/**` |
| docs | `*.md`, `**/*.md`, `docs/**`, `LICENSE`, `CRAN_CHECKLIST.md`, `ROADMAP.md` |
| dotfiles | `.editorconfig`, `.gitattributes`, `.gitignore`, `.claude/**`: same as docs |
| core | `rust/**`, `include/**`, `c/**`, `corpus/**` |
| csharp | `csharp/**`, `global.json` |
| java | `java/**` |
| python | `python/**` |
| ruby | `ruby/**` |
| php | `php/**`, `composer.json` (the extension itself lives in `rust/`, so is `core`) |
| swift | `swift/**`, `Package.swift` |
| typescript | `typescript/**` |
| r | `r/**` |
| go | `go/**` |
| everything | any path not listed |

`ci` is first on purpose: `.github/RELEASING.md` is a `.github/` file before it is a
Markdown file, and a workflow edit can reach any job, so `ci` implies everything. `core`
implies every binding (each suite runs the corpus against the core). `everything` is the
fail-safe for an unclassified path: the board runs in full rather than guessing. The matcher
is gitignore-style (`pathspec`), not `fnmatch`, so `**/*.md` matches a root file and `*.md`
is listed anyway. Forge changes are not visible in this repo's diff; they are proven by the
Forge's own validation and the next consumer run, as today.

On `workflow_dispatch` and `schedule` there is no gating: every output is true.

### Where it lives

The Forge gains new workflows beside its existing ones; nothing existing changes until the
siblings have moved:

- `hyper-core.yml`: the seven core legs (matrix inside), taking `mode`, the crate and project
  names and the C smoke inputs, the musl legs in their Alpine containers, osx-x64 off in PR
  mode. Every attestation step keeps its `mode != 'pr'` guard.
- `hyper-lint-<binding>.yml`, one per binding, one job on `ubuntu-26.04-arm`.
- `hyper-test-<binding>.yml`, one per binding, a matrix over the binding's hosts, taking the
  inputs the native workflow takes for it today (`csharp_test_project`, `jdk_floor_version`,
  `ruby_compat_version`, `php_versions`, …) and `mode`. The Windows `ffi` and `swift-ruby`
  shards disappear: Swift and Ruby on Windows are their own workflows' Windows legs. Each
  artifact name has exactly one producing job (the table below); a matrix job uploads only
  names that carry its RID.
- `hyper-core-checks.yml`: the three-host matrix above.
- `hyper-build-native.yml`, `hyper-check-core.yml` and `hyper-test-c.yml` stay as they are,
  with their inputs, for HyperCast and HyperTabular, which call them at master. They go when
  the siblings have moved, in a Forge change of their own.

This repo's `ci.yml` becomes `changes`, `core`, `build-static`, `build-android`,
`core-checks`, `pack-c`, the lint and test callers, `build-wasm`, `build-wheels`, the two
consumer tests, the two mobile jobs, `request-staging` with its `needs` rewritten, and
`pr-board`. Release mode keeps every binding on, so `stage-native-binaries.yml`,
`prepare-release.yml`, `release.yml` and the Forge's packers and publishers work unchanged.

### Release contract

The artifact names the release side downloads, each with its one producer after the split.
The split moves producers between jobs; it changes no name and no content.

| Artifact | Producer tomorrow | Consumers |
|---|---|---|
| `native-<rid>` (seven RIDs, osx-x64 in release mode) | `core` legs | stage-natives, pack-nuget, pack-c, pack-gems, prepare-maven, release-guard, test-nuget-package |
| `native-android-<arch>` | `build-android` | ci.yml's `test-android`; the `native-*` patterns above |
| `static-libs` | `build-static` | stage-natives, pack-nuget, pack-c, release-guard, Swift's Linux legs, test-nuget-package, `pack-c` |
| `swift-release-assets` | `build-static` | stage-natives, release-gate, release-guard |
| `jni-<rid>` | `test-java` legs | prepare-maven |
| `node-<rid>` | `test-typescript` legs | pack-npm, test-npm-package |
| `node-wasm32-wasip1` | `build-wasm` | pack-npm, test-npm-package |
| `ruby-native-<rid>-<abi>` | `test-ruby` legs | pack-gems |
| `ruby-wasm-<abi>` | `build-wasm` (release) | pack-gems |
| `php-ext-<rid>-<minor>` | `test-php` legs | release-gate |
| `pypi-wheels-<rid>`, `-sdist`, `-pyodide` | `build-wheels` | verify-wheels |
| the wasm archive (`artifact_name`) | `build-wasm` | pack-nuget, test-blazor |
| `aot-report-<rid>` | `test-csharp` legs | the native workflow's own summary today; kept |
| `c-packages-ci` | `pack-c` | the C smokes today; after the split, the layout check's own output |

### Runner labels and the image's toolchains

Every `runs-on` names its image: `ubuntu-26.04`, `ubuntu-26.04-arm`, `xcode-27`,
`windows-2025-vs2026`, `windows-11-vs2026-arm`. No `-latest`: an image move is a deliberate
edit, reviewed like a floor, and the two macOS images in use today (`test-apple-mobile` on
`xcode-27`, the osx-arm64 leg on `macos-latest`, which is macOS 26 with Xcode 26) become one.
The Forge's eleven `macos-latest` and six `windows-latest` move with it, in the new workflows
first and in the kept ones when the siblings move.

What the labels resolve to, from the runner-images README on 2026-10-31:

- `windows-latest`, `windows-2025` and `windows-2025-vs2026` are three labels for one image
  today, Windows Server 2025 with Visual Studio 2026. The win-x64 legs use
  `windows-2025-vs2026` regardless: the label states which toolchain the leg builds with, so a
  reader sees Visual Studio 2026 without opening a readme, and GitHub repointing the shorter
  labels at a later image cannot move the leg in silence.
- `windows-11-arm` (Visual Studio 2022) and `windows-11-vs2026-arm` (Visual Studio 2026) are
  two images. Moving win-arm64 is a real image change, and a good one: the VS 2026 image has
  Rust 1.99.1, Python 3.14.9 and 3.15.1 cached, Go 1.26.10 and .NET 10.0.403, where the VS
  2022 image lags on all four.
- `xcode-27` is listed as a preview image; `macos-latest` is `macos-26`. The owner takes the
  preview by name. If GitHub retires or renames it, the fallback is `macos-26` with Xcode 26,
  and the Swift assertion below is what reports that change.

The five images, from their readmes on 2026-10-31 (image versions 20261026.0314.1,
20261026.180.1, 20261026.168.1, 20261021.278.1, 20261026.197.1), against what the legs
install today and what each setup step measured on the last green PR board:

| Toolchain | Pinned | xcode-27 | ubuntu-26.04 (x64, arm64) | windows-2025-vs2026 | windows-11-vs2026-arm |
|---|---|---|---|---|---|
| Rust | stable | 1.99.1 | 1.99.1 | 1.99.1 | 1.99.1 |
| Swift | 6.4 | Xcode 27.1 default, version not stated | 6.4.1 | absent | absent |
| Python | 3.14 | 3.14.9 cached | 3.14.9 cached | 3.14.9 cached | 3.14.9 cached |
| Python, free-threaded | 3.15t | absent | absent | absent | absent |
| Node | 26 | 22, 24 | 22, 24 | 22, 24 | 22, 24 |
| Go | 1.26 from `go.mod` | 1.26.10 | 1.26.10 | 1.26.10 | 1.26.10 |
| .NET | 11 rc.3 from `global.json`, plus 10 | 10.0.403 | 10.0.403 | 10.0.402 | 10.0.403 |
| Java | 25, floor 21 | 21 default, 25 | 25 default, 21 | 17 default, 21, 25 | 21 default, 23 |
| Ruby | 4.0 | 4.0.9 | 4.0.9 | 4.0.7 | 3.4.12 only |
| R | release, oldrel | absent | absent | 4.6.1 | 4.6.1, emulated x64 |
| PHP | 8.5 on the PR board | absent | 8.5.4 | 8.5.11 | 8.4.27 |
| Bun | latest | absent | absent | absent | absent |
| MSVC toolsets | 14.29 for PHP 8.3, 14.44 for 8.4 and 8.5 | n/a | n/a | 14.29 (v142 group), 14.44, and VS 2026's own | 14.29 and 14.44 ARM64, and VS 2026's own |
| Android NDK | the image's | n/a | x64 has 27, 29, 30; arm64 none | n/a | n/a |

The PHP extension's Windows legs are unaffected by Visual Studio 2026: each links with the
toolset its minor's php.net build used, selected by `vcvarsall -vcvars_ver`, and the step
installs the component when the image lacks it (the Forge's `hyper-build-php-extension.yml`,
with the table in `php_ext_package.py`). Both VS 2026 images list the 14.29 and 14.44
components, so that install path should go quiet. The one thing the arm64 move has to prove
first is Swift on Windows arm64 under Visual Studio 2026, since setup-swift's toolchain
expects Visual Studio's build tools and swift.org names 2022; the plan's first run is that
leg, and if it fails the arm64 Swift job alone stays on `windows-11-arm` with the reason
commented at the pin.

Measured setup cost today, per leg, where it is more than a few seconds: setup-swift 54 s on
macOS and 27 to 31 s on Linux; the free-threaded Python download about 30 s on macOS, 53 s on
win-x64 and 102 s on win-arm64; R and its dependencies 70 to 80 s on every host, with the
floor R a second 25 to 30 s; the Windows Swift toolchain restore 58 to 69 s; setup-dotnet's
.NET 11 download 45 s on win-x64 and 23 s on win-arm64; setup-go 23 s on win-x64 and 48 s on
win-arm64 for a module with two dependencies and a four-line `go.sum`.

What the PR board stops paying for:

- **Swift's action on Linux and macOS.** The images ship 6.4.1 and Xcode 27.1; a step asserts
  `swift --version` against the pin's minor and fails on drift, and setup-swift goes from
  those legs. Windows has no Swift, so the cached toolchain stays there, paid once by Swift's
  own Windows jobs and by no other job.
- **The free-threaded Python suite runs on linux-x64 only in PR mode.** No image carries a
  free-threaded build, and the download is the longest setup step on Windows. Release mode
  keeps it on every host, since the free-threaded wheels ship per platform.
- **The floor R check runs once per board**, on linux-arm64 beside R's lint, not on every
  host. R itself is absent from every image but Windows, so the release-R setup stays; the
  pin is `release`, a floating name, so Windows' image R is not asserted against it.
- **PHP's linux-x64 PR build takes the image's 8.5**; no setup-php there. The win-x64 floor
  build keeps setup-php and the toolset step, since that path is what it is there to prove.
- **setup-go's module cache is tried off.** For two dependencies the restore likely costs
  more than the download; the plan measures both on win-arm64 and keeps the cheaper.
- **Java's toolcache is used as is**: setup-java is a selector when the version is present
  (1 to 3 s) and a download only on win-arm64, where neither VS image has a 25.

What stays, and why: .NET 11 rc.3 is on no image until its release and the next image
refresh, after which `global.json`'s pin matches and setup-dotnet becomes a selector (memory:
the RTM switch); Node 26 and Bun are downloads of a few seconds; the Android NDK is already
taken from the x64 image, which is why Android builds on x64 only.

An assertion step replaces an action only where the image's version satisfies the pin; a
floor or a version the image lacks is still installed. The images move on their own
schedule, so each assertion names the pin, not the image's version, and a drifted image fails
with the toolchain's version in the message.

### Expected shape, by pull request

| PR touches | jobs | wall clock, expected |
|---|---|---|
| docs only | 2 | seconds |
| one binding | `changes`, 7 core legs, 1 lint, up to 7 test legs, `pr-board`, plus that binding's extras | about 5 min |
| the core, corpus or `.github/` | the whole grid: 7 core legs, 3 core-checks, 10 lints, about 60 test legs, static, Android, wasm, wheels, pack-c, the consumer tests | 12 to 16 min, in waves through the cap |

The core-change sweep still exceeds the cap, and should: it is the pull request that
deserves the whole grid and the rarest kind. Its total runner time rises against today's:
about 60 test legs each pay a checkout, the Forge-revision checkout, an artifact download
and a Rust toolchain, 20 to 30 job-minutes in all, in exchange for a critical path of core
plus one binding instead of core plus every binding. The receipts for these three rows are
measured once the board runs (Proof) and written back into this section.

### Error handling

- A red core leg fails the board; the test callers that need `core` skip rather than run
  against nothing, and the other legs finish so a host-specific failure is seen as such.
- A red lint stops its binding's suites on every host and nothing else.
- A red `core-checks` matrix job fails the board but stops nothing: `fail-fast: false`, and
  the legs and suites it runs beside finish, so one push reports everything it broke.
- A skipped caller reports skipped, never success; `pr-board` turns an unexpected skip into a
  failure, so a wrong `changes` rule is a red board, not a green one.

## What does not change

- The release board's coverage: every host, every binding, every attestation, on dispatch
  and schedule. The disposition table is the check: every row's release-mode home exists.
- `stage-native-binaries.yml`, `prepare-release.yml`, `release.yml` and the Forge's packers
  and publishers: untouched in spec one, kept working by the release contract above.
  `request-staging` in `ci.yml` is touched, for its `needs` list only.
- The Forge's existing workflows and their inputs, for the siblings, until they move.
- The Forge's fail-fast ordering rule for new bindings becomes moot rather than broken: each
  binding fails in its own job, visibly, and the rule is retired from the Forge's `CLAUDE.md`
  when the old native workflow goes.
- Floors, pins, toolchains, the corpus, and every binding's own tests.

## Proof

- The shape tests in `.github/scripts/tests/test_workflow_contracts.py` pin the job graph:
  `changes` and `pr-board` are the only unconditional jobs, every lint caller has no
  `needs`, every test caller needs its lint and `core`, `core-checks` is the three-host
  matrix with `fail-fast: false`, release mode has no gate, `request-staging` needs every
  release-mode job by its new name, every Forge `uses:` is at master, and every artifact
  name in the release contract has one producer. The Forge's own tests pin each new
  workflow's shape.
- The classifier has unit tests: root `README.md` is docs; `.github/RELEASING.md`,
  `.github/workflows/ci.yml` and `.github/scripts/tests/test_workflow_contracts.py` are ci; a
  rename from `rust/` to `docs/` and the reverse are both core; an unclassified path is
  everything; `typescript/README.md` is docs, not typescript.
- One deliberate pull request per row of the expected-shape table, before this merges:
  docs-only, one binding, core. Each receipt is the job count and wall clock from `gh run
  view`, with the `core` barrier (slowest leg minus fastest), the longest `core-checks`
  matrix job and every lint's duration on linux-arm64 read off the same runs; those numbers
  decide the escape hatches above. Negative controls: a one-line change under `typescript/`
  produces no C# job; a one-line change to an unclassified root file produces the full
  board; the lint control, a formatting error in one binding, runs that binding's lint and
  no suite of it on any host.
- The full board, dispatched on the branch once the Forge side is at master, green with
  every artifact name in the release contract present: that is what the stager and packers
  consume.

## Risks and rollout

- **Order.** The shape test requires every Forge call at master, so the new workflows must
  exist there before `ci.yml` can call them. 1. Merge the Forge's new workflows, with the
  old ones untouched. 2. Switch this repo's `ci.yml`, proven on a branch against the Forge's
  master. 3. Remove the old Forge workflows after HyperCast and HyperTabular have moved,
  never before. No release is in flight during 1 or 2 (`.github/RELEASING.md`).
- **The Forge is at master.** Step 1 adds files and changes nothing the siblings call; it
  cannot break them. Step 3 is the one that can, and waits for them.
- **A `changes` rule that is too narrow** skips a test that should have run. The fail-safe
  class, `pr-board` and the negative controls bound it; a miss is fixed by widening the rule.
- **The barrier and the long pole** are predictions until measured; the escape hatches (per-
  host callers, a further split of `core-checks`, a lint moved off the gate) are named above
  and taken only on the numbers.
- **The win-arm64 image move** is proven by its Swift leg first; the fallback is in the image
  section.

## Open questions

None for spec one. Spec two's open decision, carried forward unchanged: whether package
build provenance comes from the preparation run on master with only the release attestation
made at the tag.

## Revision log

- 2026-10-31: first draft, from the conversation of the same day.
- 2026-10-31, review round 1. Verified against the workflows and folded in: `static-libs`
  has one producer (B1); a caller's `needs` gates the whole called workflow, so lints get
  their own workflows (B2); `core-checks` is a three-host matrix, not one job, and carries
  the C and C++ format and shellcheck (B4, C4); `ci` precedes `docs` in the classifier, root
  files and renames are handled and tested (B5, B6); rollout adds new Forge workflows beside
  the old and removes nothing until the siblings move (B7); `request-staging`'s `needs` is
  rewritten (B8); the disposition and release-contract tables (C1, hazards 3 and 8); PHP keeps
  win-x64 on the PR board (C2); Go is matrixed per host (C3); `pack-c` stays on PR (C5);
  lint-r's extra steps move with it (hazard 2); attestation guards survive (hazard 4); the
  classifier reads the PR files API (hazard 5); `pr-board` replaces the "required check"
  wording (hazard 6); Docker scoped (hazard 7); evidence corrected to 84 jobs, 31 check-core
  jobs and 20 to 30 job-minutes of overhead. Pushed back, with the numbers: the `core`
  barrier is real but is the spread between core-only legs once bindings leave them, about a
  minute, and per-host callers are the measured escape hatch rather than the default (B3);
  the longest waits are 295 to 315 s, not 4.7 min; PR-mode PHP is eight jobs today (the
  newest minor on six RIDs plus the floor on linux-x64), not five.
