# TypeScript: the WebAssembly fallback (design, 2026-10-31)

ROADMAP Phase 3, item 2, moved from 1.2 into 1.1.0 by the owner (2026-10-31): the last feature
before the tag, after the PHP extension and the Go translated core. One PR in the Forge, then one
in HyperUuid, CI green before the next step. The owner's conditions from the design conversation
and two review rounds (2026-10-31: eight findings and two corrections, then three more, all
verified against the CLI source, npm issue 4828, the rustc target page and a rustc 1.99.1 build
of a `wasm32-wasip1` cdylib with `-C panic=unwind`) are folded in below and marked where they
shaped a decision.

## Goal

`npm install hyperuuid` gives a working binding on every platform this spec's consumer tests
cover: the eight native platforms, and the WebAssembly fallback proven on Linux x64 (glibc and
Alpine), macOS arm64 and Windows x64 natively, and on s390x under emulation. On the eight
platforms with a native addon nothing changes: the `.node` loads, and the WebAssembly package is
installed but never initialised. Everywhere else, and on any install that lost its native package
(an `--omit=optional` install, a lockfile made on a Mac and installed on Linux, a Docker image that
strips optional dependencies), the root package falls back on its own to the same engine built
for `wasm32-wasip1`, loaded in-process through napi-rs's WASI loader, with the same synchronous
API, the same 194 corpus vectors and the same error codes. Platforms outside that coverage are
described as "the WebAssembly fallback should load; not tested", never as working. Browsers get a
path that is tested in Chromium and claimed for nothing else.

What this is not: a performance feature (the fallback is slower, and the loss is printed), a Bun
or Deno promise (napi documents known gaps there; we state what the suite measures and nothing
more), or an edge-runtime claim (Cloudflare Workers get the entry the CLI emits and no test, so
no claim).

## Decisions (owner, 2026-10-31)

- **A tenth package, a hard dependency of the root.** `@skunkwerkx/hyperuuid-wasm32-wasip1`
  is pinned in the root's `dependencies` at the release version, beside the eight native
  packages in `optionalDependencies`. Every consumer downloads it; that is the price of
  "automatic", and the alternative (opt-in) contradicts the roadmap. The wasm stays its own
  attested tarball, so the artifact model is uniform. Chosen over shipping the wasm inside the
  root for artifact consistency, not aesthetics.
- **The threadless flavour, `wasm32-wasip1`.** The addon is synchronous, with no async work
  and no threadsafe functions, so the threaded flavour's Worker pool and `SharedArrayBuffer`
  buy nothing and cost browsers cross-origin isolation. napi's documentation describes only the
  threaded flavour as "the supported default"; the CLI (3.10.9) manages both, with separate
  loaders, so the first implementation task is a probe build and the suite under
  `NAPI_RS_FORCE_WASI=error`. If the probe fails on something structural, the flavour becomes
  `wasm32-wasip1-threads` (package suffix `wasm32-wasi`) and this spec is amended before
  anything else is built.
- **Native first; WASI initialised only on fallback.** The generated loader enters the WASI
  chain only when the native `require` failed or `NAPI_RS_FORCE_WASI` asks. A consumer test
  asserts that a native install never emits the `node:wasi` warning.
- **The synchronous API survives.** The threadless Node loader instantiates with
  `instantiateNapiModuleSync`; the suite under forced WASI proves every door. When neither
  binding loads, `require('hyperuuid')` still succeeds, `isAvailable()` is `false`, and every
  door throws `ERR_HYPERUUID_UNAVAILABLE` whose `cause` is napi's chain of every native and
  WASI attempt.
- **Runtime dependencies live where they are imported.** The WASI package declares
  `@napi-rs/wasm-runtime`, `@emnapi/core` and `@emnapi/runtime` in its own `dependencies`. The
  root declares only the WASI package. Nothing relies on hoisting.
- **No platform restriction on the WASI package.** The CLI writes no `cpu`, `os` or `libc` for
  wasm32 today; the pack script asserts all three are absent, so a CLI release that starts
  writing `cpu: ["wasm32"]` (the form the docs show) fails the pack rather than a mandatory
  dependency refusing to install on an ordinary CPU.
