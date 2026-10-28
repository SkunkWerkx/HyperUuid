# Python sdist and the PHP 8.3 floor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** PyPI gains `hyperuuid-<version>.tar.gz`, built, install-proven at exactly `rust-version` and attested beside the 17 wheels; the core's suite runs on big-endian s390x under QEMU; and the PHP floor moves from 8.2 to 8.3.

**Architecture:**
- **Part B, PHP (Task 1):** repo only. Composer floors, the lock hash, the docs and an explicit `php_floor_version: "8.3"` in `ci.yml`. The Forge default stays 8.2 for the siblings.
- **Part A, the sdist in this repo (Task 2):** maturin already writes a self-contained sdist. One `include` makes it build on musl too, and a local proof covers it.
- **Part A in the Forge (Tasks 3-5):** one PR, merged first, every new input off by default:
  - the `sdist` key and the release-side checks;
  - the `build-sdist` and `sdist-fallthrough` jobs;
  - the `test-big-endian` job.
- **Part A wiring and docs (Tasks 6-8):** `hyper.json`, `ci.yml` and the docs. The fall-through wording is written last, after a CI run of the probe leg.

**Tech Stack:** maturin 1.15 (`maturin-action` v1.51.0), PyO3 0.30, twine, Docker (`quay.io/pypa/manylinux2014_x86_64`, `python:3.11-alpine`, `php:8.x-cli-alpine`), qemu-user, Composer, GitHub Actions reusable workflows and composite actions in the Forge.

**Spec:** `docs/superpowers/specs/2026-10-28-python-sdist-php83-design.md`. It is plan 2 of 5 for Phase 1. Plan 1 (MSRV 1.89, `uuid1`) is already in the working tree, and `rust/Cargo.toml:5` reads `rust-version = "1.89"`. The order is in `.superpowers/brainstorm/2026-10-28-phase1/integration.md`.

## Global Constraints

- **Git:** the owner drives it (CLAUDE.md). Implementers stage nothing and commit nothing, and every task ends with the work left in the working tree. This holds for `../HyperForge` too.
- **Forge tasks (3-5):** they need the owner's go before execution. The edits go in the `../HyperForge` working tree only. They form one Forge PR, which merges before this repo's Task 6 changes reach CI. Every new input or key is off by default, so the siblings stay byte-for-byte unchanged. No Forge merge while a release is in flight.
- **The PHP floor:** `"php": ">=8.3"` in `composer.json` and `php/composer.json`, and `php_floor_version: "8.3"` in `ci.yml` `build-native`, landing together. The Forge's `php_floor_version` default (`hyper-build-native.yml:360-364`) stays `"8.2"`.
- **The sdist:**
  - **Name:** `hyperuuid-<version>.tar.gz` (PEP 625), with top directory `hyperuuid-<version>/`.
  - **Opt-in:** `"sdist": true` at the top level of `.github/hyper.json`. The Forge default is false.
  - **Inventory:** the pypi group is 17 wheels plus 1 sdist, **18 files**. `wheels` stays 17.
  - **Artifact:** `pypi-wheels-sdist`, holding `python/dist/hyperuuid-<version>.tar.gz`.
- **The Rust floor for a source build:** `rust-version` from the sdist's own `rust/Cargo.toml`, 1.89 today. Proven at exactly that toolchain, and one minor lower must fail with `requires rustc 1.89`.
- **The big-endian leg:**
  - The input is `big_endian`: a string, default `""`, appended as the last job of `hyper-check-core.yml`.
  - The caller passes `big_endian: s390x-unknown-linux-gnu`.
  - It runs the default-feature suite, corpus included.
- **Wording:** "no compiler needed" stays, scoped to platforms with a wheel. Install snippets keep `<version>`. READMEs state the current state, and history goes in CHANGELOG `## [Unreleased]` with no semver callout.
- **Checks:** every check names its negative control. Copy the flags from CI:
  - repo: `python3 -m unittest discover -s .github/scripts/tests`;
  - Forge: `python3 -m unittest discover -s .github/scripts -p 'test_*.py'` from the Forge root;
  - both: `actionlint -shellcheck= -ignore 'property "workflow_(sha|repository)" is not defined'` (`validate.yml`, and `hyper-check-core.yml:757`).

  Forge tests that need llvm fail on this Mac for environmental reasons. Name them in the report, don't "fix" them.
- **Siblings:** HyperCast and HyperTabular get no tasks; they are frozen until 1.1 ships.
- **Parity:** Part A is Python-only and Part B is PHP-only by nature. No other binding has a source package or a PHP floor to move. The s390x leg covers the core, and every binding inherits it.

## Spec corrections found while planning

1. **The sdist did not build on musl.** The spec says the sdist carries `.cargo/config.toml`, but that file is `rust/.cargo/config.toml`, which cargo never reads in a source build: cargo looks for config from its cwd, the sdist root.
   - `python/.cargo/config.toml`, the musl `-crt-static` fix, is not in the sdist at all.
   - Evidence: the 1.0.0-shaped sdist, `pip install`ed on `python:3.14-alpine` (aarch64) with Rust 1.89, failed with `cannot produce cdylib for hyperuuid ... aarch64-unknown-linux-musl`.
   - The same sdist with `include = [{ path = ".cargo/config.toml", format = "sdist" }]` in `[tool.maturin]` placed `hyperuuid-1.0.0/.cargo/config.toml` at the root. It installed, and `new_v7` ran.
   - Fixed in Task 2 (`python/pyproject.toml` is touched beyond line 11). Task 4 adds an Alpine install proof.
2. **The spec's PHP negative control does not discriminate.** Under 8.2, `composer install` from the committed lock already refuses today, because the lock holds dev packages requiring `>=8.4.1` (e.g. `php/composer.lock:2550`).
   - CI's floor leg deletes the lock first (Forge `musl-suite/suite-musl.sh:119`) and resolves from `composer.json`.
   - Task 1's control reproduces that recipe. Verified while planning: `php:8.3-cli-alpine` resolves `>=8.3`, and `php:8.2-cli-alpine` says "could not be resolved".
   - The image the spec called unverified exists (arm64 checked).
