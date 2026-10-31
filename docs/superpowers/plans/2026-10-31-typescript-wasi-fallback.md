# TypeScript: the WebAssembly fallback — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `npm install hyperuuid` works where no native addon fits, through a tenth package, `@skunkwerkx/hyperuuid-wasm32-wasip1`, that the root depends on and napi's loader falls back to, proven by consumer tests on three OSes, an emulated s390x and headless Chromium, with the Forge packing and attesting it like the other nine.

**Architecture:** The addon crate in `typescript/` gains a `wasi` cargo profile (abort) and a `build:wasi` script; `napi build` regenerates the committed `binding.js` with the WASI chain. The Forge gains `npm_package.py` (operations `pack` and `pack-wasi`) used by `pack-npm`, by a new `build-node-wasi` job in `hyper-build-wasm.yml` and by ci.yml's new `test-npm-package` consumer job, which serves the packed tarballs from a local Verdaccio and runs nine install scenarios plus the whole suite under forced WASI on every leg. A Vite page in `typescript/browser-smoke/` imports the package by name and runs the corpus in Chromium.

**Tech Stack:** napi-rs CLI 3.10.9 (`wasm32-wasip1` threadless flavour), rustc 1.99.1 with the `wasm32-wasip1` target, Node 26 / Bun 1.4.4, Verdaccio, Vite, esbuild, webpack, Docker with QEMU (s390x), Python 3 unittest in the Forge, the Forge (`../HyperForge`).

**Spec:** `docs/superpowers/specs/2026-10-31-typescript-wasi-fallback-design.md`

## Global Constraints

- **No git operations.** The owner commits, pushes, tags and dispatches. Every task leaves its work verified in the working tree of `HyperUuid` or `../HyperForge` and reports it; "done" below means verified, not committed. Files are moved or removed with `mv`/`rm`, never `git mv`/`git rm`.
- Forge first, then HyperUuid: the Forge tasks (3 to 6) must be complete and their suite green before the HyperUuid CI task (8) is run, since ci.yml calls the Forge at master.
- The flavour is `wasm32-wasip1` (package suffix `wasm32-wasip1`, loader suffix `wasip1`). If Task 1 fails structurally, stop and amend the spec before any other task.
- The WASI build is `napi build --profile wasi --platform --target wasm32-wasip1 --js binding.js --dts binding.d.ts` and nothing else; `[profile.release]` keeps `panic = "unwind"`; `[profile.wasi]` inherits release with `panic = "abort"`.
- `engines.node` is exactly `^22.13.0 || >=23.5.0` on the root; the floor lists still say Node 22; `node_floor_versions` stays `22 24`.
- The WASI package declares `@napi-rs/wasm-runtime`, `@emnapi/core` and `@emnapi/runtime` at the exact versions in `typescript/package-lock.json`; the root declares only `@skunkwerkx/hyperuuid-wasm32-wasip1` (in `dependencies`) and the eight natives (in `optionalDependencies`), both injected at pack time and never committed. The WASI manifest carries no `os`, `cpu` or `libc`.
- `index.js` and `index.d.ts` do not change. `check_exports.py` does not change.
- Every number in a README is measured on this machine (Apple M6, Mac18,5, Node 26.13.0, Bun 1.4.4) and dated; the loss is printed. Bun's forced-WASI result is recorded, never required.
- Coverage wording: the fallback is "proven on Linux x64 (glibc and Alpine), macOS arm64, Windows x64, and s390x under emulation"; elsewhere "should load; not tested". Browsers: "Chromium on Linux, without cross-origin isolation"; Firefox and Safari "untested".
- Docs state the current state only; history goes to the CHANGELOG under the TypeScript and plumbing bullets of the current section. Install snippets keep `<version>`.
- A verification counts only when its negative control fails; each task names its control.
- Third-party actions are pinned to a commit with the version in a trailing comment.

## Review Focus

- A batch whose result cannot fit the initial guest memory (5,000,000 ids as bytes, 80 MB, against `initialMemory` 992 pages = 65 MB) must succeed under WASI, and the v7 stream must survive the grow: the next `newV7()` is greater than the batch's last id. The growth itself is established by Task 1 Step 7 (992 × 65,536 < 80,000,000, so the test cannot pass without `memory.grow`); the test is pinned as Task 1's `an 80 MB batch is whole and the stream survives it`.
- A fill past the core's 64 KiB `getrandom` chunk on WASI (`rust/src/entropy.rs` `fill_with`): `fillV7` on 10,000 slots must write 10,000 well-formed, strictly increasing v7s. Pinned in Task 1's `a fill past the WASI getrandom chunk is whole`.
- Each `worker_threads` worker instantiates its own WASI module and so its own v7 stream: a worker must produce ids, and the README must say the stream is per instance there. Pinned in Task 1's `tests/workers.test.ts` and Task 9's README sentence.
- A consumer directory with a space in its path must load the `.wasm` on Windows and macOS (the loader reads it by path). Pinned in Task 7: scenario 2 runs from `consumer two/`.
- The browser shim's realtime clock feeds `newV7()`'s default time: the page asserts `|v7UnixMillis(newV7()) - Date.now()| < 1000`. Pinned in Task 2's page.

---

### Task 1: The probe: the WASI build in `typescript/`, the suite under forced WASI, the measurements

**Files:**
- Modify: `typescript/Cargo.toml` (add `[profile.wasi]`), `typescript/package.json`, `typescript/.gitignore`, `typescript/.cargo/config.toml` (the comment), `typescript/binding.js` and `typescript/binding.d.ts` (regenerated), `typescript/package-lock.json` (by `npm install`)
- Create: `typescript/tests/workers.test.ts`
- Modify: `typescript/tests/generate.test.ts`
- Modify: this plan's **Probe results** section (bottom)

**Interfaces:**
- Produces: `npm run build:wasi` writing `typescript/hyperuuid.wasm32-wasip1.wasm` and the loaders `hyperuuid.wasip1.cjs`, `hyperuuid.wasip1.d.cts`, `hyperuuid.wasip1-browser.js`, `hyperuuid.wasip1-deferred.js`, `hyperuuid.wasip1-deferred.d.ts`, `hyperuuid.wasm32-wasip1.wasm.d.ts` (plus whatever `.workerd.mjs`/`.d.mts` files the CLI emits; the probe records the exact list, which Task 5's artifact definition and Task 6's upload glob `hyperuuid.wasm32-wasip1.* hyperuuid.wasip1*` must cover). `binding.js`'s flavour list is `['wasm32-wasip1']` and `__napiBindingTarget` is `'native' | 'wasm32-wasip1'`.

- [ ] **Step 1: The profile.** Append to `typescript/Cargo.toml`:

```toml
# The WASI build only. wasm32-wasip1's std ships no unwinder: rustc 1.99.1 refuses
# `-C panic=unwind` for it ("the crate `panic_unwind` does not have the panic strategy
# `unwind`"), so the explicit unwind above cannot apply there. Panics abort in the .wasm and
# `catch_unwind` has nothing to catch; the TypeScript README says so.
[profile.wasi]
inherits = "release"
panic = "abort"
```