- **Published tarballs are tested in clean consumers before the tag**, in ci.yml, from a
  local registry, on Linux, macOS and Windows, each after the full suite under forced WASI
  on that OS: `--omit=optional`, the damaged Mac lockfile on
  Linux, an absent native binary, a present loader with a missing `.wasm`, forced WASI, an
  emulated non-x64/arm64 host (s390x under QEMU), the exact Node floor (22.13.0), and the
  negative controls. Bundler asset inclusion is a separate scenario that must end in a usable,
  documented path. `request-staging` needs the job (review finding 8).
- **Panics on the WASI path abort; no containment is claimed there** (review findings 7 and,
  in the second round, 1). The target's default is abort, and the crate's explicit
  `panic = "unwind"` does not reach rustc: cargo emits `-C panic` only for abort, so a profile
  that says unwind passes no flag and the target's default applies. (A raw `rustc --target
  wasm32-wasip1 --crate-type cdylib -C panic=unwind` fails on rustc 1.99.1 with `the crate
  panic_unwind does not have the panic strategy unwind`, but cargo never issues that line.)
  So a release-profile WASI build is silently abort. The crate gains `[profile.wasi]`,
  `inherits = "release"` with `panic = "abort"` and a comment saying why, so the strategy is
  explicit and verifiable instead of a default; the WASI build is `napi build --profile wasi
  --platform --target wasm32-wasip1` (the CLI takes `--profile`), and the release profile is
  untouched. The probe verified the effective invocation on 2026-10-31: the WASI build's rustc
  line, read from a verbose cargo run, carries `-C panic=abort`; the native and release-profile
  lines carry no `-C panic` flag (rustc's default: unwind natively, abort on wasm32-wasip1).
  The negative control the earlier draft expected, a release-profile WASI build failing, does
  not occur; the control is the release-profile line's missing flag against the wasi
  profile's explicit one. In the `.wasm`, every
  `#[napi(catch_unwind)]` has nothing to catch; what a panic does from JavaScript's side (a
  trap, surfaced however the runtime surfaces it, with the instance's state unknown) is
  observed in the probe and written into the README from the observation, not promised here.
  The core's no-panic proof covers the core's own code and excludes nothing else: memory
  traps, allocation failure and faults in dependencies remain possible on this path. A
  test-only export is not added to a shipped artifact.
- **Bun under forced WASI is measured, not promised.** A forced-WASI failure on Bun does not
  block the release, since Bun fallback support is excluded above; native Bun must stay green.
  The README states the Bun version tested and the result, and the WebAssembly benchmark column
  says "unavailable" for Bun if initialisation fails.
- **Browser: a Chromium leg in CI, and only that claimed.** A Vite page over the WASI
  package's browser loader runs in headless Chrome; Firefox and Safari are stated untested, in
  the Blazor archive's words.
- **CI shape: the WASI job in the Forge's wasm workflow, consumer tests in ci.yml, one pack
  script for both.** Wasm work sits with wasm work, where headless Chrome already runs; the
  release packs exactly what ci.yml tested.

## The packages and the loader

`typescript/package.json`:

- `napi.targets` gains `wasm32-wasip1`, and `scripts` gains `build:wasi`: `napi build --profile
  wasi --target wasm32-wasip1 --js binding.js --dts binding.d.ts`, the one command CI and the
  local loop both run (CLAUDE.md's TypeScript paragraph names it). `napi build` then regenerates
  `binding.js` with the
  flavour list `['wasm32-wasip1']` and its fallback chain (the local `./hyperuuid.wasip1.cjs`
  first, then the package `@skunkwerkx/hyperuuid-wasm32-wasip1`; its version is compared with the
  root's only when `NAPI_RS_ENFORCE_VERSION_CHECK` is set, so the pin in the root's
  `dependencies` is what holds the two together in practice). The regenerated loader and
  `binding.d.ts` are committed; CI's `git diff --exit-code` on the four wrapper files keeps
  holding them. `index.js` and `index.d.ts` do not change.
- `engines.node` becomes `^22.13.0 || >=23.5.0`, the exact range the CLI writes on the WASI
  package (review finding 3: `>=22.13.0` would admit Node 23.0 to 23.4, which the child
  excludes, and a hard dependency must not carry a narrower floor than its parent). Still
  "Node 22" in the floor lists; the floor sentences say 22.13, the Node floor entry in CLAUDE.md
  gains the range, and `node_floor_versions` in ci.yml stays `22 24`. The consumer tests run the
  exact floor, `node:22.13.0-alpine`, beside the floating `node:22-alpine`, and check the packed
  graph with `engine-strict=true`.
- `devDependencies` add `emnapi`, `@emnapi/core` and `@emnapi/runtime` at one pinned version
  (the CLI refuses a mismatch between the three) and `@napi-rs/wasm-runtime`, pinned. The
  lockfile is then the single source of the versions the WASI package declares, and
  Dependabot's monthly PR moves all four with `@napi-rs/cli`.
- `napi.wasm.initialMemory` (and `threadlessInitialMemory` if the CLI needs the threadless
  one set apart) is set from the probe's measurement, not napi's 4000-page (250 MiB) default;
  `maximumMemory` stays the 4 GiB default, since a `MAX_V7_BATCH` batch is 1 GiB of guest
  memory.
- `files` is unchanged: the root tarball carries JavaScript and types only. The local build's
  `hyperuuid.wasm32-wasip1.wasm`, `hyperuuid.wasip1.cjs`, `hyperuuid.wasip1-browser.js`, the
  deferred and workerd entries and their type definitions join `*.node` in `.gitignore`. The
  `.cargo/config.toml` comment that says the wasm target is never built goes. The
  loader's first WASI candidate is that local file, so `NAPI_RS_FORCE_WASI=error npm test` runs
  in a checkout with nothing else installed.

Ten published packages: the root `hyperuuid`, eight `@skunkwerkx/hyperuuid-<platform>` natives
and `@skunkwerkx/hyperuuid-wasm32-wasip1`. The root's two dependency maps are injected at pack
time and never committed, as `optionalDependencies` already is; the committed `package.json`
carrying either fails the pack. The WASI package's `dependencies` are pinned by the pack script
from `typescript/package-lock.json`, replacing the `~<latest>` that `napi create-npm-dirs`
writes. That rewrite pins the output; it does not remove the lookup: `create-npm-dirs` still
queries the registry for the latest `@napi-rs/wasm-runtime` while it runs, so the pack step
needs the network for that call and its result is discarded. The owner publishes a tenth `0.0.1`
placeholder and registers `release.yml` as its trusted publisher before 1.1.0, with the other
nine.

## The Forge

- `hyper_config.py`: `NODE_WASI_PLATFORM = 'wasm32-wasip1'` as the default when `node` is on,
  with a `node.wasi` override in `hyper.json`; the empty string means no WASI package, which the
  siblings keep until they carry TypeScript. `npm_names` returns the root, the natives and the
  WASI name; `release_inventory.py` counts from it, so the inventory expects ten.
- `release_package_metadata.py`'s `npm_tarball`: the root's `dependencies` must be exactly the
  WASI package at the version and its `optionalDependencies` exactly the natives; a native
  tarball holds one `.node` and no `.wasm`; the WASI tarball holds no `.node`, one `.wasm`, its
  loaders, and a manifest with no `os`, `cpu` or `libc` and `dependencies` equal to the
  lockfile's versions; the root holds neither. The Forge tests that say nine say ten, and the
  WASI tarball shape gets its own cases, positive and negative.
- A new `npm_package.py` owns the pack body, with two operations over one set of functions
  (second-round finding 3: the browser leg has one artifact, not nine, and must not grow its
  own packaging). `pack` is the complete pack: copy `typescript/` under a scratch directory,
  `npm ci --ignore-scripts`, `napi create-npm-dirs`, copy each artifact into its platform
  directory (the eight `.node` files; the WASI directory's `.wasm` and loaders), rewrite the
  WASI package's `dependencies` from the lockfile, `npm pkg set` the root's two maps, `npm
  pack` each, then the tarball checks (ten tarballs, one `.node` per native, the WASI shape,
  none in the root). `pack-wasi` takes only the `node-wasm32-wasip1` artifact: the same
  scratch copy, `create-npm-dirs`, the same dependency rewrite from the lockfile, `npm pack` of
  the WASI directory alone, and the same WASI tarball checks; it writes no root maps and packs
  no other tarball. `pack` calls the functions `pack-wasi` is made of for its WASI member, so the
  tarball the browser leg tests has the same artifact inputs, dependency pinning and packaging
  checks as the one the release packs (shared functions alone prove no more than that; the
  tarballs are packed from different runs). `pack-npm` keeps the
  download, the provenance verification (now over `node-wasm32-wasip1` too), the attestation
  and the upload, and calls `pack`; ci.yml's consumer test calls `pack` with the same artifacts
  and no attestation, from a Forge checkout at master under `.hyperforge`, as `release.yml`
  does; `build-node-wasi` calls `pack-wasi` on the artifact it just built. `test_forge_actions.py`'s
  pack-npm test follows, and `pack-wasi` gets its own tests, positive and negative.
- `hyper-build-wasm.yml`: a `node_wasi` input (default false; ci.yml passes true) and a
  `build-node-wasi` job, first in the workflow's order as the newest code: `rustup target add
  wasm32-wasip1`; `npm ci`; `napi build --profile wasi --platform --target wasm32-wasip1 --js binding.js
  --dts binding.d.ts`; the wrapper diff; `NAPI_RS_FORCE_WASI=error npm test` and `npm run
  typecheck`; `npm_package.py pack-wasi` over the build's output, from the workflow's own Forge
  checkout; the browser page (below) against that tarball; upload `node-wasm32-wasip1` (the
  `.wasm` and the generated loaders, as a directory) and attest it under the same conditions as
  the native legs. No wasi-sdk:
  Rust's bundled wasi-libc links a pure-Rust addon through `rust-lld`, and the CLI picks the
  emnapi archive to match it.
- `publish-npm` is unchanged: the WASI tarball matches its `skunkwerkx-<crate>-*-<version>.tgz`
  platform glob and publishes before the root that depends on it.

## The consumer tests: ci.yml's `test-npm-package`

Needs `build-native` and `build-wasm`; a matrix over `ubuntu-26.04`, `macos-26` (arm64) and
`windows-2025` (x64), release mode and PR mode alike, PR mode packing without osx-x64 (`--skip osx-x64`, the native `hyper-build-native` does not build there, as `test-c` does) (review finding 5: a native pass on an OS
proves nothing about WASI initialisation and file resolution there). Each leg packs the ten
tarballs with `npm_package.py` from the run's artifacts and serves them from a local Verdaccio
registry, so `npm install` resolves, selects platforms and writes lockfiles exactly as it would
against npmjs. The npm that runs the scenarios is the one `setup-node` ships with the pinned
Node; its version is printed and asserted, since finding 2 below depends on npm's behaviour.
Each leg first runs the full suite under forced WASI (second-round finding 2: a smoke script
proves installation, not the API; byte-buffer mutation, batches, timestamps and error
propagation need the conformance tests, and on Windows the loader's filesystem preopen and
path handling are untested until then). The leg checks out the repository, runs `npm ci` in
`typescript/`, places the `node-wasm32-wasip1` artifact's files beside `binding.js`, where the
loader's first WASI candidate looks, with no `.node` built or downloaded, and runs
`NAPI_RS_FORCE_WASI=error npm test`, the same command `build-node-wasi` runs on Linux; on Bun
too, where the native suite runs on Bun, with the result recorded under the Bun decision above.
A Node conformance failure on any matrix OS is a failure of the job; Bun's stays recorded. Then the installation scenarios, each a
fresh consumer directory with one script that requires `hyperuuid`, generates a v4 and a v7,
checks one v5 corpus vector and one SQL-order vector, and prints `isAvailable()` and
`binding.__napiBindingTarget` (`native` or `wasm32-wasip1`), which the step asserts.
`request-staging` adds `test-npm-package` to its `needs`, and `test_workflow_contracts.py` pins
it there beside `test-nuget-package` (review finding 8): the staging request waits on consumer
success, not merely on the job existing.

On every OS:

1. **Plain install.** Loads `native`; the WASI package is in `node_modules` and stderr is empty
   (no `ExperimentalWarning`: WASI was never initialised).
2. **`--omit=optional`.** Loads `wasm32-wasip1`, with Node's `ExperimentalWarning: WASI is an
   experimental feature` on stderr, which the step tolerates and the README states.
3. **Absent native binary.** A plain install, then this platform's `.node` deleted:
   `wasm32-wasip1` loads and `nativeVersion()` equals the crate's version.
4. **Forced WASI.** `NAPI_RS_FORCE_WASI=error` on the plain install, on Node; on Bun too where
   the native suite already runs on Bun (not win-arm64, which is not in this matrix anyway).
   Bun's result is recorded; see the decision above.
5. **Negative controls** (review finding 1, replacing a control that could never run: an
   unpublished mandatory dependency fails `npm install` before `require` does).
   - The WASI package unpublished from the registry, empty npm cache: `npm install` must fail,
     and the error must name `@skunkwerkx/hyperuuid-wasm32-wasip1`.
   - A successful `--omit=optional` install, then the installed WASI package directory
     deleted: `require` succeeds, `isAvailable() === false`, and a door throws
     `ERR_HYPERUUID_UNAVAILABLE` whose `cause` chain names both the native attempts and the WASI
     one.
   - A successful `--omit=optional` install, then only the `.wasm` deleted with the loader left
     in place: the same observable outcome, with the loader's own error in the chain.

On Linux only:

6. **The damaged Mac lockfile** (review finding 2, correcting a scenario that confused installed
   packages with lockfile contents: a clean platform-targeted install writes every optional
   platform into the lockfile, so `npm ci` on Linux would get its native package). The 4828
   shape is: install, delete only the lockfile, keep `node_modules`, `npm install` again; the
   regenerated lockfile lists only the platform package that was on disk. Reproduced on one
   Linux runner with `npm install hyperuuid --os=darwin --cpu=arm64`, `rm package-lock.json`,
   `npm install --os=darwin --cpu=arm64` again; the step asserts the lockfile now names
   `darwin-arm64` and none of the other seven natives before going on. Then `rm -rf
   node_modules`, `npm ci` with no platform override: no native package is present and
   `wasm32-wasip1` loads. If the pinned npm repairs the lockfile instead (the thread reports
   fixes and regressions across 11.x), the assertion fails loudly and a committed
   damaged-lockfile fixture (`typescript/consumer-tests/fixtures/mac-lockfile/`) becomes the
   regression test, run the same way.
7. **The exact floor.** Scenario 2 inside `node:22.13.0-alpine`, with `engine-strict=true` so an
   `engines` mismatch anywhere in the packed graph fails the install, and again inside the
   floating `node:22-alpine` and `node:24-alpine` for coverage (review finding 3).
8. **An emulated non-x64/arm64 host.** `node:24-alpine` for `linux/s390x` under QEMU (the
   repo's existing binfmt setup) installs from the registry; no native package exists for it,
   so `wasm32-wasip1` loads on a big-endian host. Described everywhere as emulated.
9. **Bundlers** (review finding 6: a bundle that loads native proves nothing about wasm
   inclusion). esbuild (`--bundle --platform=node`) and webpack bundle the consumer; the output
   is copied to an isolated deployment directory with no access to the original `node_modules`
   and run there under `NAPI_RS_FORCE_WASI=error`, asserting `wasm32-wasip1` loaded. If the
   plain bundle fails, the step then bundles with `hyperuuid` external, deploys the bundle with
   `npm install --omit=dev` of its declared dependencies in the deployment directory, and that
   configuration must pass. The README documents the configuration that passed, and only that.
   Recording a failure alone does not close this scenario.

## The browser page

`typescript/browser-smoke/`: a Vite project whose page imports
`@skunkwerkx/hyperuuid-wasm32-wasip1` by package name, so the bundler resolves the `browser`
condition of the package's `exports` map to `hyperuuid.wasip1-browser.js` (review finding 4: the
exports map does not expose the loader file as a subpath, so a direct file import could pass
where every consumer fails). The package is installed from the packed tarball, not from a
directory. The page runs the conformance corpus (v5, SQL order both ways, inspection, refusals)
and the generation doors, and writes `PASS` or the first failure into the DOM. The
`build-node-wasi` job installs the tarball its `npm_package.py pack-wasi` step produced (the
shared packer's WASI-only operation, above: the same dependency pinning and tarball checks as
the release pack, with no need for the eight native artifacts), builds, serves `dist/` with
plain static headers and no COOP/COEP, which is what the threadless flavour buys, and loads it
in the runner's Chrome (`--headless=new --dump-dom` under a bounded timeout, the Ruby wasm leg's
pattern, with the exit code enforced): the step fails unless Chrome exits 0 and the DOM's last
line of output is `PASS` with no `FAIL` before it; a corrupted corpus expectation is the control.
The page is a smoke test in the repository, not a shipped artifact; it is linted and formatted
with the rest of `typescript/`. Claimed: tested in Chromium on Linux, without cross-origin
isolation. Firefox and Safari: untested, in the words `docs/platforms.md` uses for the Blazor
archive.

## Measurements and docs

- `bench/bench.ts` runs on the Apple M6 under `NAPI_RS_FORCE_WASI=error` on Node and on Bun, and
  the TypeScript README's table gains a WebAssembly column beside native, dated and labelled
  with the machine. The loss is printed, with the `.wasm` size and the install's extra bytes
  (the WASI package and its three runtime dependencies).
- `typescript/README.md`: Install names ten packages and what the tenth is for; the platform
  table gains an "everything else" row; the floor sentence says Node 22.13; "When the addon is
  missing" becomes "When no native addon fits": the fallback, the `ExperimentalWarning`, the
  Bun result, the bundler rule, the browser statement, and what `--omit=optional` now does.
- Root README: the TypeScript platform row and the package count in the release paragraph;
  `docs/platforms.md`'s TypeScript row; `docs/provenance.md` where it counts tarballs; CHANGELOG
  (the TypeScript bullet loses "No WebAssembly fallback yet" and gains the fallback; the
  plumbing bullet says ten); `.github/RELEASING.md`; CLAUDE.md's npm list ("nine" to "ten", the
  WASI package and its lockfile-pinned runtime dependencies as a list that moves together, the
  Node floor patch) and its open-follow-ups count; the Forge runbook's npm paragraph; ROADMAP
  item 2 marked done in 1.1.0 with the summary table's npm cell and the TypeScript landing note.
- `check_exports.py` is untouched: the `.wasm` exports the same `#[napi]` set. `catch_unwind`
  stays on every function for the native builds; on the WASI build, under the `wasi` profile,
  there is no unwinder for it to catch with (panics abort, above), and the README's fallback
  section says so with the observed behaviour.

## Failure modes and where each is caught

| Failure | Caught by |
| --- | --- |
| A CLI release writes `cpu`/`os`/`libc` on the WASI package | `npm_package.py`'s manifest assertion, at pack and in ci.yml |
| The WASI package's runtime dependencies drift from the lockfile | the pack script pins them; `npm_tarball` compares |
| `binding.js`'s flavour list and the built target disagree | the wrapper diff after every build |
| The WASI loader cannot find its `.wasm` or its runtime | scenario 5's three controls, scenario 2's load |
| A floor regression in the loader, or an `engines` mismatch in the graph | scenario 7 on `node:22.13.0-alpine` with `engine-strict` |
| A native install starts initialising WASI | scenario 1's empty stderr on three OSes |
| WASI file resolution differs on Windows or macOS | scenarios 2 to 4 on those legs |
| npm's lockfile behaviour changes under us | scenario 6's lockfile assertion, then the fixture |
| The root ships without the tenth package or with it unpinned | `npm_tarball`'s root check; the inventory count |
| The browser `exports` condition breaks | the Chromium page, importing by package name from the tarball |
| A consumer failure reaches staging | `request-staging` needs `test-npm-package` |

## Out of scope

Firefox and Safari; Deno; Cloudflare Workers and other edge runtimes (the CLI's workerd entry
ships because the CLI emits it, with no claim); a threaded build; suppressing Node's
`node:wasi` warning on consumers' behalf; panic containment on the WASI path; a wasm build of
the Rust core's own tests.

## Open until the probe

The `.wasm` size, the initial memory, the panic strategy in the effective rustc invocation and
what a panic looks like from JavaScript, the Bun result, and the bundler configuration that
passes are measured, not assumed; each is written into the README from the measurement.