3. **`composer test` does not exist** (`php/composer.json` has no `scripts`). The command is `HYPERUUID_NATIVE_LIBRARY=… php vendor/bin/phpunit` (`.github/scripts/local-core.sh:59`, `hyper-build-native.yml:1154`).
4. **Running `python/tests` "from the checkout" tests the checkout, not the install.** `python/tests/conftest.py:9` puts `python/src` first on `sys.path`.
   - The suite runs from a copy, `mkdir -p <tmp>/suite && cp -rL python/tests <tmp>/suite/tests` (the `corpus` symlink is dereferenced, as `pyodide-suite` does). A bare `cp -rL python/tests <tmp>/suite/` into a missing `suite/` copies the files themselves there, with no `tests/` (checked).
   - With mypy installed, `test_typing.py`'s consumer check then fails against an installed package. It sets `MYPYPATH` to the package's parent, which is site-packages, and mypy refuses that (`site-packages is in the MYPYPATH`, reproduced on 3.14t). Task 2 fixes the test.
   - It asserts `hyperuuid.__file__` lies inside the venv.
5. **The `gh attestation verify hyperuuid-<version>*` glob fails once the wheel and the sdist are both downloaded.** `gh` takes one subject and answers `too many arguments` (checked). The README examples loop over the files instead.
6. **The release-notes switch must look at `pypi/` only.** The spec's "when the inventory contains a `.tar.gz`" also matches `cran/hyperuuid_<v>.tar.gz`, which every HyperUuid release has.
7. **The repo contract test is not unchanged.** `test_workflow_contracts.py:227-235` pins `check-core`'s exact `with:` set, so `big_endian` needs it updated. `ForgeInterfaceTests` (`:262-288`) checks every `with:` key against `../HyperForge`, so Task 6 runs after Task 5 exists in the Forge working tree.
8. **The QEMU runner needs the s390x sysroot,** `qemu-s390x -L /usr/s390x-linux-gnu`, and `libc6-dev-s390x-cross` beside `gcc-s390x-linux-gnu`.
   - Verified while planning on `ubuntu:24.04` (qemu 8.2.2, stable 1.99.1): `cargo test --release --locked --target s390x-unknown-linux-gnu` passed every test binary, including the 5 `conformance` tests reading `../corpus` at run time.
9. **Upload in every mode.** `sdist-fallthrough` runs in `pr` mode, so it needs `pypi-wheels-sdist` uploaded there too, as `build-pyodide-wheel` uploads in every mode (`hyper-build-wheels.yml:442-446`). Attestation stays release-only, under the wheels' exact `if` (`:327-330`). Release reads only release-mode runs, so a `pr` upload is never published.
10. **The README lists one more Forge-signed sentence.** The spec lists `README.md` 173, 190 and 195, but 187 ("The crate, the jar, the wheels, the pre-push NuGet package…") names the Forge-signed set too, and gains the sdist.

Probe results while planning, which are not gates:
- The 1.0.0-shaped sdist built and passed the whole suite on free-threaded 3.14 (`manylinux_2_28_aarch64`, PyO3 built `cp314t`): 322 passed and 1 skipped, with `sys._is_gil_enabled()` False after import.
- PyPy 3.11 (`pp311-pypy311_pp73`) also built, and the whole suite passed: 320 passed and 3 skipped.
- No 3.13t interpreter was available locally.

All of this ran on linux-arm64 only. CI's probe leg on x64 decides Task 8's wording.

## Review Focus

1. **A musl host with no wheel** (PyPy or 3.13t on Alpine, Alpine on s390x) should get a working source build, not `cannot produce cdylib`. Pinned by:
   - Task 2's member check and its Alpine negative control;
   - Task 4's Alpine install proof.
2. **A source-build test that silently tests the checkout.** Every suite run against an installed sdist must import from the venv. Pinned by:
   - Task 2's `__file__` assertion;
   - Task 4's same assertion in `sdist-fallthrough`.
3. **`"sdist": "false"` (a string) in hyper.json** must not switch the sdist on. Pinned by Task 3's `test_sdist_must_be_a_boolean`.
4. **A CRAN tarball, or a second `.tar.gz`, read as the sdist.** The inventory, metadata and notes look only at `pypi/`, and exactly one sdist is allowed. Pinned by:
   - Task 3's `test_sdist_layout_is_exact`;
   - `test_cran_tarball_is_not_an_sdist` (notes);
   - `test_sdist_count_is_exact` (metadata).
5. **An sdist that drops its lock or its crate** would build against whatever crates resolve that day. Pinned by Task 3's metadata test `test_sdist_must_carry_the_crate_and_its_lock`, which requires `rust/Cargo.toml` at the release version and `rust/Cargo.lock`.

---

### Task 1: PHP floor 8.3 (repo only)

**Files:**
- Test: `.github/scripts/tests/test_workflow_contracts.py`, a new test in `CiTests`.
- Modify: `composer.json:18` and `php/composer.json:18`: `"php": ">=8.2",` → `"php": ">=8.3",`.
- Modify: `php/composer.lock`, through `composer update --lock` in `php/` only. Expected diff: `content-hash` and `:3679` only.
- Modify: `php/README.md`:
  - `:13`: "PHP 8.2 is the" → "PHP 8.3 is the" (the sentence wraps onto `:14` "floor.");
  - `:153`: "**PHP 8.2 or later**" → "**PHP 8.3 or later**".
- Modify: `README.md`:
  - the floors sentence (`:165`), the PHP clause only: "PHP 8.2 and Node 22" → "PHP 8.3 and Node 22". Plan 1 already changed the Rust clause on that line, so keep it;
  - `:404`: "the 8.2+ binding" → "the 8.3+ binding".
- Modify: `ROADMAP.md`:
  - `:15` floor cell `8.2` → `8.3`;
  - `:58`: tick the row, `| PHP |` → `| [x] PHP |`, the form plan 1 used at `:59`.
