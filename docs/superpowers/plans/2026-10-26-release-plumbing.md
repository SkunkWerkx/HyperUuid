# npm and CRAN Release Plumbing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** CI legs for the TypeScript and R bindings, npm packing and publishing, CRAN tarball packing and attestation, and the version bump, all in the Forge and driven by inputs, with this repo wired to them and its docs updated.

**Architecture:** The Forge (`/Users/buvy/Developer/SkunkWerkx/HyperForge`, checked out beside this repo) gains `node` and `r` legs in `hyper-build-native.yml`, three composite actions (`pack-npm`, `pack-r`, `publish-npm`), npm/cran inventory groups, and directory-conditional version files. Everything new defaults off so HyperCast and HyperTabular, which call the Forge at master, are untouched until their cascade. This repo passes the inputs, adds the three release jobs, and updates every count and list the spec names.

**Tech Stack:** GitHub Actions, Python 3.14 unit tests (`unittest`), actionlint, `r-lib/actions`, `actions/setup-node`, `oven-sh/setup-bun`, npm ≥ 11.5.1 (Node 24) for Trusted Publishing.

**Spec:** `docs/superpowers/specs/2026-10-26-r-and-typescript-bindings-design.md` (sections "Forge changes", "This repo's wiring", "Release"). Prerequisites: the TypeScript and R binding plans are implemented in the working tree (the legs run their suites).

## Global Constraints

- Owner drives git in both repos. The Forge is unpinned at master, so nothing here is merged while a release is in flight, and every Forge default keeps HyperCast and HyperTabular's behaviour byte-for-byte: new workflow inputs default `false`, new version files are directory-conditional, new inventory groups are optional. (The spec said `node`/`r` default true with the siblings passing false; this plan defaults them false and HyperUuid passes true, which needs no sibling edit and no merge ordering. Flip the defaults in the cascade.)
- Forge paths: composite actions under `.github/actions/<name>/action.yml`; scripts and their tests flat in `.github/scripts/` (`test_*.py`); run with `python3 -m unittest discover -s .github/scripts -p 'test_*.py' -v` from the Forge root. This repo's tests: `python3 -m unittest discover -s .github/scripts/tests -v`.
- Third-party actions pinned to a commit with `# vX.Y.Z` trailing; Forge calls `@master`. No `${{ secrets.` inside composite actions. Every `run` has a `shell`; every input a description and required-or-default.
- Identity rule: anything that signs for the consumer or publishes is a composite action run from `.hyperforge`; the consumer's OIDC identity is what npmjs.com registers.
- npm: nine tarballs per release, `hyperuuid-<v>.tgz` and `skunkwerkx-hyperuuid-<name>-<v>.tgz` for the eight napi names; the root's `optionalDependencies` are injected at pack time (`npm pkg set`), never committed. Never `napi prepublish`. Publish order: the eight platform packages, then the root. First publish of every package is by the owner with a granular token; this plan only wires OIDC.
- CRAN: one `hyperuuid_<v>.tar.gz` in the `cran/` group, attested, inventoried, never uploaded by any job; the owner submits by hand from the preserved bytes.
- Version files: `typescript/package.json` joins `default_version_files` only when `typescript/` exists (regex `^  "version": "([^"]+)",$`), `r/DESCRIPTION` only when `r/` exists (`^Version: (.+)$`). `typescript/package-lock.json` is regenerated (`npm install --package-lock-only --ignore-scripts`) by prepare-release when `typescript/package.json` is in the list; it is not a version file. (Spec said a multi-match bump mode; regenerating the lock makes that unnecessary.) `typescript/Cargo.lock` and `r/src/rust/Cargo.lock` join the companion-lockfile loop (`cargo update --package <crate>`), the R one after `r/tools/vendor.sh --stage` has put the core copy in place.
- Node: `node_version` default `"26"`, `node_floor_versions` default `"22 24"`, `bun_version` default `"latest"`. R: `r_version` default `"release"`, `r_floor_version` default `"oldrel"`.

## Review Focus