- [ ] **Step 2: The manifest.** In `typescript/package.json`: `engines.node` → `"^22.13.0 || >=23.5.0"`; `napi.targets` += `"wasm32-wasip1"` (last); `scripts["build:wasi"] = "napi build --profile wasi --platform --target wasm32-wasip1 --js binding.js --dts binding.d.ts"`; `napi.wasm = { "initialMemory": <from Step 7> }` (leave out until Step 7). Then `npm install --save-dev --save-exact emnapi @emnapi/core @emnapi/runtime @napi-rs/wasm-runtime` (`emnapi` and the two `@emnapi/*` at the one version the CLI accepts: the installed `@napi-rs/cli` depends on a range of each; pick the latest that satisfies all three).

- [ ] **Step 3: Ignore the outputs.** Append to `typescript/.gitignore`: `hyperuuid.wasm32-wasip1.*`, `hyperuuid.wasip1*`, `browser-smoke/dist/`, `browser-smoke/node_modules/`, `consumer-tests/node_modules/`, `consumer-tests/work/`. In `typescript/.cargo/config.toml`, drop "and the wasm target is never built" from the header comment and say the WASI build takes its panic strategy from `[profile.wasi]` in Cargo.toml, not from here.

- [ ] **Step 4: Negative control, then the build.** `rustup target list --installed | grep wasm32-wasip1`. Run `cd typescript && npx napi build --release --target wasm32-wasip1 --js binding.js --dts binding.d.ts`; expected: FAIL with the `panic_unwind` error. Then `npm run build:wasi`; expected: the files in Interfaces exist; `git diff --stat` shows only `binding.js`, `binding.d.ts`, `package.json`, `package-lock.json`, `Cargo.toml`, `.gitignore`, `.cargo/config.toml`. Then `npm run build` (native) and `git diff -- index.js index.d.ts` is empty.

- [ ] **Step 5: The effective invocation.** `cd typescript && cargo build -v --profile wasi --target wasm32-wasip1 2>&1 | grep -o '\-C panic=[a-z]*' | sort -u` prints `-C panic=abort`; `cargo build -v --release 2>&1 | grep -o '\-C panic=[a-z]*' | sort -u` prints `-C panic=unwind` (touch `src/lib.rs` first if cargo says fresh). Record both lines in Probe results.

- [ ] **Step 6: The suite under forced WASI.** `NAPI_RS_FORCE_WASI=error npm test` passes with the `ExperimentalWarning: WASI` on stderr; `npm run typecheck`; `NAPI_RS_FORCE_WASI=error npm run test:bun`, result recorded (pass or the first failure, Bun 1.4.4). Negative control: `rm hyperuuid.wasm32-wasip1.wasm && NAPI_RS_FORCE_WASI=error npm test` fails (then rebuild). Native `npm test` and `npm run test:bun` still pass (the loader must not touch WASI: `NODE_OPTIONS=--trace-warnings npm test 2>&1 | grep -c ExperimentalWarning` prints 0).

- [ ] **Step 7: Memory and size.** `brew install wabt` if absent; `wasm-objdump -x -j Import hyperuuid.wasm32-wasip1.wasm | grep -i memory` gives the module's declared minimum pages; `stat -f %z hyperuuid.wasm32-wasip1.wasm` the size. Set `napi.wasm.initialMemory` to the declared minimum rounded up to the next multiple of 16 pages (1 MiB), rebuild with `npm run build:wasi`, rerun Step 6's Node suite. Growth probe: assert the chosen pages × 65,536 < 16,000,000, so Step 9's 16 MB batch test can only pass by growing guest memory; record both numbers. Record pages and bytes in Probe results, with `du -sk node_modules/@napi-rs/wasm-runtime node_modules/@emnapi/core node_modules/@emnapi/runtime` (the install's extra bytes).

- [ ] **Step 8: What a panic looks like.** In a scratch copy of the crate under the scratchpad (never in the tree): add `#[napi] pub fn boom() { panic!("probe") }`, build with `build:wasi`, call it from Node under `NAPI_RS_FORCE_WASI=error`, and record the error's constructor name and message, and whether a following `newV4()` on the same instance still works. Delete the scratch copy. Record in Probe results.

- [ ] **Step 9: Review-focus tests.** Add to `typescript/tests/generate.test.ts`:

```ts
test('a 16 MB batch is whole and the stream survives it', () => {
  const bytes = newV7BatchBytes(1_000_000)
  assert.equal(bytes.length, 16_000_000)
  for (const at of [0, 15_999_984]) {
    assert.equal(bytes[at + 6] >> 4, 7)
    assert.equal(bytes[at + 8] & 0xc0, 0x80)
  }
  const last = Buffer.from(bytes.subarray(15_999_984)).toString('hex')
  assert.ok(Buffer.from(newV7Bytes()).toString('hex') > last)
})

test('a fill past the WASI getrandom chunk is whole', () => {
  const buf = new Uint8Array(16 * 10_000)
  fillV7(buf)
  let previous = ''
  for (let i = 0; i < buf.length; i += 16) {
    const id = Buffer.from(buf.subarray(i, i + 16)).toString('hex')
    assert.equal(id[12], '7')
    assert.equal(buf[i + 8] & 0xc0, 0x80)
    assert.ok(id > previous)
    previous = id
  }
})
```

Create `typescript/tests/workers.test.ts`: a `Worker` created from a data URL or `eval: true` that does `require(path.join(dir, '../index.js')).newV7()` and posts it; the test asserts the posted value is a v7 string and that the main thread's `newV7()` is also one. Both tests run under native and forced WASI (`npm test`, then `NAPI_RS_FORCE_WASI=error npm test`); `npm run lint` and `npm run typecheck` pass.

- [ ] **Step 10: The bench.** `NAPI_RS_FORCE_WASI=error npm run bench` and `NAPI_RS_FORCE_WASI=error npm run bench:bun` (or "unavailable" with the first error), the native `npm run bench` and `bench:bun` the same night; paste the five rows for each into Probe results. Report the task; if the forced-WASI Node suite failed on something structural (instantiation, a door that cannot work threadless), stop here and say so.

### Task 2: The browser page

**Files:**
- Create: `typescript/browser-smoke/package.json`, `typescript/browser-smoke/index.html`, `typescript/browser-smoke/main.js`, `typescript/browser-smoke/vite.config.js`, `typescript/browser-smoke/check-dom.mjs`, `typescript/browser-smoke/check-dom.test.mjs`, `typescript/browser-smoke/package-lock.json`
- Modify: `typescript/biome.json` (`files.includes` += `"browser-smoke/*.js"`, `"browser-smoke/*.mjs"`, `"consumer-tests/**/*.cjs"`, `"consumer-tests/**/*.mjs"`)

**Interfaces:**
- Consumes: the WASI tarball from Task 5's `pack-wasi` (locally from Step 2 below).
- Produces: `npm ci && npm install --no-save <tarball> && npm run build` in `browser-smoke/` writes `dist/`; serving `dist/` and loading `index.html` leaves `<pre id="out">` containing `PASS` on the last line or `FAIL: <reason>` as its first failing line.