- Modify: `.github/workflows/ci.yml` `build-native` `with:`. Insert after the `ruby_compat_version: "3.4"` line:
  ```yaml
      # The floor composer.json declares; the musl job runs the PHP suite on it beside
      # php_version. Explicit because the Forge's default (8.2) serves the siblings.
      php_floor_version: "8.3"
  ```
- Modify: `CLAUDE.md` "Lists that move together". Append the spec's PHP bullet verbatim after the "R's vendored-crate credits" bullet.
- Modify: `CHANGELOG.md` `## [Unreleased]` → `### Changed`. Append the spec's bullet verbatim: "**PHP: the floor is 8.3.** … *(Packagist)*".

- [ ] **Step 1: Write the failing test.** Add `test_php_floor_is_explicit_and_matches_both_composer_files` to `CiTests`:
  - `re.search(r'^      php_floor_version: "(\d+\.\d+)"$', self.jobs['build-native'], re.M)` matches, with group 1 `'8.3'`;
  - for `ROOT/'composer.json'` and `ROOT/'php/composer.json'`, `json.loads(...)['require']['php'] == '>=' + floor`.
- [ ] **Step 2: RED.** Run `python3 -m unittest discover -s .github/scripts/tests -k php_floor`. Expected: FAIL, because the match is `None`.
- [ ] **Step 3: Make the edits listed under Files.** In `php/`, run `composer update --lock && composer validate`. Expected: `./composer.json is valid`, and `git diff php/composer.lock` shows exactly two changed lines (`content-hash`, `"php": ">=8.3"`).
- [ ] **Step 4: GREEN.** Run:
  - `python3 -m unittest discover -s .github/scripts/tests`;
  - `actionlint -shellcheck= -ignore 'property "workflow_(sha|repository)" is not defined' .github/workflows/ci.yml`.

  Expected: all PASS.
  - NC: set one composer file back to `>=8.2`, and the new test FAILS. Revert.
- [ ] **Step 5: The suite on the checkout's core.** Run `.github/scripts/local-core.sh`, then, in `php/`, `composer install && HYPERUUID_NATIVE_LIBRARY=$PWD/../rust/target/release/libhyperuuid.dylib php vendor/bin/phpunit`. Expected: OK. Use the path `local-core.sh` prints if it differs.
- [ ] **Step 6: The floor leg's own recipe, both sides.** Copy `php/` without `vendor`, `src/native` or `composer.lock` to a scratch dir. In `php:8.3-cli-alpine` and `php:8.2-cli-alpine`, run `composer install --no-interaction --prefer-dist --no-progress --ignore-platform-req=ext-ffi --dry-run`, installing Composer as `suite-musl.sh:116-117` does.
  - Expected: 8.3 resolves; 8.2 prints "Your requirements could not be resolved".
  - Control: the same in 8.2 on a copy with the old `>=8.2` floor resolves. This proves that the edit, not the lock, causes the refusal.
- [ ] **Step 7: Sweep.** Run `grep -rn '8\.2' composer.json php/composer.json php/README.md README.md ROADMAP.md CLAUDE.md`. Expected: no PHP-floor hit remains (`CHANGELOG.md` history is left alone).
- [ ] **Step 8: Leave the work in the working tree.** CI-only: the musl floor leg on `php:8.3-cli-alpine` (x64 and arm64).

### Task 2: A self-contained sdist that builds on glibc and musl (repo only)

**Files:**
- Modify: `python/pyproject.toml`:
  - `[tool.maturin]`: after `features = ["python"]`, add `include = [{ path = ".cargo/config.toml", format = "sdist" }]`, with a two-line comment: cargo reads config from its cwd, the sdist root, so the musl `-crt-static` fix travels there;
  - `:11` description: "no compiler needed to install" → "no compiler needed to install on supported platforms".
- Modify: `python/.cargo/config.toml:1-2` and `:5-9` comments. Say that the file also ships at the sdist's root (via `include`), where it is what makes a source build on musl produce a cdylib.
- Modify: `python/tests/test_typing.py` `test_a_consumer_type_checks_against_the_package` (`:47-56`): set `MYPYPATH` only when `str(PACKAGE.parent)` is neither `sysconfig.get_path("purelib")` nor `get_path("platlib")`, with a one-line comment (an installed package is found through site-packages, which mypy refuses on `MYPYPATH`). The checkout run is unchanged. Probed while planning: on 3.14t, from an installed sdist, the suite went from 1 failed to 323 passed with 0 skipped.
- Modify: `python/src/hyperuuid/__init__.py:10-12` docstring (stale on sight): "no compiler needed to install" gains "where a wheel fits; elsewhere pip builds it from the sdist, which needs Rust".

**Interfaces:**
- Produces: `maturin sdist` in `python/` yields `hyperuuid-<v>.tar.gz`. Its members include `hyperuuid-<v>/PKG-INFO`, `/.cargo/config.toml`, `/rust/Cargo.toml` (whose `version` equals `<v>`) and `/rust/Cargo.lock`, and no `python/tests`. Tasks 3 and 4 rely on this shape.

- [ ] **Step 1: RED, the member check.** Run, in `python/`:
  ```
  maturin sdist --out "$S" && tar tzf "$S"/hyperuuid-1.0.0.tar.gz | grep -cx 'hyperuuid-1.0.0/.cargo/config.toml'
  ```
  Here `S` is a fresh scratch dir. Expected: `0` on today's tree.
- [ ] **Step 2: Edit `pyproject.toml` and the two comments.**
- [ ] **Step 3: GREEN.** Re-run Step 1 into a fresh `S`. Expected: `1`. Also check, with `grep -c`, that `hyperuuid-1.0.0/rust/Cargo.toml` and `hyperuuid-1.0.0/rust/Cargo.lock` are present and that `tests/test_` is absent.
- [ ] **Step 4: Local proof (macOS).** Run:
  ```
  python3 -I -m venv "$V" && "$V"/bin/pip install "$S"/hyperuuid-1.0.0.tar.gz
  ```
  Then `mkdir -p "$T/suite" && cp -rL python/tests "$T/suite/tests"`. In `$T/suite/tests`, run `"$V"/bin/pip install pytest mypy && "$V"/bin/python -m pytest -rs`.
  - Expected: all pass, `test_a_consumer_type_checks_against_the_package` passes rather than skips, and `"$V"/bin/python -c 'import hyperuuid; print(hyperuuid.__file__)'` prints a path under `$V`.
  - NC: extract the sdist into an empty dir, delete its `rust/`, and `pip install` that dir. It must FAIL.
