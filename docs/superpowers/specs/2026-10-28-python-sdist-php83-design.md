# Python sdist and the PHP 8.3 floor

Brainstormed 2026-10-28 (Phase 1, "small floors"). Draft for owner review. Two separable
parts in one spec: **Part A** is the Python sdist, **Part B** is the PHP floor. Neither
depends on the other; either can be cut without touching the other. Both ship in 1.1.0.

## Goal

Reach. Part A lets pip install hyperuuid on platforms with no wheel (FreeBSD, ppc64le,
s390x, a glibc older than 2.28) by building the Rust core from source, with the claim
"no compiler needed" kept true for every platform that has a wheel. Part B moves the PHP
floor from 8.2 to 8.3 now, before 8.2 reaches end of life on 2026-12-31 and before real
adoption in 2027, so the floor never rests on an unsupported runtime.

## Decisions

- **Python sdist: ship it** (owner, 2026-10-28). Built in the Forge's `hyper-build-wheels.yml`,
  one more inventory file, opted in by a `hyper.json` key.
- **sdist wording** (owner, 2026-10-28). Keep "no compiler needed" for platforms with a wheel;
  platforms without one build from the sdist and need Rust >= 1.89 (the MSRV bump decided
  under "Rust uuid interop").
- **PHP: floor 8.2 to 8.3, in 1.1.0** (owner, 2026-10-28). Adoption is 2027, past 8.2's EOL.
- **Ruby 3.3: decided against** (owner, 2026-10-28).
- **Siblings frozen** until v1.1 ships (owner, 2026-10-28); no sibling work is listed here.
- **Release vehicle:** 1.1.0, the ubiquity release; the release-mode board proves the new
  artifact shape before the tag.

# Part A: Python sdist

## Design

**The artifact.** `maturin sdist` run in `python/` yields `hyperuuid-<version>.tar.gz`
(PEP 625 name, verified 2026-10-28 on 1.0.0: 125 KB; top directory `hyperuuid-1.0.0/`
holding `PKG-INFO`, `pyproject.toml`, `LICENSE`, `README.md`, `src/hyperuuid/`, and the whole
`rust/` crate with `Cargo.lock` and `.cargo/config.toml`; `manifest-path` rewritten to
`rust/Cargo.toml`). It is self-contained because the core is the crate being built, not a
path dependency. It carries the feature-gated PHP and Ruby extension sources, which are
inert here. It carries no `python/tests`.

**Opt-in.** `.github/hyper.json` gains `"sdist": true` (top level, beside `wheels`). In
`hyper_config.py` the config gets `self.sdist = bool(data.get('sdist', False))`, the
`shell` output a `SDIST=true|false` line, the JSON output the same key, and the module
docstring one entry. Default false: a consumer without the key is byte-for-byte unchanged.
`wheels` stays 17; the pypi inventory group is then 17 wheels plus 1 sdist = **18 files**.