- [ ] **Step 1: The page.** `package.json`: `"private": true`, `"type": "module"`, devDependency `vite` pinned exact, script `build: vite build`. `vite.config.js`: `build.target: 'esnext'` (top-level await). `main.js` imports `* as binding from '@skunkwerkx/hyperuuid-wasm32-wasip1'` (the package's `browser` export condition), awaits its default export if the loader exports a promise (read `hyperuuid.wasip1-browser.js` from Task 1 to see which; record the shape in a comment), imports the five corpus files from `../../corpus/`, and ports `tests/corpus.test.ts`'s five tests one for one, assertion for assertion: `v5.json` (every vector as bytes and, where `name` is present, as a string); `sql_order.json` (both directions, string and 16-byte forms); `sql_order_refused.json` (the code `ERR_HYPERUUID_WRONG_VERSION` and the exact message); `timestamp.json` (`version` in the vector's layout, `getTimestamp` null where `unix_millis` is null, in the layout and by default for `rfc9562`; otherwise `v7UnixMillis`, `getTimestamp` and `v7Timestamp` in the layout, and by default for `rfc9562`); `inspect.json` (`version` and `isRfc` in the layout, `isRfc` false for the other fifteen versions, and `variant`, `version` and `isRfc` by default where `variant` is present). Then the generation doors: `newV4()`, `newV7()`, `newV7Batch(10)` strictly increasing, `fillV7(new Uint8Array(160))` with version 7 and the RFC variant in every slot; `Math.abs(v7UnixMillis(newV7()) - Date.now()) < 1000`; `version('x')` throws `ERR_HYPERUUID_INVALID_ID`. Each check appends a line to `#out`; the last line is `PASS` or the first failure is `FAIL: ...` and nothing after it. `typescript/browser-smoke/check-dom.mjs <dom.html>` is the one acceptance check, used locally and in CI: it extracts the text of `<pre id="out">` from the serialized DOM, decodes HTML entities, splits into lines, and exits 0 only if the last non-empty line is exactly `PASS` and no line starts with `FAIL`; otherwise it prints the lines and exits 1. Its own tests (`node --test browser-smoke/check-dom.test.mjs`, run by `npm test` in `browser-smoke/`) feed it four fixtures: a passing DOM; a `FAIL: ...` line; `PASS` followed by another output line (rejected: `PASS` must be last); a DOM with no `#out`.

- [ ] **Step 2: Local run.** After Task 5: `python3 ../HyperForge/.github/scripts/npm_package.py pack-wasi --package typescript --artifact <dir holding Task 1's outputs> --out /tmp/npm-wasi` then `cd typescript/browser-smoke && npm ci && npm install --no-save /tmp/npm-wasi/skunkwerkx-hyperuuid-wasm32-wasip1-*.tgz && npm run build`, serve `dist/` with `python3 -m http.server 8092 --bind 127.0.0.1`, and `timeout 120 "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --disable-gpu --virtual-time-budget=60000 --dump-dom http://127.0.0.1:8092/index.html > dom.html` must exit 0, then `node check-dom.mjs dom.html` must exit 0. Negative controls: edit a corpus expectation in a scratch copy of `main.js`, rebuild, `check-dom.mjs` exits 1 on a `FAIL` line; and its fixture test proves `PASS` followed by further output is rejected. Confirm the served `dist/` has no COOP/COEP headers (`curl -sI` shows none) and the page still passes. `npm run lint` from `typescript/` passes.

### Task 3: Forge: `hyper_config.py` knows the WASI platform

**Files:**
- Modify: `../HyperForge/.github/scripts/hyper_config.py` (after `NODE_PLATFORMS`; `HyperConfig.__init__`; `shell`; `main`'s json)
- Test: `../HyperForge/.github/scripts/test_hyper_config.py`

**Interfaces:**
- Produces: `hyper_config.NODE_WASI_PLATFORM = 'wasm32-wasip1'`; `cfg.node_wasi: str` (`data['node'].get('wasi', NODE_WASI_PLATFORM)`, `''` meaning none); shell line `NODE_WASI=<value>`; json key `node_wasi`.

- [ ] **Step 1: Failing test.** In `test_hyper_config.py` add `test_node_wasi_platform_defaults_and_can_be_cleared`: default `cfg.node_wasi == 'wasm32-wasip1'`; `HyperConfig({**MINIMAL, 'node': {'wasi': ''}}).node_wasi == ''`; a non-string `wasi` raises `ValueError`; the shell script contains `NODE_WASI=wasm32-wasip1`; the json has `node_wasi`.
- [ ] **Step 2: Run** `cd ../HyperForge/.github/scripts && python3 -m unittest test_hyper_config -k node_wasi -v`; expected FAIL (no attribute).
- [ ] **Step 3: Implement** the constant with a comment (napi's platformArchABI for the threadless WASI target; the package every consumer installs; empty = no WASI package), the attribute, the shell line and the json key.
- [ ] **Step 4: Run** the whole file; expected PASS.

### Task 4: Forge: ten packages in the metadata validator and the inventory

**Files:**
- Modify: `../HyperForge/.github/scripts/release_package_metadata.py` (`DEFAULTS`, `npm_names`, new `npm_wasi_name`, `npm_all_names`, `npm_tarball`, `validate_package_metadata`), `../HyperForge/.github/scripts/release_inventory.py:149-152`
- Test: `../HyperForge/.github/scripts/test_package_inventory_metadata.py` (`write_npm`, the rejection cases), `../HyperForge/.github/scripts/test_release_inventory.py:399-420`

**Interfaces:**
- Produces: `npm_wasi_name(crate, wasi) -> str | None` (`f'{NPM_SCOPE}/{crate}-{wasi}'` or `None` for `''`); `npm_all_names(crate, platforms, wasi) -> list[str]` (root, natives, then the WASI name if any); `WASI_RUNTIME = ('@napi-rs/wasm-runtime', '@emnapi/core', '@emnapi/runtime')`; `npm_tarball(path, crate, version, platforms, wasi='', wasi_runtime=None) -> str` with the checks below; `DEFAULTS.node_wasi = hyper_config.NODE_WASI_PLATFORM`; `validate_package_metadata` passes `getattr(cfg, 'node_wasi', '')` and expects `npm_all_names`.
- `npm_tarball` checks: name in `npm_all_names`, version, filename (unchanged); root: `optionalDependencies == {native: version}`, `dependencies == {wasi_name: version}` when `wasi` else absent, no `.node` and no `.wasm` entry; native: exactly one `.node`, no `.wasm`; WASI: no `.node`, exactly one `.wasm` named `package/{crate}.{wasi}.wasm`, `package/{crate}.{wasi.removeprefix('wasm32-')}.cjs` present, none of `os`/`cpu`/`libc` in the manifest, `dependencies` keys == `set(WASI_RUNTIME)` with every value an exact SemVer, prerelease and build metadata allowed (`^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$`; `EXACT_SEMVER` in the module), so ranges (`~`, `^`, `>=`, `x`, `*`), tags (`latest`, `next`) and URLs are refused, equal to `wasi_runtime` when given.

- [ ] **Step 1: Failing tests.** In `test_package_inventory_metadata.py`: `NODE_WASI = 'wasm32-wasip1'`; `write_npm(..., wasi=True, wasi_dependencies=None)` adds the root's `dependencies` and a tenth tarball (`package/package.json` with `dependencies={'@napi-rs/wasm-runtime': '1.0.0', '@emnapi/core': '1.5.0', '@emnapi/runtime': '1.5.0'}`, `package/hypercast.wasm32-wasip1.wasm`, `package/hypercast.wasip1.cjs`); docstring says ten. Extend `test_rejects_npm_tarballs_with_stale_or_wrong_identity` with cases: root missing `dependencies`; root `dependencies` at another version; WASI manifest with `cpu: ['wasm32']`, with `os`, with `libc`; WASI `dependencies` with a `~` range, with `^`, with `latest`, with a `git+https` URL, with a fourth key, missing one; a WASI manifest pinning `2.0.0-alpha.4` passes (the CLI itself accepts prerelease emnapi); WASI tarball with a `.node`; two `.wasm`; native tarball carrying a `.wasm`; root carrying a `.node`. Add `test_npm_tarball_compares_wasi_runtime_when_given` (equal passes, one version off raises). In `test_release_inventory.py` the count becomes 10 and `broken` gains `'missing wasi package'`.
- [ ] **Step 2: Run** `python3 -m unittest test_package_inventory_metadata test_release_inventory`; expected FAIL on the new cases.
- [ ] **Step 3: Implement** per Interfaces; `release_inventory.py` builds its expected set from `npm_all_names(crate, cfg.node_platforms.values(), cfg.node_wasi)`; the error text says "root, platform and wasi package inventory".
- [ ] **Step 4: Run** the two files and `test_release_guard`, `test_release_provenance` (they import the module); expected PASS.

### Task 5: Forge: `npm_package.py`, the one pack body

**Files:**
- Create: `../HyperForge/.github/scripts/npm_package.py`
- Test: `../HyperForge/.github/scripts/test_npm_package.py`

**Interfaces:**
- Consumes: `hyper_config.load(root)` (`crate`, `node_platforms`, `node_wasi`), `release_package_metadata.npm_tarball`, `npm_wasi_name`, `WASI_RUNTIME`.
- Produces, CLI: `npm_package.py pack --package typescript --artifacts <dir> --out <dir> [--root .] [--allow-missing-natives]` and `npm_package.py pack-wasi --package typescript --artifact <dir> --out <dir> [--root .]`. `--artifacts` holds `node-<rid>/<crate>.<suffix>.node` for every RID in `node_platforms` and `node-<wasi>/` with the WASI build's files; `--artifact` is that one directory. Exit 0 and the tarball paths on stdout, or a message on stderr and exit 1. `--allow-missing-natives` is for a developer machine (one native built): absent natives are skipped, the root still pins all eight, and the output is never for release (the action never passes it; Task 6's test asserts that), and it cannot stand in for the eight published natives in the lockfile scenario (Task 7's guard).
- Produces, functions: `lockfile_versions(package_dir: Path, names) -> dict[str, str]` (from `package-lock.json`'s `packages["node_modules/<name>"].version`, raising on a missing one); `stage(package_dir, work) -> None` (`cp -R`, `npm ci --ignore-scripts`; raises if the committed manifest has `dependencies` or `optionalDependencies`, or `napi.packageName != '@skunkwerkx/<crate>'`); `stage_wasi_sources(work, artifact_dir) -> None` (copies the artifact's files into `work/`, where `napi create-npm-dirs` looks for WASI loaders); `create_npm_dirs(work) -> None`; `fill_wasi_dir(work, artifact_dir, crate, wasi) -> Path` (copies the `.wasm` and every artifact file into `npm/<wasi>/`; raises unless every entry of the generated manifest's `files` exists there); `pin_wasi_dependencies(wasi_dir, versions) -> None` (rewrites `dependencies` to exact versions; raises if a key is not in `WASI_RUNTIME`); `check_wasi_manifest(wasi_dir) -> None` (no `os`/`cpu`/`libc`; `engines.node == '^22.13.0 || >=23.5.0'`); `fill_native_dir(work, artifact_dir, crate, suffix) -> Path`; `pack_dir(directory, out) -> Path` (`npm pack --ignore-scripts --pack-destination`); `set_root_dependencies(work, scope_name, version, natives, wasi) -> None` (`npm pkg set`); `pack_wasi(package_dir, artifact_dir, out, cfg, work=None) -> Path`; `pack(package_dir, artifacts_dir, out, cfg, work=None, allow_missing_natives=False) -> list[Path]`; `check_tarballs(paths, crate, version, cfg, versions, allow_missing_natives=False) -> None` (every path through `npm_tarball(..., wasi_runtime=versions)`; the set of names equals `npm_all_names`, minus skipped natives when allowed).

- [ ] **Step 1: Failing tests** in `test_npm_package.py` over fixtures written under `tempfile`: `lockfile_versions` reads three versions and raises on a missing name; `pin_wasi_dependencies` turns `~1.2.3` into `1.2.3` and raises on an unknown key; `check_wasi_manifest` raises on each of `cpu`, `os`, `libc` and on a different `engines.node`; `fill_wasi_dir` raises when a `files` entry is absent; `stage` raises on a committed `dependencies`, on `optionalDependencies`, on a wrong `napi.packageName` (mock `npm ci` out: pass a `runner` callable, default `subprocess.run`); `check_tarballs` accepts ten well-formed fixtures built with `test_package_inventory_metadata.write_npm(wasi=True)` and rejects a missing WASI tarball, a missing native (unless allowed), an extra tarball. A CLI test runs `pack-wasi` with a `runner` that records the commands and asserts the order: `npm ci --ignore-scripts`, `napi create-npm-dirs`, `npm pack` of `npm/wasm32-wasip1` only, and no `npm pkg set`.
- [ ] **Step 2: Run** `python3 -m unittest test_npm_package`; expected FAIL (module missing).
- [ ] **Step 3: Implement** the module; docstring: what the two operations are, why the WASI dependencies are pinned from the lockfile and why `create-npm-dirs` still reaches the registry.
- [ ] **Step 4: Run** the file; expected PASS. Then the real thing from the HyperUuid tree: `python3 ../HyperForge/.github/scripts/npm_package.py pack-wasi --package typescript --artifact <dir with Task 1's outputs> --out /tmp/npm-wasi` succeeds; `tar tzf` of the result lists one `.wasm`, the loaders and no `.node`; `python3 -c` with `npm_package(...)` shows the pinned exact versions and no `cpu`. Then `pack --allow-missing-natives` with `node-osx-arm64/hyperuuid.darwin-arm64.node` and the WASI dir: three tarballs; `shasum typescript/package.json typescript/package-lock.json` before and after are identical (the copy was rewritten, not the checkout; CI's `git diff --exit-code` is the same check where the tree is committed). Negative control: run `pack` without `--allow-missing-natives`; expected: exit 1 naming the first missing RID.

### Task 6: Forge: `pack-npm` calls the script; `hyper-build-wasm.yml` builds, tests, packs, attests

**Files:**
- Modify: `../HyperForge/.github/actions/pack-npm/action.yml` (description "ten"; the two pack/check steps become one `python3 "$FORGE_SCRIPTS/npm_package.py" pack --package typescript --artifacts artifacts/node --out npm-packages` plus `grep -q` for both maps and `git diff --exit-code -- typescript`)
- Modify: `../HyperForge/.github/workflows/hyper-build-wasm.yml` (inputs `node_wasi` boolean default false, `node_version` string default `"26"`; job `build-node-wasi` placed first after `jobs:`, before `build-wasm`)
- Modify: `../HyperForge/.github/scripts/test_forge_actions.py:296-330`, `../HyperForge/.github/scripts/test_forge_producers.py` (new test on the wasm workflow), `../HyperForge/docs/release-runbook.md:20-24,171`, `../HyperForge/README.md:113` (pack-npm's parenthesis)

**Interfaces:**
- Produces: artifact `node-wasm32-wasip1` (a directory of Task 1's output files), attested when `inputs.mode != 'pr'` and not a fork PR, exactly the archive's condition at `hyper-build-wasm.yml:218-221`.
- `build-node-wasi` steps, in order: checkout; validate Forge sha; Forge checkout to `.hyperforge`; `dtolnay/rust-toolchain` stable with `targets: wasm32-wasip1`; `setup-node` with `inputs.node_version`; `npm ci` and `npm run build:wasi` in `typescript/`; `git diff --exit-code -- typescript/index.js typescript/index.d.ts typescript/binding.js typescript/binding.d.ts`; `ls -l typescript/hyperuuid.wasm32-wasip1.wasm`; `NAPI_RS_FORCE_WASI=error npm test` and `npm run typecheck`; collect `mkdir node-wasm32-wasip1 && cp typescript/hyperuuid.wasm32-wasip1.* typescript/hyperuuid.wasip1* node-wasm32-wasip1/`; `python3 .hyperforge/.github/scripts/npm_package.py pack-wasi --package typescript --artifact node-wasm32-wasip1 --out npm-wasi`; browser page: `cd typescript/browser-smoke && npm ci && npm install --no-save ../../npm-wasi/*.tgz && npm run build`, `python3 -m http.server 8092 --bind 127.0.0.1` over `dist/` in the background, `timeout 120 google-chrome --headless=new --no-sandbox --disable-gpu --virtual-time-budget=60000 --dump-dom http://127.0.0.1:8092/index.html > dom.html` under `set -euo pipefail` (a non-zero Chrome exit or the timeout fails the step; no `|| true`), then `node typescript/browser-smoke/check-dom.mjs dom.html` (the same assertion as the local run: `#out`'s last non-empty line is `PASS`, no `FAIL` line; the Ruby leg's Chrome invocation in `suite-ruby-wasm.sh:120-136`, with the exit code enforced and the DOM check made exact; the spec names the runner's Chrome, so no browser download); attest `node-wasm32-wasip1/*`; upload `node-wasm32-wasip1` with `if-no-files-found: error`.

- [ ] **Step 1: Failing tests.** `test_forge_actions.py`: rename the pack-npm test to `..._packs_ten_tarballs_through_npm_package_py`; assert `npm_package.py" pack --package typescript --artifacts artifacts/node` in the runs, `NotIn('--allow-missing-natives')`, `NotIn('npm pkg set')` (the script does it), the `grep -q` for `dependencies` and `optionalDependencies`, and the rest unchanged. `test_forge_producers.py`: `test_node_wasi_job_builds_tests_packs_and_attests_first`: inputs `node_wasi` (boolean, default False) and `node_version` (default `"26"`); `list(self.wasm['jobs'])[0] == 'build-node-wasi'`; `job['if'] == 'inputs.node_wasi'`; needles in the joined runs: `npm run build:wasi`, `git diff --exit-code -- typescript/index.js typescript/index.d.ts typescript/binding.js typescript/binding.d.ts`, `NAPI_RS_FORCE_WASI=error npm test`, `npm run typecheck`, `npm_package.py pack-wasi`, `browser-smoke`, `timeout 120 google-chrome`, `--dump-dom`, `check-dom.mjs dom.html`; no `playwright`, no `|| true` in that step; the attest step's `if` equals the archive's; upload name `node-wasm32-wasip1`; `rust-toolchain` with `targets: wasm32-wasip1`; the Forge checkout step present.
- [ ] **Step 2: Run** `python3 -m unittest test_forge_actions test_forge_producers`; expected FAIL.
- [ ] **Step 3: Implement** the action and the job; comment at the job: why first (newest code, fail fast), why no wasi-sdk, why the browser import is by package name, why no COOP/COEP.
- [ ] **Step 4: Run** the whole Forge suite: `cd ../HyperForge/.github/scripts && python3 -m unittest discover -s . -p 'test_*.py'`; expected PASS. `actionlint .github/workflows/hyper-build-wasm.yml` clean (install via `brew install actionlint` if absent).
- [ ] **Step 5: Docs.** Runbook lines 20-24: ten names, the tenth `@skunkwerkx/<crate>-wasm32-wasip1`; line 171 "each of the ten". README line 113: "(the ten npm tarballs, the root's `dependencies` and `optionalDependencies` injected at pack time on a copy, never committed)". Report the Forge PR as ready for the owner.

### Task 7: Consumer-test scripts, proven locally

**Files:**
- Create: `typescript/consumer-tests/package.json` (devDependencies pinned exact: `verdaccio`, `esbuild`, `webpack`, `webpack-cli`; `"private": true`), `typescript/consumer-tests/package-lock.json`, `typescript/consumer-tests/verdaccio.yaml`, `typescript/consumer-tests/smoke.cjs`, `typescript/consumer-tests/bundled.cjs`, `typescript/consumer-tests/container/consumer.sh`, `typescript/consumer-tests/run.sh`, `typescript/consumer-tests/bundlers/esbuild.sh`, `typescript/consumer-tests/bundlers/webpack.config.cjs`, `typescript/consumer-tests/README.md`
- Create (only if Step 4 finds npm repairs the lockfile): `typescript/consumer-tests/fixtures/mac-lockfile/package.json`, `.../package-lock.json`
- Modify: `.github/dependabot.yml:85-86` (directories += `/typescript/browser-smoke`, `/typescript/consumer-tests`)

**Interfaces:**
- `smoke.cjs`: `const api = require('hyperuuid')`, then `api.isAvailable()` first. When `false`: prints `available=false target=none`, calls `api.newV4()`, asserts the thrown error's `code === 'ERR_HYPERUUID_UNAVAILABLE'` (exit 1 otherwise), prints its `cause` chain one per line as `cause: <constructor>: <message>` and exits 0. When `true`: prints `available=true target=<'native'|'wasm32-wasip1'> version=<nativeVersion()>`, where the target is read only now, from `require(path.join(path.dirname(require.resolve('hyperuuid')), 'binding.js')).__napiBindingTarget` (`index.js` is what tolerates a failing `binding.js`; the smoke never requires it when nothing loaded); generates a v4 and a v7, checks `v5.json`'s first vector and `sql_order.json`'s first vector from `HYPERUUID_CORPUS` (a directory) and exits 1 on any mismatch.
- `bundled.cjs`: the public-API half of `smoke.cjs` only (`isAvailable() === true`, the generation and the two vectors) with no `binding.js` inspection, so a self-contained bundle is judged on what it does; the backend is implied, since the scenario runs under `NAPI_RS_FORCE_WASI=error`, under which the loader never consults the native addon.
- `container/consumer.sh`: POSIX `sh` (the Alpine images carry no bash) taking `REGISTRY`, `VERSION`, `SCENARIO` (`omit-optional`, `floor` or `s390x`) and `HYPERUUID_CORPUS`. Its body, so that `require('hyperuuid')` resolves from the install and not from `/ct`:

```sh
cd /work
cp /ct/smoke.cjs ./smoke.cjs
printf 'registry=%s\n' "$REGISTRY" > .npmrc
[ "$SCENARIO" = floor ] && printf 'engine-strict=true\n' >> .npmrc
case "$SCENARIO" in s390x) omit="" ;; *) omit="--omit=optional" ;; esac
npm install $omit "hyperuuid@$VERSION" --cache /work/cache
node ./smoke.cjs > result.txt 2>&1 || { cat result.txt; exit 1; }
cat result.txt
grep -q 'target=wasm32-wasip1' result.txt
```

  (No pipe into `tee`: under `sh` it would hide Node's exit status and let the grep pass over a failed assertion.)

  `floor` and `omit-optional` install with `--omit=optional` (the spec's scenario 2 inside the image); `s390x` installs plainly, since the point is that no native package fits there. Containers see only `consumer-tests/container/` and `smoke.cjs` (mounted read-only at `/ct`), the corpus read-only at `/corpus`, and an empty `/work` (a tmpfs), never the host's `node_modules`; they reach the host's Verdaccio over `--network host` at `http://127.0.0.1:4873` on Linux, or `http://host.docker.internal:4873` on Docker Desktop (`REGISTRY_HOST`, with Verdaccio listening on `0.0.0.0`).
- `run.sh <tarballs-dir> <scenario...>` (bash on the host; Git Bash on Windows; it is the registry orchestration and never runs inside a container): starts Verdaccio on port 4873 with `verdaccio.yaml` (uplink `npmjs` for `**`, none for `hyperuuid` and `@skunkwerkx/*`; `publish: $anonymous`, `access: $all`; storage under `consumer-tests/work/storage`; if the installed Verdaccio refuses an anonymous publish, `auth.htpasswd.max_users: 1` and one `npm adduser` against it, with the token in the consumer `.npmrc`, is the fallback, recorded in `consumer-tests/README.md`), publishes the tarballs in the order the scenario needs, prints the host `npm --version`, then runs the named scenarios (`negative-unpublished`, `plain`, `omit-optional`, `absent-native`, `forced-wasi`, `negative-deleted-package`, `negative-deleted-wasm`, `mac-lockfile`, `floor`, `s390x`, `bundlers`, or `all`), each in a fresh directory under `consumer-tests/work/<scenario>/` with an `.npmrc` naming the registry and a fresh `--cache`, asserting the printed `target=` and the stderr rule, and exits non-zero on the first failure. Scenario details are the spec's 1 to 9; `omit-optional` runs from `work/consumer two/` (the space). `negative-unpublished` runs before the WASI tarball is published; the rest after. Bun runs (`forced-wasi` with `bun smoke.cjs`) print `bun: <result>` and never fail the script. `floor` and `s390x` run `container/consumer.sh` in `node:22.13.0-alpine` (then `node:22-alpine`, `node:24-alpine`) and in `--platform linux/s390x node:24-alpine`; the runtime image needs nothing but Node and npm. `mac-lockfile` uses its own npm, `LOCKFILE_NPM=11.20.0` pinned at the top of `run.sh` with a comment (the reproduction's tool, re-verified by hand when moved, not a floor), installed with `npm install -g npm@$LOCKFILE_NPM --prefix work/npm-lockfile` and asserted equal to `"$(work/npm-lockfile/bin/npm --version)"` before the scenario runs; it refuses to run unless all eight native names resolve in the registry (`npm view` each), since a partial pack would make the lockfile assertion pass for the wrong reason.
- `bundlers`: esbuild bundles `bundled.cjs` (`--bundle --platform=node --outfile=bundle.cjs`); the bundle alone is copied to a deployment directory from `mktemp -d` outside the repository (no ancestor holds a `node_modules`; `NODE_PATH` unset as well) and run there under `NAPI_RS_FORCE_WASI=error` with `HYPERUUID_CORPUS` set; on failure, rebuild with `--external:hyperuuid`, write a `package.json` in a fresh `mktemp -d` declaring `hyperuuid@<version>`, `npm install --omit=dev` there from the registry, rerun; the configuration that passed is printed as `bundler: esbuild <plain|external>`. Webpack the same with `externals: { hyperuuid: 'commonjs hyperuuid' }` for the external form. Either bundler ending with no passing form fails the scenario.

- [ ] **Step 1: Write** `smoke.cjs`, `verdaccio.yaml`, `run.sh` and the bundler files; `npm ci` in `consumer-tests/`.
- [ ] **Step 2: Local tarballs.** From the repo root: `python3 ../HyperForge/.github/scripts/npm_package.py pack --package typescript --artifacts /tmp/node-artifacts --out /tmp/npm-local --allow-missing-natives`, where `/tmp/node-artifacts/node-osx-arm64/hyperuuid.darwin-arm64.node` is Task 1's native build and `/tmp/node-artifacts/node-wasm32-wasip1/` its WASI files.
- [ ] **Step 3: Run** `typescript/consumer-tests/run.sh /tmp/npm-local negative-unpublished plain omit-optional absent-native forced-wasi negative-deleted-package negative-deleted-wasm bundlers` on this Mac. Expected: each prints its assertion and `OK`; `plain` shows `target=native` and empty stderr; `omit-optional` shows `target=wasm32-wasip1` and only the `ExperimentalWarning` lines; the three negative controls end as the spec says; `bundlers` prints which form passed. Negative control: run `plain` with `HYPERUUID_CORPUS` pointing at an empty directory; expected: smoke exits 1 and `run.sh` fails.
- [ ] **Step 4: The lockfile scenario.** It cannot run against the partial local pack (the guard above refuses), so its first run is Task 8's Linux leg in CI, where all eight natives exist. Locally, prove the guard: `run.sh /tmp/npm-local mac-lockfile` exits 1 naming the missing natives. Read the CI log's lockfile assertion: if npm 11.20.0 writes the 4828 shape (`darwin-arm64` only), the scenario stands; if it repairs the lockfile, the assertion fails loudly as designed and a follow-up adds `fixtures/mac-lockfile/` by hand (a `package.json` depending on `hyperuuid@<version>` and a `package-lock.json` whose `packages` list the root, the WASI package, its runtime and only `@skunkwerkx/hyperuuid-darwin-arm64`), makes `mac-lockfile` continue from that fixture after reporting the repair, and says so in `consumer-tests/README.md`. The version in the fixture is substituted from `typescript/package.json` at run time on a copy, never committed as a concrete release.
- [ ] **Step 5: Floor and s390x, locally.** With Verdaccio on `0.0.0.0:4873` and `REGISTRY_HOST=host.docker.internal`: `run.sh /tmp/npm-local floor` runs `container/consumer.sh` in `node:22.13.0-alpine` and passes (`engine-strict=true` inside), then the two floating images; `run.sh /tmp/npm-local s390x` runs it in `--platform linux/s390x node:24-alpine` under Docker Desktop's QEMU and shows `target=wasm32-wasip1`. Negative control for `floor`: `FLOOR_IMAGE=node:22.12.0-alpine run.sh /tmp/npm-local floor` must fail the install with `EBADENGINE`.
- [ ] **Step 6: Lint.** `npm run lint` in `typescript/` (biome now includes `consumer-tests/**/*.cjs`); `shellcheck typescript/consumer-tests/run.sh typescript/consumer-tests/bundlers/esbuild.sh typescript/consumer-tests/container/consumer.sh` clean (the last with `--shell=sh`). `consumer-tests/README.md` says what the scenarios are and how to run them locally (the two commands above).

### Task 8: ci.yml: `test-npm-package`, `build-wasm`'s input, the contract tests

**Files:**
- Modify: `.github/workflows/ci.yml` (`build-wasm` `with:` += `node_wasi: true` with a comment; new job `test-npm-package` after `test-nuget-package`; `request-staging` `needs` += `test-npm-package`)
- Modify: `.github/scripts/tests/test_workflow_contracts.py` (new test; `test_nuget_package_job_proves_the_packed_layout` unchanged)

**Interfaces:**
- `test-npm-package`: `needs: [build-native, build-wasm]`; `strategy.fail-fast: false`, `matrix.os: [ubuntu-26.04, macos-26, windows-2025]`; `runs-on: ${{ matrix.os }}`; `timeout-minutes: 45`; no `if:` (runs on pull requests). Steps: checkout; Forge checkout (`repository: SkunkWerkx/.github`, `ref: master`, `path: .hyperforge`, `persist-credentials: false`); `setup-node` `node-version: "26"`; `oven-sh/setup-bun` (pinned as in the Forge, `bun-version: latest`); download `node-*` into `artifacts/node` (`merge-multiple: false`); the forced-WASI suite: `cd typescript && npm ci && cp ../artifacts/node/node-wasm32-wasip1/* . && NAPI_RS_FORCE_WASI=error npm test && npm run typecheck`, then `NAPI_RS_FORCE_WASI=error npm run test:bun` with `continue-on-error: true` and its outcome appended to `$GITHUB_STEP_SUMMARY`; pack: `python3 .hyperforge/.github/scripts/npm_package.py pack --package typescript --artifacts artifacts/node --out npm-packages`; `cd typescript/consumer-tests && npm ci`; Linux: `docker/setup-qemu-action` (pinned, `platforms: s390x`) then `bash run.sh "$GITHUB_WORKSPACE/npm-packages" all`; macOS and Windows: `bash run.sh "$GITHUB_WORKSPACE/npm-packages" negative-unpublished plain omit-optional absent-native forced-wasi negative-deleted-package negative-deleted-wasm bundlers` (`shell: bash` on every step).

- [ ] **Step 1: Failing test.** `test_npm_package_job_proves_the_fallback_in_consumers`: header has `needs: [build-native, build-wasm]`, the three OSes, no `if:`; needles in the job: `npm_package.py" pack`, `NAPI_RS_FORCE_WASI=error npm test`, `node-wasm32-wasip1`, `run.sh`, `setup-qemu-action`, `path: .hyperforge`; `request-staging`'s needs include `test-npm-package`; `build-wasm`'s `with:` has `node_wasi: true`. Also assert `test_release_mode_ci_on_master_requests_staging_of_its_own_run` still holds (it already requires every job in `needs`).
- [ ] **Step 2: Run** `python3 -m unittest discover -s .github/scripts/tests`; expected FAIL on the new test.
- [ ] **Step 3: Implement** the job and the two edits; comment on the job: what it proves, why three OSes, why Bun does not fail it, why the Forge is checked out at master here (as `release.yml` does).
- [ ] **Step 4: Run** the contract tests (including `ForgeInterfaceTests`, which sees `node_wasi` in the Forge checkout) and `actionlint .github/workflows/ci.yml`; expected PASS and clean.

### Task 9: Documentation and measurements

**Files:**
- Modify: `typescript/README.md`, `README.md:164,176,186-188,196-197,239`, `docs/platforms.md:43`, `docs/benchmarks.md:32,127-140`, `CHANGELOG.md:60-80`, `.github/RELEASING.md:37-42`, `CLAUDE.md` (npm list at 58-63, Node floor at 64-67, TypeScript local loop, open follow-ups "nine"), `ROADMAP.md:18,24,77`

- [ ] **Step 1: TypeScript README.** Install: ten packages, the tenth `@skunkwerkx/hyperuuid-wasm32-wasip1` as a dependency of the root, "the WebAssembly fallback" row in the platform table ("everything else: proven on Linux x64 glibc and Alpine, macOS arm64, Windows x64, s390x under emulation; elsewhere it should load; not tested"); the floor sentence "Node 22.13 or newer (`^22.13.0 || >=23.5.0`), or Bun"; replace the "no WebAssembly fallback" sentence. Rename "When the addon is missing" to "When no native addon fits": the fallback, the `ExperimentalWarning` line verbatim and that it is Node's, `NAPI_RS_FORCE_WASI=error` to force it, the Bun result from Probe results (version and outcome), the bundler rule from Task 7's passing form, the browser statement (Chromium on Linux, no cross-origin isolation, Firefox and Safari untested), what `--omit=optional` now does, the per-instance v7 stream under `worker_threads`, and the panic paragraph: on the WASI path panics abort (the observed error from Probe results), `catch_unwind` has nothing to catch, the core is proven no-panic for its own code and nothing else is claimed. Update the "Every door catches a Rust panic" sentence in Errors to say "on the native addon". Benchmarks: a WebAssembly column for Node and Bun (or "unavailable") from Probe results, the `.wasm` size and the install's extra bytes, dated. Local loop gains `npm run build:wasi` and `NAPI_RS_FORCE_WASI=error npm test`, and `consumer-tests/README.md`'s commands. Verify every snippet by running it.
- [ ] **Step 2: Root docs.** README: 188 "TypeScript and R are not built" → TypeScript has a fallback package tested in Chromium, R is not built; 196 "nine" → "ten"; 239 tree line mentions the fallback package; 176 unchanged. `docs/platforms.md:43` TypeScript row: both cells per the coverage wording. `docs/benchmarks.md:32` ("Neither has a fallback" → TypeScript's is WebAssembly, below) and its TypeScript table gains the WebAssembly column. CHANGELOG TypeScript bullet: replace "No WebAssembly fallback yet." with the fallback sentence (tenth package, where proven, Chromium); plumbing bullet "nine" → "ten (... the eight platform packages and the WebAssembly package, whose runtime dependencies are pinned from the lockfile)". RELEASING.md 37-42: "ten names". CLAUDE.md: npm list says ten and adds the WASI package's three runtime dependencies as a list that moves together (pinned in `package-lock.json`, moved by Dependabot with `@napi-rs/cli`); the Node floor entry gains the range; the TypeScript local-loop paragraph gains `npm run build:wasi`, `NAPI_RS_FORCE_WASI=error npm test` and the consumer tests; open follow-ups "nine attested npm tarballs"/"all nine names" → ten. ROADMAP: line 18 cell "npm: root package + 8 platform packages + the WebAssembly package"; line 24 drop "There is no wasm fallback yet..." for the fallback; item 2 `[x]` done 2026-10-31 with one sentence and the spec path.
- [ ] **Step 3: Check.** `grep -rn "nine npm\|nine names\|No WebAssembly fallback\|no wasm fallback\|not built" README.md CLAUDE.md CHANGELOG.md ROADMAP.md docs typescript/README.md .github/RELEASING.md` shows only intended hits (R's "not built"). `npm run lint` in `typescript/` passes.

### Task 10: Final verification and the report

- [ ] **Step 1: HyperUuid.** From `typescript/`: `npm ci && npm run build && npm test && npm run test:bun && npm run typecheck && npm run lint`, then `npm run build:wasi && NAPI_RS_FORCE_WASI=error npm test`; `git diff --exit-code -- index.js index.d.ts` empty. `python3 -m unittest discover -s .github/scripts/tests` passes. `python3 .github/scripts/check_exports.py` (or its test) passes unchanged.
- [ ] **Step 2: Forge.** `cd ../HyperForge/.github/scripts && python3 -m unittest discover -s . -p 'test_*.py'` passes; `actionlint` on the two workflows clean.
- [ ] **Step 3: Report** to the owner: the two trees' `git status --short`; what ran locally and passed; what only CI can prove (the three-OS consumer matrix, s390x on the runner, the Linux Chromium leg, provenance verification in `pack-npm`, the attestations); the Probe results; the owner's steps (Forge PR, then HyperUuid PR; the tenth `0.0.1` placeholder and trusted-publisher registration before 1.1.0).

## Probe results (filled by Task 1, 2026-10-31, Apple M6 Mac18,5, rustc 1.99.1, @napi-rs/cli 3.10.9, emnapi 2.0.0-beta.2, @napi-rs/wasm-runtime 1.2.6)

| Measurement | Value |
| --- | --- |
| WASI build rustc line's panic flag (`--profile wasi`) | `-C panic=abort` (verbose cargo log `probe-wasi-verbose.log`) |
| Native build rustc line's panic flag | none: cargo emits `-C panic` only for abort, so `panic = "unwind"` is rustc's default, which on native is unwind |
| Release-profile WASI build (negative control) | builds, with no `-C panic` flag on the rustc line (the target default, abort, applies); the raw-rustc `panic_unwind` error in the spec is never reached through cargo. The `wasi` profile is kept for the explicit, verifiable flag |
| Files `build:wasi` wrote | `hyperuuid.wasm32-wasip1.wasm`, `hyperuuid.wasm32-wasip1.debug.wasm`, `hyperuuid.wasip1.cjs`, `hyperuuid.wasip1.d.cts`, `hyperuuid.wasip1-browser.js`, `hyperuuid.wasip1-deferred.js`, `hyperuuid.wasip1-deferred.d.ts`, plus `browser.js` in the package root (gitignored, not in `files`). The loader falls back to the `.debug.wasm` when the `.wasm` is missing, so the pack must exclude the debug module |
| `build:wasi` needs `--platform` | without it the CLI treats `binding.js` as a stale managed file and deletes it; with it both builds write a byte-identical loader (flavours `['wasm32-wasip1']`) |
| `.wasm` size (bytes) | 173,445 (release+lto+strip; the debug module 173,502) |
| Declared minimum memory (pages) → `napi.wasm.initialMemory` | 977 pages imported (`env.memory`, max 65,536): napi-build links `-zstack-size=64000000`, not configurable → `initialMemory: 992` (62 MiB; napi's default was 4000 = 250 MiB). Growth probe: 992 × 65,536 = 65,011,712 < 80,000,000, so the 80 MB batch test passes only through `memory.grow` |
| Install's extra bytes (runtime packages, `du -sk`) | `@napi-rs/wasm-runtime` 764 KiB, `@emnapi/core` 964 KiB, `@emnapi/runtime` 324 KiB, `tslib` 124 KiB: 2,176 KiB on disk |
| Forced-WASI Node suite (Node 26.13.0) | 60/60 pass after the `fillV7` fix (before it, 3 `fillV7` tests failed: emnapi hands Rust a guest-memory copy of the caller's view; `Uint8Array::sync(env)` copies it back) |
| Forced-WASI Bun 1.4.4 suite | fails to initialise: `wasi.initialize is not a function` (Bun's `node:wasi` has no reactor `initialize`); the loader then reports `WASI binding not found` |
| A panic from JavaScript | `RuntimeError: unreachable` thrown from the call, after Rust's panic message on stderr; `newV4()` and `newV7()` on the same instance returned ids afterwards (observed once; the instance's state is not claimed sound). Natively the same panic is napi's `Error: probe` via `catch_unwind` |
| Bench, forced WASI, Node 26.13.0 | `newV4()` 1.50 µs (`crypto.randomUUID()` 67.2 ns, 22.3x faster); `newV7()` 1.52 µs (`uuid` `v7()` 378 ns, 4.0x faster); `newV7Batch(1000)` 611 µs (1000x `newV7()` 1.53 ms); `newV7BatchBytes(1000)` 40.4 µs; `v7UnixMillis(id)` 890 ns |
| Bench, forced WASI, Bun 1.4.4 | unavailable: `wasi.initialize is not a function` |
| Bench, native, same session, Node 26.13.0 | `newV4()` 250 ns (`crypto.randomUUID()` 66.0 ns, 3.8x faster); `newV7()` 171 ns (`uuid` `v7()` 350 ns, 2.0x faster for us); `newV7Batch(1000)` 46.3 µs (1000x 157 µs); `newV7BatchBytes(1000)` 3.39 µs; `v7UnixMillis(id)` 113 ns |
| Bench, native, same session, Bun 1.4.4 | `newV4()` 278 ns (`crypto.randomUUID()` 40.3 ns, 6.9x faster); `newV7()` 173 ns (`uuid` `v7()` 134 ns, 1.3x faster than us); `newV7Batch(1000)` 28.9 µs (1000x 177 µs); `newV7BatchBytes(1000)` 2.93 µs; `v7UnixMillis(id)` 166 ns |