- [ ] **Step 5: glibc 2.17 at the floor.** In `quay.io/pypa/manylinux2014_aarch64` (the CI job uses `_x86_64`), install the toolchain with rustup `--default-toolchain 1.89`. Then `/opt/python/cp311-cp311/bin/python -m venv /v && /v/bin/pip install hyperuuid-1.0.0.tar.gz` and call `native_version()`, `new_v4()`, `new_v7()` and `new_v5(uuid.NAMESPACE_DNS, 'x')`.
  - Expected: `1.0.0` and three ids (this ran while planning).
  - NC: toolchain 1.88. The install FAILS, and its log contains `requires rustc 1.89`.
- [ ] **Step 6: musl.** In `python:3.14-alpine`, run `apk add gcc musl-dev curl`, rustup 1.89, venv, `pip install` the new sdist, then `import hyperuuid; hyperuuid.new_v7()`. Expected: PASS.
  - NC: the Step 1 (pre-edit) sdist FAILS with `cannot produce cdylib`.
- [ ] **Step 7: `twine check --strict`** on the new sdist, in a throwaway `python3 -I -m venv`. Expected: `PASSED`.
  - NC: a scratch copy of `python/` whose `pyproject.toml` sets `readme = { file = "README.md", content-type = "text/x-rst" }` builds an sdist that FAILS `--strict`.
- [ ] **Step 8: Leave the work in the working tree.**

### Task 3: Forge, the `sdist` key and the release-side checks

> **Owner's go required before execution; edits in the ../HyperForge working tree only, no git.**

**Files (all under `../HyperForge/.github/`):**
- Modify: `scripts/hyper_config.py`:
  - the docstring (`:17-18`): add the optional `sdist` (boolean, default false: also publish the Python source distribution);
  - `__init__`, after `self.wheels`: `self.sdist`;
  - `shell()`: add `f'SDIST={"true" if cfg.sdist else "false"}'` after `WHEELS`;
  - `main()` json: `sdist=cfg.sdist`.
- Modify: `scripts/release_package_metadata.py`:
  - `DEFAULTS` (`:33`) gains `sdist=False`;
  - new `sdist(path, crate, version)` beside `wheel()`;
  - `validate_package_metadata` (`:265-270`).
- Modify: `scripts/release_inventory.py` `validate_layout` (`:112-117`).
- Modify: `scripts/release_notes.py` `verifying()` (`:21-62`).
- Modify: `actions/verify-wheels/action.yml`: the description, the script (`:41-63`) and the upload `path` (`:68`).
- Modify (stale on sight): `../HyperForge/docs/predicates/release-v1.md:18`: "the 17 wheels" gains ", and the sdist from a consumer that sets `sdist`". `../HyperForge/docs/release-runbook.md:95-99`: "the wheels" and "per wheel" gain the sdist.
- Test:
  - `scripts/test_hyper_config.py`;
  - `scripts/test_package_inventory_metadata.py` (`create_packages` gains `sdist=False`);
  - `scripts/test_release_inventory.py`;
  - `scripts/test_release_notes.py`;
  - `scripts/test_forge_actions.py`.

**Interfaces:**
- Produces:
  - `HyperConfig.sdist: bool`;
  - the shell line `SDIST=true|false`;
  - the JSON key `"sdist"`;
  - `release_package_metadata.sdist(path: Path, crate: str, version: str) -> None`, which raises `ValueError`;
  - `create_packages(root, crate='hypercast', version='0.10.0', sdist=False)`, which with `sdist=True` writes `pypi/{crate}-{version}.tar.gz` holding `{crate}-{version}/PKG-INFO` (`Name`, `Version`), `{crate}-{version}/rust/Cargo.toml` (`[package] version`) and `{crate}-{version}/rust/Cargo.lock`.

- [ ] **Step 1: Write the failing tests.**
  - `test_hyper_config.py`:
    - `test_sdist_is_off_by_default_and_reaches_shell_and_json`: `HyperConfig(MINIMAL).sdist is False`; with `{**MINIMAL, 'sdist': True}` it is `True`; with `write_config` plus `sdist`, bash `echo $SDIST` gives `true`, and the json gives `data['sdist'] is True`.
    - `test_sdist_must_be_a_boolean`: `'false'`, `1` and `None` each raise `ValueError` matching `sdist must be true or false`.
  - `test_package_inventory_metadata.py`:
    - `test_accepts_an_sdist_when_the_config_says_so`: `cfg = SimpleNamespace(**vars(DEFAULTS) | {'sdist': True})`.
    - `test_sdist_count_is_exact`: with `sdist` on and none present, FAIL `expected 17 wheels and 1 sdist`; with it off and one present, FAIL; two present, FAIL.
    - `test_sdist_rejects_wrong_identity`: the `PKG-INFO` `Version` is stale; two top directories; the top directory is not `{crate}-{version}`.
    - `test_sdist_must_carry_the_crate_and_its_lock`: no `rust/Cargo.toml`; a `rust/Cargo.toml` version that differs from the release; no `rust/Cargo.lock`. Each raises `ValueError`.
  - `test_release_inventory.py`, `test_sdist_layout_is_exact`. With `.github/hyper.json` `{**base, 'sdist': True}` in the workspace (the `test_layout_follows_the_consumer_config_at_b` pattern), 18 files PASS. Then each of these FAILS, matching `sdist`:
    - the sdist missing;
    - an extra `pypi/hypercast-0.10.0-1.tar.gz`;
    - a stale `pypi/hypercast-0.9.0.tar.gz`;
    - the sdist present with the key off.
  - `test_release_notes.py`:
    - `test_sdist_is_named_when_pypi_holds_one`: the body contains "the Python wheels and sdist" and "every wheel and the sdist".
    - `test_cran_tarball_is_not_an_sdist`: an inventory with only `cran/x_1.0.0.tar.gz` keeps today's text.
  - `test_forge_actions.py`, `test_verify_wheels_counts_names_and_verifies_the_sdist`. Extract the verify step's `run` (the `test_missing_or_mutable_forge_revision_fails_before_checkout` pattern). Run it with bash in a temp checkout holding `.github/hyper.json`, `rust/Cargo.toml` (`version = "0.10.0"`) and `dist/`, with `FORGE_SCRIPTS` set to the scripts dir and a fake `gh` on `PATH` that logs its arguments and exits 0.
    - These PASS: 17 `.whl` files plus `hypercast-0.10.0.tar.gz` with `sdist: true`, where `gh` logged 18 subjects; and 17 wheels with no sdist and no key.
    - These FAIL: the sdist with the key off; the key on and no sdist; `hypercast-0.9.0.tar.gz`; two `.tar.gz` files.
    - Also assert that the upload step's `path` contains both `dist/*.whl` and `dist/*.tar.gz`.