**Forge build (`hyper-build-wheels.yml`).** One new job, `build-sdist`, written before
`build-wheels` in the file (fail fast: newest code first). A one-step `sdist-config` job
checks out the Forge at `job.workflow_sha` into `.hyperforge` (the pattern at the
Pyodide job's checkout) and reads `hyper_config.load('.').sdist`. `build-sdist` needs it and
runs only when true. It runs in `pr` mode too, because the build and install test are cheap
and a broken sdist should fail the pull request; attestation and upload stay release-only, as
for the wheels. Steps, in order:
1. `PyO3/maturin-action` at the pin the wheel job already uses, `command: sdist`,
   `args: --out python/dist`, on ubuntu-26.04.
2. Name check: the only file is `python/dist/${CRATE}-${want}.tar.gz`, `want` read from
   `rust/Cargo.toml` as the wheel job does.
3. `twine check --strict` on it (a metadata or README-rendering defect would be rejected by
   PyPI, and a rejected file name is spent for good).
4. **Install proof, floor toolchain.** In `quay.io/pypa/manylinux2014_x86_64` (glibc 2.17,
   older than any wheel tag we ship), install the toolchain named by `rust-version` read
   from the sdist's own `rust/Cargo.toml` (1.89 once the MSRV bump lands), then
   `pip install ./hyperuuid-<v>.tar.gz` into a clean venv on CPython 3.11, import, check
   `hyperuuid.__version__`-equivalent against the sdist version, and call `new_v4`, `new_v7`
   and `new_v5`. This proves the README's "Rust >= 1.89" lower bound exactly. The
   `check-msrv` job checks the library with `cdylib` (and `uuid1` once the Rust item lands), never
   the `python` feature, so it cannot prove PyO3 builds at 1.89; this step does.
5. **Negative control** (same job): the same install with the toolchain one minor lower
   (1.88) must fail, and the step asserts the failure. If it passes, "1.89" is not the floor
   and the job fails.
6. Attest (`actions/attest-build-provenance`, same pin and `if` as the wheels, subject
   `python/dist/*.tar.gz`), then upload as artifact `pypi-wheels-sdist`
   (`path: python/dist/*.tar.gz`, `if-no-files-found: error`). The name matches
   `verify-wheels`'s existing `pypi-wheels-*` download pattern, so no download change.

**Probe leg (non-gating).** A second job `sdist-fallthrough`, `continue-on-error: true`,
matrix free-threaded 3.13, free-threaded 3.14 and PyPy 3.11, runs `pip install` of the same
sdist and the same three calls, and writes pass or fail plus the last 20 build lines to the job
summary. It is a measurement, not a gate; see Fall-through.

**Inventory and checks (exact names).** Preserved path `pypi/hyperuuid-<version>.tar.gz`,
beside `pypi/hyperuuid-<version>-<tags>.whl`.
- `verify-wheels/action.yml`: count `dist/*.whl` against `$WHEELS` as now; add
  `sdists=(dist/*.tar.gz)`, which must hold exactly 1 when `SDIST=true` and 0 otherwise
  (`eval` of `hyper_config.py shell` already supplies the variable); the `case` accepts
  `"$CRATE"-"$want".tar.gz`; the same `gh attestation verify --repo --signer-repo
  SkunkWerkx/.github --source-digest` loop covers it. The final upload's `path` becomes
  both `dist/*.whl` and `dist/*.tar.gz`. The header comment says "every file".
- `release_inventory.py` `validate_layout` (lines 112-117): expect `cfg.wheels` `.whl` files
  whose name starts `{crate}-{version}-`, plus one `{crate}-{version}.tar.gz` when `cfg.sdist`,
  and nothing else; an sdist present with `sdist` off is an error, so is one missing.
  `registry-plan` (line 300) is keyed on file name and sha256 and needs no change.
- `release_package_metadata.py` (lines 265-270): the glob adds `*.tar.gz`; a new `sdist()`
  beside `wheel()` opens the tarball, requires exactly one top directory
  `{crate}-{version}`, reads its `PKG-INFO`, applies the same Name and Version
  normalisation as `wheel()`, and requires `{crate}-{version}/rust/Cargo.toml` whose
  `version` equals the release (self-containment, the property the whole shape rests on).
- `release_notes.py` lines 52 and 62: "Python wheels" and "every wheel" become "Python wheels
  and sdist" and "every wheel and the sdist" when the inventory contains a `.tar.gz`.
- **Release attestation:** `inventory.json` lists every file, so the sdist is covered with
  no code; `release-gate` downloads `pypi-packages` whole.
- `release.yml`, `publish-pypi` and `registry-confirm`: no change (the publish action
  uploads every file in `pending-pypi`).

**Upload and PEP 740.** `pypa/gh-action-pypi-publish` (`release.yml:404`) uploads the sdist
with the wheels and, under Trusted Publishing, files a PEP 740 attestation for each file it
uploads, source distributions included (PyPI's producing-attestations page). The release
checks the PyPI file page for the sdist (see Release-time). The README sentence "a PEP 740
attestation for every wheel" becomes "every wheel and the sdist".

**Fall-through.** Today pip answers `No matching distribution found` where no wheel fits.
With the sdist it starts a source build. PyO3 applies its stable ABI (`abi3-py311`) to
GIL CPython, and `abi3t-py315` only from 3.15; whether PyO3 0.30 builds a version-specific
extension for free-threaded 3.13/3.14, and whether the CPython-C-API fast path (the
`uuid.UUID` built through the C API's own entry points) builds on PyPy, are **unverified**:
the brief did not probe them and neither has this spec. The probe leg above settles it before
the tag, on the real sdist. Whatever it finds, two rules hold. The outcome is a loud pip
build error or a working source build, never a silent wrong install. And the README claims
only what a gating leg runs:
The three calls are discovery only (amended after review, 2026-10-28). Promotion needs more.
The probe records one of three outcomes per interpreter: **build failed**; **built, suite
failed**; **built, suite passed**.
- **Promotion to a gating leg** ("builds from the sdist and is tested") requires the whole
  `python/tests` suite against the installed sdist build. That includes
  `test_free_threading.py`'s concurrent-resize and shared-buffer fill tests, the cases this repo
  already treats as crash-sensitive. On 3.13t/3.14t it also requires `sys._is_gil_enabled()` to
  be `False` after `import hyperuuid`, so the GIL was not silently re-enabled.
- **Built, suite failed:** the README says the source build completes but is not supported,
  and names what failed.
- **Build failed:** the README says pip will attempt a source build that fails. The leg stays
  a non-gating record. Failing in a
  build is a worse message than "No matching distribution", and that cost is accepted.

**s390x and big-endian.** A static read finds no hazard (explicit `to_be_bytes` and
`from_le_bytes` in `sql.rs`, `v5.rs`, `v7.rs`; no transmute), and `cargo check --features
python` passed for FreeBSD, ppc64le and s390x (check only; brief, 2026-10-28). Nothing has run
on big-endian. **Recommendation: add a real leg, not just a label.** Reason, by the
nitpick-proof criterion: the layout is persisted bytes, and a README that says "untested"
is true but invites the exact objection a bank on IBM Z will raise; a leg turns it into a
receipt. Cost is one cheap job. In the Forge's `hyper-check-core.yml`, a job `test-big-endian`
(toggle `inputs.big_endian`, a target string, default empty) installs `qemu-user` and
`gcc-s390x-linux-gnu`, adds `s390x-unknown-linux-gnu`, and runs `cargo test --release
--target s390x-unknown-linux-gnu` with `CARGO_TARGET_S390X_UNKNOWN_LINUX_GNU_RUNNER=qemu-s390x`
(plus the linker variable) over the default-feature suite, which includes the corpus
vectors, so every persisted byte layout (SQL Server v8, v5, v7 fields) is asserted
big-endian. Repo `ci.yml` passes `big_endian: s390x-unknown-linux-gnu` in its `check-core`
`with:`. Negative control: the same job flips one corpus vector's expected byte in a scratch
copy and requires the run to fail. What the leg does not prove, and the README says: the
PyO3 extension and the sdist build on s390x have not run; only the core has.
If the owner declines the leg, the fallback is the label alone, "untested" in the README.

**Local proof.** From `python/`: `maturin sdist --out S`; extract into a fresh empty
directory; `python3 -I -m venv v && v/bin/pip install S/hyperuuid-<v>.tar.gz`; then
`v/bin/python -c "import hyperuuid; print(hyperuuid.new_v4(), hyperuuid.new_v7())"`, and run
`python/tests` against the installed build (the tests are not in the sdist, so they run from
the checkout). Done by hand on 1.0.0 on 2026-10-28: `maturin build --release` of the unpacked
sdist took 6 s and produced a wheel (the import and pytest steps are still to run).
Negative control: delete `rust/` from the extracted sdist; the build must fail.

## Contract changes

`python/README.md`, replacing line 40-44 ("Only wheels are published — there is no sdist...")
and the sentence at lines 20-22:

> Platforms with a wheel (the table below) install with no compiler, nothing to compile and
> no dependencies. Every other platform builds from the source distribution,
> `hyperuuid-<version>.tar.gz`, and needs Rust 1.89 or newer and a C linker. CI builds it at
> exactly Rust 1.89 on glibc 2.17. pip's build isolation fetches `maturin`, and where PyPI
> has no `maturin` wheel (FreeBSD) pip builds it too, or install your platform's
> `py-maturin` and use `--no-build-isolation`. s390x (big-endian): the core's test suite,
> byte-exact corpus included, passes on s390x under QEMU; the extension itself has not run
> there. FreeBSD and ppc64le compile (`cargo check`) and are untested at run time.
> Free-threaded 3.13 and 3.14 and PyPy: `<one of the two wordings under Fall-through>`.

The free-threaded paragraph at lines 42-44 stays but loses "not a source build". `python/
pyproject.toml:11` description keeps "no compiler needed to install" and gains "on
supported platforms". Root `README.md` 88: "pip always resolves a native wheel" gets "on every
platform that has a wheel". 229: "and no sdist, so there is no install a wasm backend could
fill in for" becomes "and a source distribution for platforms without one". 173, 190, 195:
"the wheels" gains "and the sdist". `python/README.md` 366, 376, 389: "Every wheel" gains
"and the sdist" (the `gh attestation verify` examples take `hyperuuid-<version>*` globs).

CHANGELOG, Unreleased, Added: "**Python: a source distribution.** `hyperuuid-<version>.tar.gz`
now ships with the wheels, so pip builds on platforms with no wheel (needs Rust 1.89 or
newer). Attested like the wheels. *(PyPI)*"

## Files

`[own]` Part A, repo: `python/README.md`, `python/pyproject.toml` (line 11 only).
`[shared]`, repo:
- `.github/hyper.json`: add `"sdist": true`.
- `.github/workflows/ci.yml`: `check-core` `with:` gains `big_endian` (lines ~200-204 only); the
  `build-wheels` comment at 165-172 gains one sentence for the sdist.
- `README.md`: lines 88, 173, 190, 195, 229 (clauses above).
- `CHANGELOG.md`: one bullet under Unreleased, Added.
- `CLAUDE.md`: Part A adds to "Lists that move together": "The Python sdist: built in the
  Forge's `hyper-build-wheels.yml` when `.github/hyper.json` says `sdist: true`; one more
  pypi inventory file, `hyperuuid-<version>.tar.gz`, beside the 17 wheels. Its Rust floor is
  `rust-version`, proven at exactly that toolchain by the build job."
Forge `[shared]`: `hyper-build-wheels.yml` (new jobs only, own to Python);
`actions/verify-wheels/action.yml` (count, case, upload path); `scripts/release_inventory.py`
(112-117); `scripts/release_package_metadata.py` (265-270, new `sdist()`);
`scripts/release_notes.py` (52, 62); `scripts/hyper_config.py` (docstring, `__init__`, `shell`,
JSON); `hyper-check-core.yml` (new `test-big-endian` job and one input, appended at the end of
the job list, touching no other job); tests `test_hyper_config.py`,
`test_release_inventory.py` (18-file layout; sdist missing, extra, wrongly named, sdist with
the key off), `test_package_inventory_metadata.py` (good sdist; wrong version; no `rust/`),
`test_release_notes.py`, `test_forge_actions.py` for the verify-wheels shape.

## Ordering constraints

Forge first (all Forge files in one merge), then the repo (`hyper.json`, `ci.yml`, docs): with
the key absent the Forge change is inert, so the Forge can merge ahead safely. The README
wording is written last, after the probe leg's result. The Rust MSRV bump to 1.89 (separate
spec) must land before the repo change, or step 4 installs a 1.85 floor and the README's 1.89
is false. No Forge merge while a release is in flight.

## Verification

Local, each with its negative control:
- Forge `python3 -m unittest` in `.github/scripts`: 18-file inventory passes; sdist missing
  with `sdist` on fails; sdist present with it off fails; metadata with the wrong version
  fails; a tarball without `rust/Cargo.toml` fails.
- Repo `.github/scripts/tests` (workflow shapes, not just actionlint): unchanged, still green.
- Local proof above; negative control: remove `rust/`.
- `twine check --strict` on the local sdist; negative control: a scratch copy with a
  malformed README content type fails.

CI-only, by job: `build-sdist` (manylinux2014 install at exactly `rust-version`, and its
1.88 negative control), `sdist-fallthrough` (3.13t, 3.14t, PyPy, non-gating), `test-big-endian`
(s390x under QEMU, and its flipped-byte negative control), `check-msrv` (existing).

Release-time, the first publish of a new artifact shape: the release-mode board is run at the
release commit and must show `build-sdist` green before the tag. The tag run's `pypi-pack`
(`verify-wheels` expecting 18, inventory, package metadata) is the proof the shape is
accepted; then the `workflow_dispatch` publish, then `registry-confirm` for all 18, then by
hand: the sdist's PyPI file page shows the PEP 740 attestation, `pip download hyperuuid==1.1.0
--no-binary :all: --no-deps` returns the file, and `gh attestation verify` on it passes with the
release predicate.

**First-publish risk.** PyPI file names are spent forever: a bad `hyperuuid-1.1.0.tar.gz`
cannot be replaced, only yanked, and 1.1.0 would then have no sdist. What can still go wrong
after the board: PyPI rejecting the file for a reason only it checks. The metadata is the
same pyproject-derived set as the 1.0.0 wheels PyPI accepted, and `twine check --strict` plus
the name check cover the sdist-specific rules, so the residual is small and is **unproven**.
Recovery if the sdist alone fails transiently: re-dispatch from the preserved bytes
(the plan uploads only what is missing). If PyPI refuses the bytes, the sdist ships in 1.1.1.
The owner may add a TestPyPI rehearsal; it needs a Trusted Publisher registered there, and the
spec does not recommend it.

## Nitpick audit

| Hole | Closure |
| --- | --- |
| "Rust >= 1.89" unproven | `build-sdist` installs at exactly `rust-version`; 1.88 must fail |
| "No compiler needed" read as universal | Wording scopes it to platforms with a wheel |
| glibc < 2.28 build unproven | The install runs on manylinux2014 (glibc 2.17) |
| s390x correctness | Core suite and corpus under QEMU; extension labelled not run there |
| FreeBSD, ppc64le | Labelled: compile-checked, not run; FreeBSD maturin caveat stated |
| Free-threaded 3.13/3.14, PyPy | Probe leg with three outcomes. Promotion needs the full suite, the free-threading tests included, and the GIL still off after import |
| Sdist differs from the wheels' source | One build step from the same checkout, same version check |
| Root README said "twenty-fifth wheel" (line 229) while `WHEELS` is 17 | Fixed on sight on 2026-10-28 (now "seventeenth"), ahead of this item |
| PyPI rejects the sdist | Residual, unproven, stated above |
| Fall-through worse than "No matching distribution" | Stated in the README; accepted |
| Siblings lack an sdist | Frozen; not claimed for them |

## Out of scope / Later

Windows ARM or musl behaviour (they have wheels). A FreeBSD or ppc64le CI runner. Running the
PyO3 extension on s390x. HyperCast and HyperTabular sdists, after v1.1 ships and a real release
has carried one (their cores may have dependencies, so the proof does not transfer).

# Part B: PHP 8.3 floor

## Design

Pure floor change. The CI matrix already runs PHP 8.5; the only run on the old floor is the
musl floor leg, whose version is the Forge's `php_floor_version`. No leg disappears and none
is added. `phpunit/phpunit ^11` needs PHP >= 8.2, so there is no dependency conflict. The
floor leg proves the claim: a floor nothing runs is not known to be true.

## Contract changes

Exact lines (all verified 2026-10-28):
- `composer.json:18` (root, what Packagist reads) and `php/composer.json:18`:
  `"php": ">=8.2",` becomes `"php": ">=8.3",`.
- `php/composer.lock:3679` (the `platform` block): regenerated by `composer update --lock`
  in `php/`, never by hand; it also changes `content-hash`. The `"php": ">=8.2"` /
  `"^8.2"` lines at 25, 530 and 626 are dependencies' own requirements and stay.
- `php/README.md:13`: "PHP 8.2 is the floor." becomes "PHP 8.3 is the floor."
- `php/README.md:153`: "**PHP 8.2 or later**, 64-bit." becomes "**PHP 8.3 or later**, 64-bit."
- `README.md:165`: "Python 3.11, Ruby 3.3, PHP 8.2 and Node 22" becomes "... PHP 8.3 and Node 22".
- `README.md:404`: "the 8.2+ binding" becomes "the 8.3+ binding".
- `ROADMAP.md:15` (table, PHP floor column): `8.2` becomes `8.3`.
- **The Forge default stays `"8.2"`** (amended after review, 2026-10-28). Changing it would move
  the frozen siblings' floor leg to 8.3 while they still declare `>=8.2`, which drops their 8.2
  coverage. Repo `ci.yml` passes `php_floor_version: "8.3"` in `build-native`'s `with:`, in the
  same commit as the Composer requirements, so the floor leg runs `php:8.3-cli-alpine`.
  The two cannot land apart: the musl suite runs `composer install`, and under 8.2 that refuses
  a `>=8.3` requirement, which is the negative control below.
- `CLAUDE.md`, "Lists that move together", new bullet: "The PHP floor: `php` in
  `composer.json` and `php/composer.json`, `php_floor_version` in `ci.yml` (an explicit
  input, since the Forge's default serves the siblings), `php/composer.lock` (regenerated with
  `composer update --lock`), and the floor sentences in `php/README.md` and the root README.
  PHP 8.3 reaches EOL on 2027-12-31: then drop it from all of them."
- `CHANGELOG.md`, Unreleased, Changed: "**PHP: the floor is 8.3.** 8.2 reaches end of life on
  2026-12-31. A project on 8.2 keeps resolving 1.0.x. *(Packagist)*" No semver callout.
Left alone: `CHANGELOG.md:1144` (history).

## Files

`[own]` Part B: `php/composer.json`, `php/composer.lock`, `php/README.md`.
`[shared]`: `composer.json` (line 18); `README.md` (165, 404); `ROADMAP.md` (15);
`CLAUDE.md` (one new bullet); `CHANGELOG.md` (one bullet under Changed); `.github/workflows/ci.yml`
(`build-native`'s `with:` gains `php_floor_version: "8.3"`). Forge: none.

## Ordering constraints

One commit in this repo: the Composer requirements, `composer.lock`, the docs and ci.yml's
`php_floor_version: "8.3"`. Split, the floor leg runs `composer install` on 8.2 against
`>=8.3` and fails. No Forge change, so the siblings keep their 8.2 floor leg.

## Verification

Local: in `php/`, `composer update --lock && composer validate`, then `composer test`
against the checkout's own core (`HYPERUUID_NATIVE_LIBRARY`, per CLAUDE.md). Negative control:
under an 8.2 CLI, `composer install` must refuse on the platform check; if it installs, the
floor edit did not take. CI-only: the musl floor leg (job `build-native`'s musl suite for PHP)
on `php:8.3-cli-alpine`; the image is not fetched here and is **unverified** until CI runs it
(the tag family is the same as 8.2's). Release-time: Packagist re-reads `composer.json` on the
tag; check the 1.1.0 page declares `php >=8.3`.

## Nitpick audit

| Hole | Closure |
| --- | --- |
| "Floor 8.3" untested | The floor leg runs 8.3 on Alpine; negative control above |
| 8.2 is supported until 2026-12-31, so early | Owner's decision; stated; 1.0.x remains for 8.2 |
| Stale 8.2 text elsewhere | The line list above comes from a repo-wide grep, 2026-10-28 |
| Phase 2 (ext-php-rs) assumes 8.3-8.5 | Consistent; no conflict |

## Out of scope / Later

Dropping 8.3 at its EOL (2027-12-31), recorded in CLAUDE.md. The sibling floors, when they
thaw. Phase 2's PIE builds.

## Open items

1. **Big-endian leg or label?** Recommended: the real `test-big-endian` leg (Part A). Fallback: label only.
2. **Probe outcome** for free-threaded 3.13/3.14 and PyPy decides the README wording; a pre-tag
   run of `sdist-fallthrough` is required, and no wording is final before it.