1. A consumer with `typescript/` but no `node:` input (HyperCast mid-cascade): `node` defaults false so the leg simply does not run; the `default_version_files` still picks up `package.json`. Pinned in Task 1 (test with the directory present and absent).
2. `npm pack` at the root after `npm pkg set` must leave the working tree's `package.json` unchanged for later steps: the pack action copies `typescript/` to `$RUNNER_TEMP` first. Pinned in Task 5 (the action's self-check greps the committed file for `optionalDependencies` and fails if found).
3. A platform tarball that names a package whose version differs from the root's (a stale `npm/` dir): `validate_package_metadata`'s `npm_tarball` validator reads `package/package.json` from each tarball and requires `name` in the nine expected names and `version == version`. Pinned in Task 4.
4. `registry_plan` for npm when the root package exists but a platform package is missing (a half-published first release): pending must list exactly the missing ones and the publish loop must push platform packages before the root. Pinned in Task 4 (fixture with one 404).
5. The R tarball missing one corpus file must fail the tag run, not the CRAN check weeks later. Pinned in Task 4 (`r_source_tarball` validator lists the five members) and Task 5 (`pack-r` runs `vendor.sh --verify` and the tar grep).

---

### Task 1: `hyper_config.py`: conditional version files and the napi platform map

**Files (Forge):**
- Modify: `.github/scripts/hyper_config.py` (`default_version_files`, a module constant `NODE_PLATFORMS`, `HyperConfig.node_platforms`, `shell()`, `json` mode)
- Test: `.github/scripts/test_hyper_config.py`

**Interfaces:**
- Produces: `NODE_PLATFORMS = {'linux-x64': 'linux-x64-gnu', 'linux-arm64': 'linux-arm64-gnu', 'linux-musl-x64': 'linux-x64-musl', 'linux-musl-arm64': 'linux-arm64-musl', 'osx-x64': 'darwin-x64', 'osx-arm64': 'darwin-arm64', 'win-x64': 'win32-x64-msvc', 'win-arm64': 'win32-arm64-msvc'}`; `cfg.node_platforms` (overridable by `hyper.json` key `node.platforms`); `shell()` emits `declare -A NODE_PLATFORMS=(...)`; `json` includes `node_platforms`; `default_version_files()` adds the two conditional entries keyed on `(self.root / 'typescript').is_dir()` and `(self.root / 'r').is_dir()`.

- [ ] **Step 1: Write the failing tests** in `test_hyper_config.py`: `test_version_files_follow_the_binding_directories` (a `tmp_path` root with only the classic layout → the five paths; with `typescript/` and `r/` directories → seven, the new two with the regexes above); `test_bump_ignores_bindings_a_consumer_lacks` (write the five classic files with `1.0.0`, call `bump(cfg, root, '1.1.0')`, assert no `FileNotFoundError` and all five changed); `test_bump_fails_loudly_when_a_binding_lost_its_manifest` (`r/` dir present, no `DESCRIPTION` → `FileNotFoundError`); `test_node_platforms_in_shell_and_json`. Keep `test_defaults_derive_every_layout_name...`'s `set(cfg.version_files) == {5}` passing for a root without the directories (it uses `write_config` into a tmp root today; check it does, else point it at one).
- [ ] **Step 2: Run** the Forge suite. Expected: the new tests FAIL.
- [ ] **Step 3: Implement.** `default_version_files` reads `self.root`; `HyperConfig.load(root)` already sets it.
- [ ] **Step 4: Run** the Forge suite (PASS). Then from each sibling: `python3 ../HyperForge/.github/scripts/hyper_config.py version-files --root ../HyperCast | wc -l` → 5, same for HyperTabular; from HyperUuid → 7.

---

### Task 2: `hyper-prepare-release.yml`: the npm lock and the header

**Files (Forge):**
- Modify: `.github/workflows/hyper-prepare-release.yml` (lines 66-107: after `cargo check`, add `if [ -f typescript/package.json ]; then (cd typescript && npm install --package-lock-only --ignore-scripts); fi` with a setup-node step before it guarded by the same test; before the companion-lockfile loop at 79-83, `if [ -d r ]; then bash r/tools/vendor.sh --stage; fi` so the R wrapper's path dependency exists; the loop list becomes `rust/fuzz rust/browser-test typescript r/src/rust` (both new wrapper crates path-depend on the core, and `cargo vendor --locked` fails on a lock that still names the old version); the commit step `git add`s `typescript/package-lock.json`, `typescript/Cargo.lock` and `r/src/rust/Cargo.lock` under the same guards; header comment lines 3-10 name `typescript/package.json` and `r/DESCRIPTION` as conditional and the four companion locks)
- Test: `.github/scripts/test_forge_producers.py` if it asserts the prepare workflow's shape (read it first; add an assertion that the lock step is guarded on the file's existence).