- [ ] **Step 2: RED.** From the Forge root, run `python3 -m unittest discover -s .github/scripts -p 'test_*.py' -k sdist -k verify_wheels`. Expected: FAIL (no `sdist` attribute, unchanged counts, unchanged text).
- [ ] **Step 3: Implement.**
  - `hyper_config`: a value that is not a `bool` raises `ValueError('sdist must be true or false')`.
  - `validate_layout`: split `groups['pypi']` into the `.whl` and `.tar.gz` files, rejecting any other suffix. Expect exactly `cfg.wheels` wheels, as today, and `[f'pypi/{crate}-{version}.tar.gz']` when `cfg.sdist`, else none. Errors name `sdist`.
  - `sdist()`:
    - `tar_entries` (it already refuses links and unsafe paths);
    - exactly one top directory, `{crate}-{version}`;
    - `PKG-INFO` parsed with `BytesParser` and normalised exactly as `wheel()` does (`:116-125`);
    - `rust/Cargo.toml` `[package].version == version`;
    - `rust/Cargo.lock` present.
  - `validate_package_metadata`: glob `(root/'pypi').rglob('*.tar.gz')`. The count message becomes `expected {cfg.wheels} wheels and {int(cfg.sdist)} sdist`.
  - `release_notes`: `sdist = any(e['path'].startswith('pypi/') and e['path'].endswith('.tar.gz') for e in files)`. Line 52 reads "the Python wheels{' and sdist' if sdist else ''}", and line 62 "every wheel{' and the sdist' if sdist else ''}".
  - `verify-wheels`: `sdists=(dist/*.tar.gz)` under the existing `nullglob`. Its count must be `1` when `[ "$SDIST" = true ]`, else `0`. A `case` arm `"$CRATE"-"$want".tar.gz)` is accepted only when `SDIST=true`, and the same `gh attestation verify` loop covers `wheels` plus `sdists`. The upload `path:` is a block with `dist/*.whl` and `dist/*.tar.gz`. The description says "every file".
- [ ] **Step 4: GREEN.** Run the full Forge suite and actionlint, with the flags from Global Constraints. Expected: PASS, except the llvm-environmental tests, named in the report.
  - NC: revert the `validate_layout` change alone, and `test_sdist_layout_is_exact` FAILS.
  - NC: drop the `SDIST` guard from the `case`, and the action test FAILS.
- [ ] **Step 5: Leave the work in the Forge working tree.**

### Task 4: Forge, `build-sdist` and `sdist-fallthrough`

> **Owner's go required before execution; edits in the ../HyperForge working tree only, no git.**

**Files:**
- Modify: `../HyperForge/.github/workflows/hyper-build-wheels.yml`:
  - the header (`:3`, `:18-20`): one paragraph on the sdist;
  - the `:100-104` comment ("with no sdist published that means no install at all") becomes current;
  - three new jobs, written before `build-wheels` (`:86`) in this order: `sdist-config`, `build-sdist`, `sdist-fallthrough`.
- Test: `../HyperForge/.github/scripts/test_forge_producers.py`, a new class `SdistContracts`.

**Interfaces:**
- Consumes: `HyperConfig.sdist` and `SDIST` (Task 3), and Task 2's sdist shape.
- Produces:
  - artifact `pypi-wheels-sdist` = `python/dist/{crate}-{version}.tar.gz`, uploaded in every mode and attested in release mode only;
  - job `sdist-config`, output `sdist`.

- [ ] **Step 1: Write the failing test,** `SdistContracts.test_sdist_jobs_shape`, with `yaml_lite`:
  - `build-sdist` and `sdist-fallthrough` each have, as their first step, a plain `actions/checkout` of the consumer at the file's pin, with no `repository` or `ref`. That is how `build-wheels` (`:142`) and `build-pyodide-wheel` (`:365`) check out the caller; `repository`/`ref` appear only on the `.hyperforge` checkout. The checkout in `sdist-config` does not reach another job's workspace;
  - `sdist-config` checks out `.hyperforge` at `${{ job.workflow_sha }}` after the `Validate Forge workflow revision` step, and has `outputs.sdist`;
  - `build-sdist` has `needs: sdist-config`, `if: needs.sdist-config.outputs.sdist == 'true'`, and no `inputs.mode` in its `if`;
  - the jobs appear in the file before `build-wheels:`;
  - `build-sdist` steps contain: `command: sdist`; `twine check --strict`; `manylinux2014_x86_64`; `cp311-cp311`; `python:3.11-alpine`; `requires rustc`; `/v/bin/python -I -c` and no bare `python -I`; an attest step whose `if` equals `build-wheels`' attest `if` and whose `subject-path: python/dist/*.tar.gz`; and upload `name: pypi-wheels-sdist` with `if-no-files-found: error` and no `if`;
  - `sdist-fallthrough` has `continue-on-error: true`, `needs: build-sdist`, matrix python `["3.13t", "3.14t", "pypy3.11"]`, and its steps contain `GITHUB_STEP_SUMMARY`, `cp -rL python/tests "$RUNNER_TEMP/suite/tests"`, `mkdir -p "$RUNNER_TEMP/suite"`, `"$RUNNER_TEMP/v/bin/pip" install pytest mypy`, `"$RUNNER_TEMP/v/bin/python" -m pytest -rs`, no bare `python -I`, `_is_gil_enabled` and `__file__`;
  - every `run` step in the three jobs has `shell`.
- [ ] **Step 2: RED.** Run `python3 -m unittest discover -s .github/scripts -p 'test_forge_producers.py' -k sdist`. Expected: FAIL (`KeyError: 'sdist-config'`).
- [ ] **Step 3: Write the jobs.** Pins are the ones the file already uses. The `CRATE` env is `inputs.crate`, and `want` is read from `rust/Cargo.toml` as at `:253`.
  - **`sdist-config`** (ubuntu-26.04): checkout; the validate step and the `.hyperforge` checkout copied from `:367-380`; then `eval "$(python3 .hyperforge/.github/scripts/hyper_config.py shell)"; echo "sdist=$SDIST" >> "$GITHUB_OUTPUT"`.
  - **`build-sdist`** (ubuntu-26.04):
    0. `actions/checkout` of the consumer (the `:142` line, no inputs).
    1. `PyO3/maturin-action` (`:219` pin), with `working-directory: python`, `command: sdist` and `args: --out dist`.
    2. Name check: `python/dist` holds exactly `"$CRATE-$want.tar.gz"`, and no other file.
    3. `pipx run twine check --strict` on it.
    4. Read `msrv` from `tar xzOf … "$CRATE-$want/rust/Cargo.toml"` (`sed -n 's/^rust-version = "\(.*\)"/\1/p'`), and compute `below` as major.(minor−1).
    5. `in_image() { docker run --rm -v "$PWD/python/dist:/dist:ro" "$1" sh -euc "$2"; }`. In `quay.io/pypa/manylinux2014_x86_64`: rustup `--profile minimal --default-toolchain "$msrv"`; `/opt/python/cp311-cp311/bin/python -m venv /v`; `/v/bin/pip install /dist/…`; then `/v/bin/python -I -c` asserting (never a bare `python`: in manylinux2014 it is 2.7.5, which rejects `-I`, checked; on Alpine it is outside the venv) `native_version() == want` and calling `new_v4()`, `new_v7()` and `new_v5(uuid.NAMESPACE_DNS, "hyperuuid")`.
    6. The same with `$below`. Its exit status must be non-zero, and its output must contain `requires rustc $msrv`, or the job fails with `::error::$below built the sdist, so $msrv is not the floor`.
    7. The same as step 5 in `python:3.11-alpine`, after `apk add --no-cache gcc musl-dev curl`, at `$msrv`.
    8. The attest step, with the wheels' `if` and `subject-path: python/dist/*.tar.gz`.
    9. Upload `pypi-wheels-sdist`, with `path: python/dist/*.tar.gz` and `if-no-files-found: error`.
  - **`sdist-fallthrough`** (ubuntu-26.04, `continue-on-error: true`, `fail-fast: false`): `actions/checkout` of the consumer (the `:142` line, for `python/tests`); download `pypi-wheels-sdist`; `setup-python` with `matrix.python`; stable Rust; a fresh venv, `"$RUNNER_TEMP/v"`, created by `setup-python`'s `python-path` output. Every later call names `"$RUNNER_TEMP/v/bin/python"` or `"$RUNNER_TEMP/v/bin/pip"`, never a bare `python`, `pip` or `pytest`. Each outcome goes to `$GITHUB_STEP_SUMMARY` as one line, `<python>: <outcome>`, with the last 20 lines of the relevant log in a code block. The step fails unless the outcome is the third.
    - If `pip install` fails, the outcome is "build failed".
    - Otherwise `mkdir -p "$RUNNER_TEMP/suite" && cp -rL python/tests "$RUNNER_TEMP/suite/tests"`, `"$RUNNER_TEMP/v/bin/pip" install pytest mypy`, and check that `hyperuuid.__file__` starts with the venv's prefix.
    - On a `t` interpreter, `sys._is_gil_enabled()` must be `False` after the import.
    - Then run `"$RUNNER_TEMP/v/bin/python" -m pytest -rs` in `$RUNNER_TEMP/suite/tests`. A failure gives "built, suite failed". The summary lists every `SKIPPED` line. A pass gives "built, suite passed" only when every skip is an allowed one, else "built, suite failed (unexpected skip)". The allowed skips are by reason string: `needs a free-threaded CPython` on a GIL interpreter (PyPy), and `stdlib uuid.uuid7() was added in Python 3.14` below 3.14 (3.13t, PyPy 3.11; `test_uuid_generator.py:176`). The second is version-gated like the first, which the brief's "only the free-threading one" did not cover.
- [ ] **Step 4: GREEN.** Run the Forge suite and actionlint (Global Constraints). Expected: PASS.
  - NC: move `sdist-fallthrough` below `build-wheels`, and the test FAILS.
  - NC: put `inputs.mode != 'pr'` into `build-sdist`'s `if`, and the test FAILS.
- [ ] **Step 5: Leave the work in the Forge working tree.** CI-only after the Forge merge:
  - the manylinux2014 **x86_64** install at `rust-version` and its `$below` control;
  - the Alpine **x86_64** install;
  - the three fall-through outcomes.

  Task 2 proved the aarch64 equivalents locally.

### Task 5: Forge, `test-big-endian`

> **Owner's go required before execution; edits in the ../HyperForge working tree only, no git.**

**Files:**
- Modify: `../HyperForge/.github/workflows/hyper-check-core.yml`:
  - the input `big_endian`, after `swift_version` (`:151`);
  - the job `test-big-endian`, appended after `lint-workflows` (`:745-757`);
  - one clause in the header (`:3-13`).