- [ ] **Step 1: Read** `test_forge_producers.py` for prepare-release assertions. **Step 2: Edit** the workflow. **Step 3: Run** `actionlint -shellcheck= -ignore 'property "workflow_(sha|repository)" is not defined' .github/workflows/hyper-prepare-release.yml` and the Forge suite. Expected: clean, PASS.

---

### Task 3: CI legs: `hyper-build-native.yml`, the musl `node)` case, `hyper-check-core.yml` lints

**Files (Forge):**
- Modify: `.github/workflows/hyper-build-native.yml` (inputs after `ruby` at line 244: `node`, `r` booleans default false with the comment pattern of lines 215-219; `node_version`, `node_floor_versions`, `bun_version`, `r_version`, `r_floor_version` strings; the `mode` description's "eight languages" → "ten"; a Node leg and an R leg after the Python steps, before the C# suite at line 627; a Node cross-build in `build-native-osx-x64`; Node floor reruns and a Node suite in `build-native-musl`)
- Modify: `.github/actions/musl-suite/suite-musl.sh` (a `node)` case; header comment line 6) and `musl-suite/action.yml` if a new input is needed (none: the case reads `SUITE_IMAGE`)
- Modify: `.github/workflows/hyper-check-core.yml` (`lint_node`, `lint_r` boolean inputs default false; jobs `lint-node` and `lint-r` in the shape of `lint-python` at line 550)
- Test: `.github/scripts/test_forge_producers.py` (whatever it asserts about input lists or leg guards; read first)

**Interfaces:**
- Produces, Node leg (guard `matrix.shard != 'swift-ruby' && !cancelled() && steps.native_lib.conclusion == 'success' && inputs.node`): `actions/setup-node` (pinned) with `node-version: ${{ inputs.node_version }}`; `npm ci` in `typescript/`; `npm run build` (napi picks the host target; on win-arm64 and linux-arm64 the runner is native); `npm test`; `npm run typecheck`; `oven-sh/setup-bun` (pinned) with `bun-version: ${{ inputs.bun_version }}` and `npm run test:bun`, the Bun step skipped on win-arm64 unless Bun ships a build (`if: matrix.rid != 'win-arm64'` until verified); upload `node-${{ matrix.rid }}` with `typescript/*.node`, attested in release mode like `native-${rid}` (lines 504-511). osx-x64 job: `npm ci`, `npx napi build --release --platform --target x86_64-apple-darwin --js binding.js --dts binding.d.ts` (never without `--js`/`--dts`: napi then writes its own loader over the hand-written `index.js`/`index.d.ts`, proven locally in the TypeScript plan's Task 8), then `git diff --exit-code -- typescript/index.js typescript/index.d.ts typescript/binding.js typescript/binding.d.ts` as the guard on every leg that runs a napi build, `lipo -archs` = `x86_64`, then `actions/setup-node` with `architecture: x64` and `npm test` under Rosetta; upload `node-osx-x64`. Musl job: `musl-suite` with `binding: node` for `node:<node_version>-alpine`, a second call per floor version, and the `.node` it builds uploaded as `node-${rid}` from `out-musl`.
- R leg (guard as above with `inputs.r`, and `matrix.rid != 'win-arm64'` with a comment: R on Windows ARM64 is experimental, revisit): `dtolnay/rust-toolchain@stable` with `targets: x86_64-pc-windows-gnu` on Windows; `r-lib/actions/setup-r@v2` (pinned SHA) with `r-version: ${{ inputs.r_version }}`, `use-public-rspm: true`; `setup-r-dependencies@v2` with `working-directory: r`, `extra-packages: any::rcmdcheck`; `bash r/tools/vendor.sh`; `check-r-package@v2` with `working-directory: r`, `args: 'c("--no-manual", "--as-cran")'`, `error-on: '"note"'`; a second setup-r + check pair for `inputs.r_floor_version` when non-empty and different, on linux-x64 only.
- `suite-musl.sh` `node)`: image `node:${NODE_VERSION:-26}-alpine` (or `SUITE_IMAGE`); `apk add build-base`; install rustup for the musl host with `-crt-static` and the unwinder shim exactly as `build-musl.sh` lines 66-68; `npm ci && npm run build && npm test`; `readelf -d typescript/*.node` NEEDED must be `libc.musl-*.so.1` only (the `.node` links std, so the static unwinder matters here); copy the `.node` to the out dir; then a second container `oven/bun:alpine` running `bun test tests/` against the same `.node`.
- `lint-node`: setup-node 26, `npm ci`, `npm run lint`, `cargo fmt --check` and `cargo clippy --release -- -D warnings` in `typescript/`. `lint-r`: setup-r release, `lintr::lint_package("r")` failing on any lint, `cargo fmt --check --manifest-path r/src/rust/Cargo.toml`.

- [ ] **Step 1: Read** `test_forge_producers.py` and `hyper-build-native.yml` lines 128-331, 518-630, 1143-1198, 1230-1410 before editing.
- [ ] **Step 2: Edit** the three workflows and the script per the interfaces. Keep every existing guard shape; copy the Python floor rerun (1392-1401) for Node.
- [ ] **Step 3: actionlint** both workflows (the command in Task 2) → clean. **Step 4:** Forge suite → PASS. `bash -n .github/actions/musl-suite/suite-musl.sh` → ok.
- [ ] **Step 5: Local proof of the musl case** (Docker is available on this Mac?): `docker run --rm -v "$PWD:/src:ro" node:26-alpine sh -c 'apk add --no-cache build-base curl && curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && ...'` following the case's body against a copy of `typescript/` and `rust/`. If Docker is absent, say so in the PR report: the musl Node suite is CI-only.

---

### Task 4: Inventory, validators, registry plan, release notes

**Files (Forge):**
- Modify: `.github/scripts/release_inventory.py` (`REGISTRIES`, a new `INVENTORY_GROUPS = REGISTRIES + ('cran',)` used by `payload_files` line 75 and `validate_layout` line 83 in place of the literal tuple; `validate_layout` branches `npm` and `cran`; signer rule line 142: `name.startswith(('ruby/', 'npm/', 'cran/'))` → consumer; `registry_plan` branch `npm`; `--registry` choices stay `REGISTRIES` which now includes `'npm'`; `PLATFORMS` replaced by `hyper_config.RUBY_PLATFORMS`; the 25-wheel literal at 95-96 replaced by `cfg.wheels`)
- Modify: `.github/scripts/release_package_metadata.py` (`npm_tarball(path, crate, version, platforms)` and `r_source_tarball(path, crate, version)` validators; line 196's literal replaced by `cfg.wheels` and `len(cfg.ruby_platforms) + 2`; `validate_package_metadata` validates `npm/` and `cran/` when present)
- Modify: `.github/scripts/release_notes.py` line 31 (add npm and "a CRAN source tarball the maintainer submits by hand"); `.github/actions/prepare-maven-package/action.yml` lines 84-95 and `.github/workflows/hyper-pack-nuget.yml` lines 109-124 (read `LIB_NAMES` from `hyper_config.py shell` instead of the hand-written map)
- Test: `.github/scripts/test_release_inventory.py`, `test_package_inventory_metadata.py` (extend `create_packages` with `npm/` and `cran/` fixtures: nine minimal tgz with a `package/package.json`, one `hyperuuid_<v>.tar.gz` with the five corpus members and a `DESCRIPTION`), `test_release_notes.py`, `test_release_provenance.py` if it covers the Maven/NuGet lib map

**Interfaces:**
- `validate_layout`: `npm/` holds exactly `{crate}-{version}.tgz` plus `skunkwerkx-{crate}-{suffix}-{version}.tgz` for each `cfg.node_platforms` value; `cran/` exactly `{crate}_{version}.tar.gz`. Both groups are optional (absent is fine) so siblings without them still validate.
- `npm_tarball`: reads `package/package.json` with `json.loads` (no other entries executed), `name` ∈ `{crate} ∪ {'@skunkwerkx/' + crate + '-' + s}`, `version == version`; the root tarball's `optionalDependencies` must map every platform name to exactly `version`.
- `r_source_tarball`: `tar_entries` (existing helper) must contain `{crate}/DESCRIPTION` with `Version: {version}`, `{crate}/src/rust/vendor.tar.xz`, `{crate}/src/rust/core/Cargo.toml`, and the five `{crate}/tests/testthat/corpus/<file>` members (the list is a module constant `R_CORPUS_FILES`).
- `registry_plan('npm')`: for each tarball read its name/version, fetch `https://registry.npmjs.org/{quoted name}/{version}` (404 → pending); else compare `dist.integrity` with the local sha512 (`sha512-` + base64), fetch `dist.tarball` (host must be `https://registry.npmjs.org/`) and byte-compare like the pypi branch; mismatch raises the registry-conflict error. Pending files are ordered platform packages first, root last (the publish loop iterates in that order).

- [ ] **Step 1: Write the failing tests** (layout accept/reject for both groups; signer rule for `npm/` and `cran/`; both validators accept the fixture and reject a wrong version, a wrong name, a missing corpus member, an optionalDependency at another version; the npm registry plan with a fake `fetch` returning 404 for one platform package → pending is exactly that one plus nothing else, and a root-only-missing case orders the root last; the Maven/NuGet lib map test if present; release notes name npm and CRAN).
- [ ] **Step 2: Run** (FAIL). **Step 3: Implement.** **Step 4: Run** the Forge suite (PASS), and the existing `test_release_inventory.py` cases untouched.

---

### Task 5: Composite actions `pack-npm`, `pack-r`, `publish-npm`; `release-gate`; `finalize-release`

**Files (Forge):**
- Create: `.github/actions/pack-npm/action.yml`, `.github/actions/pack-r/action.yml`, `.github/actions/publish-npm/action.yml`
- Modify: `.github/actions/release-gate/action.yml` (two downloads after line 69: `npm-packages` → `release-packages/npm`, `cran-packages` → `release-packages/cran`, each behind a boolean input, `npm_packages` and `cran_packages`, defaulting `false`, so siblings' gates stay unchanged; the summary lines 126-143 list them), `.github/actions/finalize-release/action.yml` (nothing, the notes come from `release_notes.py`)
- Test: `.github/scripts/test_forge_actions.py` (`RELEASE_ACTIONS` gains the three; the mutation/wait dict at 184-209 gains `publish-npm`: mutation step "Publish only missing preserved npm packages", wait step "Verify every public npm package matches the prepared bytes")

**Interfaces:**
- `pack-npm` inputs `run_id`, `source_sha`, `token`, `node_version` (= "24"). Steps: download `node-*` → `artifacts/node` (eight dirs); verify provenance of every `.node` (the pack-gems loop, lines 60-76); setup-node; copy `typescript/` to `$RUNNER_TEMP/pack` and work there; `npm ci`; `npx napi create-npm-dirs`; place each `.node` into `npm/<name>/` (`npx napi artifacts --npm-dir npm -d ../artifacts/node` or a copy loop keyed on `NODE_PLATFORMS` from `hyper_config.py shell`); for each `npm/<name>`: `npm pack --pack-destination ../../out`; at the root: `for each platform name: npm pkg set "optionalDependencies.@skunkwerkx/${CRATE}-${suffix}=${VERSION}"` then `npm pack --pack-destination out`; gate: exactly nine tarballs, each platform tarball contains one `.node`, the root contains none (`tar tzf | grep -c '\.node$'`); self-check that the repo's `typescript/package.json` still has no `optionalDependencies`; attest `out/*.tgz`; upload `npm-packages` (`retention-days: 90`, `if-no-files-found: error`).
- `pack-r` inputs `run_id`? (not needed: the tarball is built from source at B), `source_sha`, `token`, `r_version` (= "release"). Steps: setup-r; `bash r/tools/vendor.sh && bash r/tools/vendor.sh --verify`; `R CMD build r` into `out/`; `tar tzf out/*.tar.gz | grep -c tests/testthat/corpus/` must be 5; attest; upload `cran-packages`.
- `publish-npm` inputs as `publish-rubygems` (`version, source_sha, ci_run_id, forge_sha, token, publication_run_id, publish, node_version` = "24"). Steps: release-identity; download `release-packages`; `release_inventory.py verify --registry npm --workspace .`; setup-node 24 with `registry-url: https://registry.npmjs.org`; `registry-plan --registry npm --output pending-npm` (`id: pending`); "Publish only missing preserved npm packages" `if: steps.pending.outputs.pending == 'true' && inputs.publish == 'true'`: `for f in $(cat pending-npm/order.txt); do npm publish "pending-npm/$f" --access public; done` (the plan writes `order.txt` platform-first); "Verify every public npm package matches the prepared bytes" `if: inputs.publish == 'true'`: `registry-confirm --registry npm`. No separate OIDC step: npm ≥ 11.5.1 exchanges the job's OIDC token itself; a comment says the tag run therefore cannot pre-flight the exchange.

- [ ] **Step 1: Write the failing tests** in `test_forge_actions.py` (the new actions exist, are composite, every `run` has a shell, inputs documented; the mutation gate for `publish-npm`; `release-gate` downloads the two new artifacts under its guards).
- [ ] **Step 2: Run** (FAIL). **Step 3: Write** the three actions and the gate edits. **Step 4: Run** the Forge suite and actionlint over `.github/actions/*/action.yml` (PASS). **Step 5:** Local pack rehearsal: with `typescript/hyperuuid.darwin-arm64.node` from the binding plan, run the pack-npm shell body by hand in `$RUNNER_TEMP`-like scratch with a single platform dir and confirm the root tarball's `package.json` carries the injected dependency and the repo's does not; run the pack-r body (`vendor.sh`, `R CMD build`, the tar grep) → 5.

---

### Task 6: This repo's workflows and contract tests

**Files (HyperUuid):**
- Modify: `.github/workflows/ci.yml` (`build-native` `with:` gains `node: true`, `r: true`; `check-core` gains `lint_node: true`, `lint_r: true`), `.github/workflows/release.yml` (jobs `npm-pack` and `r-pack` shaped on `ruby-pack` lines 144-168 with `pack-npm`/`pack-r`; `npm-publish` shaped on `rubygems-publish` lines 261-289 with `publish-npm`; `publication-gate` `needs` and `if` list the two packers and pass `npm_packages: true`, `cran_packages: true`; `finalize-release` `needs` and `if` list `npm-publish`)
- Modify: `.github/scripts/tests/test_workflow_contracts.py` (`PUBLISHERS` + `npm-publish`; `PACKERS` + `npm-pack`, `r-pack`; `ACTION_JOBS` + the three; the `forge_sha` exemption tuple at line 112 + `npm-pack`, `r-pack`; the check-core input assertion updated to four)

- [ ] **Step 1: Update the contract tests first** and run `python3 -m unittest discover -s .github/scripts/tests -v`. Expected: FAIL on the missing jobs.
- [ ] **Step 2: Edit** `ci.yml` and `release.yml`. **Step 3: Run** the tests (PASS; `ForgeInterfaceTests` runs because `../HyperForge` exists and must accept every new `with:` key) and `actionlint` on both files (clean).

---

### Task 7: Repo housekeeping and docs

**Files (HyperUuid):**
- Modify: `.github/dependabot.yml` (`npm` directory `/typescript`; `cargo` directories gain `/typescript` and `/r/src/rust`), `.gitignore` (lines 81-84's Node comment rewritten; add `typescript/npm/`, `typescript/*.node`, `r/src/rust/core/`, `r/src/rust/vendor/`, `r/src/rust/vendor.tar.xz`, `r/src/.cargo/`, `r/tests/testthat/corpus/`, `r/src/*.o`, `r/src/*.so`, `r/src/*.dll`), `CLAUDE.md` (lines 3-4 "seven other languages" → nine; the C-export list at 35-39 gains `r/src/core.h`, `typescript/src/lib.rs`, `typescript/binding.d.ts`; "Local loop" gains `r/tools/vendor.sh` and `npm run build`; "Lists that move together" gains the napi platform map (`NODE_PLATFORMS` in the Forge, the `napi.targets` list in `typescript/package.json`) and the first-publish-by-token rule; the release-notes rule gains "and set `r/NEWS.md`'s heading to the version"; "Bytes that are persisted" gains the raw-door line; line 117 "seven bindings" → nine; the Forge composite list at 56-72 gains the three), `README.md` (lines 13, 16, 92, 203, 241, 326, 334 counts; the mechanism list at 16, 80 and 332 gains "napi-rs for Node and Bun" and "`.Call` into a vendored static library for R"; badges after RubyGems: npm (`https://img.shields.io/npm/v/hyperuuid`) and CRAN (`https://www.r-pkg.org/badges/version/hyperuuid`); platform table rows after Python: `[TypeScript](typescript/)` ✅ on all eight with npm status, `[R](r/)` "source" on every RID, win-arm64 "experimental", status CRAN; WebAssembly table rows per the spec; the Layout block gains `typescript/` and `r/` lines in its style; Key features count; two benchmark tables dated and labelled with this Mac, kept apart from the WSL2 tables with one sentence saying so), `CHANGELOG.md` (header line 3 "All eight packages" → ten, naming TypeScript and R; `### Added` under Unreleased: one bullet `*(TypeScript)*`, one `*(R)*`, one `*(repository only)*` for the release plumbing), `rust/README.md` lines 153-154 and 177 (the thirteen exports now reach nine bindings; no count change), `corpus/README.md` (the "Replayed by" table needs no new row; add one sentence under Files naming the R staged copy under `r/tests/testthat/corpus/`)

- [ ] **Step 1: Make the edits.** **Step 2: Verify** every README snippet by running it (install snippets keep `<version>`); `git diff --stat` lists only the files above plus the two binding trees; `grep -rn "seven" README.md CLAUDE.md` shows no stale count.

---

### Task 8: Forge docs

**Files (Forge):**
- Modify: `docs/release-runbook.md` (lines 11-18: npm's first-publish rule, by token, then a trusted publisher registered per package; 43-46: the tag run cannot pre-flight npm's exchange; 57 and 164-165: "five registries" → six, plus the CRAN tarball the maintainer submits by hand; a per-registry recovery paragraph for npm after 142-145: reconcile by `dist.integrity`, publish only the missing packages, platform packages before the root; a CRAN line: nothing to recover, the preserved tarball is resubmitted), `README.md` (lines 107, 111, 136: counts and the three new actions), `CLAUDE.md` (lines 58 and 128: npm never lets a version be replaced either; "seven bindings" → nine), `docs/predicates/release-v1.md` (lines 16-24: the nine npm tarballs and the CRAN tarball as subjects; the `signer_repo` row at 37-47: the consumer for gems, npm tarballs and the CRAN tarball)

- [ ] **Step 1: Make the edits.** **Step 2:** `grep -rn "five registries\|five publisher\|seven bindings" docs README.md CLAUDE.md` → nothing stale; every path named in the docs exists (`ls` each).

---

### Task 9: Sibling repos (HyperCast, HyperTabular)

Nothing to edit: every Forge default is off or conditional, and their contract tests pass unchanged. Prove it: in each sibling, `python3 -m unittest discover -s .github/scripts/tests -v` with the edited `../HyperForge` present → PASS, and `python3 ../HyperForge/.github/scripts/hyper_config.py version-files` → the five classic lines. Record both in the PR report. The cascade (their own `typescript/` and `r/`, the Forge defaults flipped) is a later pass, after 1.1.0 proves the recipes.

---

## Verification (whole plan)

- Forge: `python3 -m unittest discover -s .github/scripts -p 'test_*.py' -v` PASS; `actionlint -shellcheck= -ignore 'property "workflow_(sha|repository)" is not defined'` clean over `.github/workflows` and `.github/actions`.
- HyperUuid: `python3 -m unittest discover -s .github/scripts/tests -v` PASS (including `ForgeInterfaceTests`); `actionlint` clean.
- Siblings: Task 9's two commands.
- Pack rehearsals from Task 5 Step 5; the registry-plan fixtures from Task 4.
- Only CI can prove (name them in the PR report): the Node and R legs on every runner, the musl Node suite and NEEDED gate, the osx-x64 Rosetta Node suite, the Windows gnu-target R build, win-arm64 Bun.
- Only the 1.1.0 release can prove: `npm publish` under OIDC with provenance, nine attested tarballs in the inventory, the CRAN tarball in the preserved bytes.
- Owner actions, by name, when asked: create the `@skunkwerkx` npm org; bootstrap-publish the nine packages by token; register `release.yml` as trusted publisher on each; CRAN maintainer address.