- Test: `../HyperForge/.github/scripts/test_forge_producers.py`, a new `CheckCoreLintContracts.test_big_endian_leg_is_off_by_default_and_guards_itself`.

**Interfaces:**
- Produces: the input `big_endian`, of type `string`, `required: false`, `default: ""`, with the description "A big-endian `<arch>-unknown-linux-gnu` target: the crate's default-feature cargo test suite, corpus included, cross-compiled and run under qemu-user; empty for none". It also produces the job `test-big-endian`. Task 6 passes `s390x-unknown-linux-gnu`.

- [ ] **Step 1: Write the failing test.** Assert:
  - `inputs['big_endian']` has `default == ''` and `type == 'string'`;
  - `jobs['test-big-endian']['if'] == "inputs.big_endian != ''"`;
  - it is the last key in `jobs`;
  - its steps contain `qemu-user`, `libc6-dev-`, `-cross`, `-L /usr/`, `cargo test --release --target` and `sql_order.json`;
  - every `run` step has `shell`.

  Also extract its triple-guard snippet and run it with bash:
  - `TARGET=s390x-unknown-linux-gnu` exits 0;
  - `TARGET=s390x-unknown-linux-musl` exits 1;
  - `TARGET=x86_64-pc-windows-msvc` exits 1.
- [ ] **Step 2: RED.** Run `python3 -m unittest discover -s .github/scripts -p 'test_forge_producers.py' -k big_endian`. Expected: FAIL (`KeyError`).
- [ ] **Step 3: Write the job** (ubuntu-26.04, `env: TARGET: ${{ inputs.big_endian }}`):
  1. Checkout.
  2. `dtolnay/rust-toolchain` at the stable pin of `:233`, with `targets: ${{ inputs.big_endian }}`.
  3. The guard step: `arch=${TARGET%%-*}`, and `[[ "$TARGET" == "$arch-unknown-linux-gnu" ]]` or `::error::` and exit 1. It exports `ARCH` to `$GITHUB_ENV`.
  4. `sudo apt-get update && sudo apt-get install -y qemu-user gcc-$ARCH-linux-gnu libc6-dev-$ARCH-cross`.
  5. In `rust`, set `v=CARGO_TARGET_$(tr a-z- A-Z_ <<<"$TARGET")`, export `${v}_LINKER=$ARCH-linux-gnu-gcc` and `${v}_RUNNER="qemu-$ARCH -L /usr/$ARCH-linux-gnu"`, then run `cargo test --release --target "$TARGET"`.
  6. The negative control:
     - `sed -i '0,/"sql": "00000000000000808000000000000000"/s//"sql": "01000000000000808000000000000000"/' ../corpus/sql_order.json`;
     - `git diff --quiet -- ../corpus` must report a change;
     - `cargo test --release --target "$TARGET" --test conformance` must FAIL, or `::error::the corpus is not read on $TARGET`;
     - `git checkout -- ../corpus`.
- [ ] **Step 4: GREEN.** Run the Forge suite and actionlint. Expected: PASS.
  - NC: set the default to `s390x-unknown-linux-gnu`, and the test FAILS.
- [ ] **Step 5: The leg's recipe locally.** In `ubuntu:24.04` (aarch64 host), with a scratch copy of the repo's `rust/` and `corpus/` mounted (never the checkout itself: step 6 edits the corpus, which is add-only and never hand-edited), run the job's steps 3-6 verbatim; in the copy, step 6's `git diff`/`git checkout` become `cmp` against, and a copy back from, the checkout's file. The `sudo` is dropped, `CARGO_TARGET_DIR` is set outside the mount, and the step uses `--locked` as the planning run did.
  - Expected: every test binary `ok` (this ran while planning), and the step-6 control FAILS `conformance`.
  - CI-only: the same on ubuntu-26.04 x64, whose package names are unverified until it runs.
- [ ] **Step 6: Leave the work in the Forge working tree.**

### Task 6: Repo wiring: `hyper.json` and `ci.yml`

**Files:**
- Modify: `.github/hyper.json`: add `"sdist": true` after `"cdylib_smoke"`.
- Modify: `.github/workflows/ci.yml`:
  - `check-core` `with:`, after `no_panic_command:`: `big_endian: s390x-unknown-linux-gnu`;
  - the comment above `check-core`: after "and the no-panic link.", add "and `big_endian` runs the core's suite, corpus included, on s390x under QEMU.";
  - the `build-wheels` comment (`# The 17 PyPI wheels…`): add the sentence "With `sdist` in `.github/hyper.json`, the source distribution too: built, `pip install`ed at exactly `rust-version` on manylinux2014 and Alpine (one minor lower must fail), attested and uploaded as `pypi-wheels-sdist`."
- Test: `.github/scripts/tests/test_workflow_contracts.py:227-235`:
  - rename to `test_static_checks_run_in_the_forge_with_this_repositorys_seven_inputs`;
  - add `'big_endian'` to the set;
  - add `self.assertIn('big_endian: s390x-unknown-linux-gnu', core)`.
- Test: `.github/scripts/tests/test_abi_probe.py`: one assertion, `self.assertIs(self.cfg.get('sdist'), True)`, in the test that reads `cdylib_smoke` (`:35`).

**Interfaces:**
- Consumes: Task 3's `sdist` key and Task 5's `big_endian` input, both present in `../HyperForge`'s working tree.

- [ ] **Step 1: Write the test edits.**
- [ ] **Step 2: RED.** Run `python3 -m unittest discover -s .github/scripts/tests`. Expected: FAIL on the set and on `sdist`.
- [ ] **Step 3: Edit `hyper.json` and `ci.yml`.**
- [ ] **Step 4: GREEN.** Run the repo suite and actionlint (flags in Global Constraints). Expected: PASS, and `ForgeInterfaceTests` ran rather than skipped. Check for `test_calls_match_declared_inputs ... ok` in `-v` output.
  - NC: with Task 5's input renamed in the Forge working tree, `test_calls_match_declared_inputs` FAILS. Revert.
  - Also run `python3 ../HyperForge/.github/scripts/hyper_config.py shell --root . | grep -x SDIST=true`. Expected: one line.
- [ ] **Step 5: Leave the work in the working tree.** CI-only after the Forge merge: the first release-mode `ci.yml` run showing `build-sdist` and `test-big-endian` green.

### Task 7: Docs, CHANGELOG, CLAUDE.md and ROADMAP

**Files:**
- Modify: `python/README.md`:
  - `:20-21`: "Real platform-specific wheels, so it lands" → "Real platform-specific wheels, so on every platform in the table below it lands".
  - `:40-42`: replace "**Only wheels are published — there is no sdist.** … not a source build." with the spec's "Contract changes" paragraph, verbatim. It goes after the platform table, so "(the table below)" reads "(the table above)", and its last sentence (free-threaded and PyPy) is left out until Task 8. The "Free-threaded CPython older than 3.15 has no stable ABI…" sentences stay as their own paragraph.
  - `:366`: "Every wheel PyPI serves, the Pyodide one included," → "Every wheel PyPI serves, the Pyodide one included, and the sdist".
  - `:376`: "Each wheel also carries" → "Each wheel and the sdist also carry".
  - `:389`: "for every wheel" → "for every wheel and the sdist".
  - The two `gh attestation verify hyperuuid-<version>-*.whl` examples: add the PyPI JSON `curl -sLO "$(curl -s https://pypi.org/pypi/hyperuuid/<version>/json | jq -r '.urls[] | select(.packagetype=="sdist") | .url')"` beside the existing `pip download`, and run each verify as `for f in hyperuuid-<version>*; do gh attestation verify "$f" …; done`.
- Modify: `README.md`:
  - `:88`: "so `pip` always resolves a native wheel and there is no `ctypes` fallback" → "so `pip` resolves a native wheel on every platform that has one, builds the same extension from the sdist everywhere else, and there is no `ctypes` fallback";
  - `:173`: "the gems, the wheels, the crate" → "the gems, the wheels and the sdist, the crate";
  - `:187`: "The crate, the jar, the wheels, the pre-push" → "The crate, the jar, the wheels and the sdist, the pre-push";
  - `:190-191`: "the wheels and the\nnatives at the preparation commit" → "the wheels, the sdist and the\nnatives at the preparation commit";
  - `:195`: "for every wheel, filed by" → "for every wheel and the sdist, filed by";
  - `:229`: "Not built. Only platform wheels are published, each carrying the PyO3 extension, and no sdist, so" → "Not built. Platform wheels are published, each carrying the PyO3 extension, and a source distribution for platforms without one, so".
- Modify: `CHANGELOG.md` `## [Unreleased]`:
  - `### Added`: the spec's sdist bullet, verbatim;
  - `### Added`: "**Core on big-endian.** CI runs the core's test suite, conformance corpus included, on s390x under QEMU. *(repository only)*".
- Modify: `CLAUDE.md` "Lists that move together": the spec's Python sdist bullet, verbatim, after Task 1's PHP bullet.
- Modify: `ROADMAP.md`:
  - `:13`: "PyPI, wheels only (no sdist)" → "PyPI, wheels and an sdist";
  - `:57`: `| Python |` → `| [x] Python |`.

- [ ] **Step 1: Make the edits.**
- [ ] **Step 2: Verify the examples by hand.** In a scratch dir, run `pip download hyperuuid==1.0.0 --no-deps -d .`, then the `for` loop with both verify forms. Expected: the 1.0.0 wheel verifies.
  - NC: the old single-command form over two files (`touch x` beside the wheel, then `gh attestation verify hyperuuid-1.0.0* x …`) prints `too many arguments`.
  - The sdist line can only be checked once 1.1.0 is published; it is listed under "Release-time" below.
- [ ] **Step 3: Sweep.** Run `grep -rn 'no sdist\|always resolves a native wheel\|Only wheels are published' README.md python/ ROADMAP.md`. Expected: no hits.
  - Run `grep -n 'sdist' python/README.md README.md CHANGELOG.md CLAUDE.md`. Expected: one hit or more in each file.
- [ ] **Step 4: Leave the work in the working tree.**

### Task 8: The fall-through wording (after a CI run of `sdist-fallthrough`)

**Files:**
- Modify: `python/README.md`: the last sentence of the Task 7 paragraph.
- Modify: `python/README.md:41-42` region: the free-threaded paragraph, only if a leg is promoted.

**Interfaces:**
- Consumes: the job summary of `sdist-fallthrough` from a release-mode `ci.yml` run after the Forge merge. Use `gh run view <id> --log` or the summary; the run id goes in the report.

- [ ] **Step 1: Record the three outcomes** (3.13t, 3.14t, PyPy 3.11) from that run.
- [ ] **Step 2: Write one sentence per interpreter group, by outcome:**
  - **built, suite passed:** "Free-threaded 3.13 and 3.14 (or PyPy) build from the sdist, and CI runs the whole suite on that build." This applies only after its leg is made gating: in a Forge follow-up, drop `continue-on-error` for that matrix entry, with the owner's go.
  - **built, suite failed:** "… the source build completes but is not supported: <what failed>."
  - **build failed:** "… pip attempts a source build, and it fails."
- [ ] **Step 3: Check the README against the record.** Each sentence must match the run's summary line for that interpreter.
  - NC: no README claim names an interpreter with no leg.
- [ ] **Step 4: Leave the work in the working tree.**

## Release-time (owner, not a task)

These come from the spec's Verification:
1. The release-mode board at the release commit shows `build-sdist` and `test-big-endian` green before the tag.
2. The tag run's `pypi-pack` accepts 18 files, as do the inventory and the package metadata.
3. The `workflow_dispatch` publish runs, then `registry-confirm` for all 18.
4. By hand:
   - the sdist's PyPI file page shows its PEP 740 attestation;
   - the README's PyPI JSON `curl` returns `hyperuuid-1.1.0.tar.gz`;
   - `gh attestation verify` passes on it with the release predicate;
   - the README's sdist download line is re-run;
   - Packagist's 1.1.0 page declares `php >=8.3`.
