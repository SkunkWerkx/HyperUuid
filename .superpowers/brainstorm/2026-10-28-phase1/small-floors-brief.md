# Design brief: three small Phase 1 floors (Ruby 3.3 ABI, Python sdist, PHP 8.3)

Researched 2026-10-28, read-only. Three separable sub-items, A (Ruby), B (Python), C (PHP). All upstream facts below were checked today.

## 1. Goal

A: Ruby 3.3 users (floor, `ruby/hyperuuid.gemspec:21`) get the Magnus fast path (2.1-5.5x over `SecureRandom.uuid`) instead of Fiddle, until 3.3 dies. B: pip can build hyperuuid from source where no wheel matches (FreeBSD, ppc64le, s390x, glibc older than 2.28), with a Rust toolchain. C: the PHP floor leaves 8.2 before its EOL. Now because 1.1.0 is unreleased and these are floors; each is cheap to do once and expensive to do after a release.

## 2. Current state

**Ruby.** Platform gems are fat over `ABIS = %w[3.4 4.0]` (`ruby/Rakefile:52`); `required_ruby_version` is `>= ABIS.first, < 4.1` (`:91`). The Forge builds one Magnus extension per ABI per platform: primary `ruby_version` 4.0 plus ONE `ruby_compat_version` input (`hyper-build-native.yml:292`), one step pair on the matrix legs (`:1274-1289`) and one on Alpine (`:1666-1681`). Packing reads `RUBY_ABIS` from `hyper_config.py:40` and expects every platform x every ABI (`pack-gems/action.yml:118`, count check `:138`). The wasm gem is packed from the SAME list (`:155-166`), built from `ruby_wasm_abis` (`hyper-build-wasm.yml:107-114`). Uuid's input: `ci.yml:116`. Published 1.0.0 platform gems are 327-360 KB (rubygems.org Content-Length: x86_64-linux-gnu 360,448; aarch64-linux-gnu 346,624; arm64-darwin 327,168; x64-mingw-ucrt 344,064; universal 105,472).

**Python.** `python/pyproject.toml:10-11` (maturin>=1.15), `manifest-path = "../rust/Cargo.toml"` (`:51`). Wheels only: 17 per `WHEELS` (`hyper_config.py:54`), built by `hyper-build-wheels.yml`, attested with `actions/attest-build-provenance` over `python/dist/*.whl` (`:326`), uploaded as `pypi-wheels-{rid}`. `verify-wheels` globs `dist/*.whl`, requires exactly `$WHEELS`, runs `gh attestation verify` per wheel, uploads `pypi-packages` as `dist/*.whl`. Inventory `validate_layout` insists the pypi group is exactly `cfg.wheels` files, each `.whl` (`release_inventory.py:112-117`); `release_package_metadata.py:265-270` rglobs `*.whl` and reads wheel METADATA. Publish is `pypa/gh-action-pypi-publish` in `release.yml:404` (PEP 740 attestations by default under Trusted Publishing). Docs: `python/README.md:40` ("Only wheels are published"), root `README.md:229`, `pyproject.toml:12` description ("no compiler needed to install").

**PHP.** Floor `>=8.2` in `composer.json:18` (root, what Packagist reads) and `php/composer.json:18`; `php/composer.lock:3679` records it as platform; READMEs `php/README.md:13,153`, root `README.md:165,404`. CI: the matrix runs PHP 8.5 (`php_version` default, `hyper-build-native.yml:322`); the only 8.2 run is the musl floor leg, `php_floor_version` default "8.2" (`:360-364`, image `php:${floor}-cli-alpine` at `:1656-1662`). Uuid's `ci.yml` passes neither, so both defaults apply. CLAUDE.md has no PHP-floor entry in "Lists that move together".

## 3. Upstream facts (checked 2026-10-28)

- Ruby 3.3: EOL **2027-03-31**, latest 3.3.13; 3.4 EOL 2028-03-31; 4.0 EOL 2029-03-31. https://endoflife.date/api/ruby.json. This equals the date already in `ruby/README.md:349`.
- Magnus README: "Ruby versions 3.2, 3.3, 3.4, and 4.0 are supported" (https://raw.githubusercontent.com/matsadler/magnus/main/README.md). We pin magnus 0.9.3, rb-sys 0.9.130 (`rust/Cargo.lock`). rb-sys `minimum-supported-ruby-version` 2.7 (https://raw.githubusercontent.com/oxidize-rb/rb-sys/main/data/toolchains.json).
- **No Ruby 3.3 for Windows ARM64.** RubyInstaller lists ARM64 only for 4.0.9-1 and 3.4.12-1; 3.3.13-1 is x64/x86 only (https://rubyinstaller.org/downloads/). setup-ruby's ARM list could not be read (points to `windows-versions.json`): **UNVERIFIED** there, but the installer fact decides it.
- `@ruby/3.3-wasm-wasi` exists on npm (ruby.wasm 2.10.1, modified today), so a 3.3 wasm archive is plausible. Our build of it is **UNVERIFIED**.
- PHP: 8.2 security support ends **2026-12-31**; 8.3 to 2027-12-31; 8.4 2028-12-31; 8.5 2029-12-31 (https://endoflife.date/api/php.json). The ROADMAP claim is right, but 8.2 is still supported today.
- getrandom 0.4.3 defaults: FreeBSD "getrandom"; Linux "getrandom syscall, else /dev/urandom" with the fallback present on s390x and powerpc64 (https://docs.rs/getrandom/0.4.3/getrandom/). No special features needed.
- Rust tiers: x86_64/aarch64-unknown-freebsd, powerpc64le-unknown-linux-gnu, s390x-unknown-linux-gnu are all Tier 2 with Host Tools, full std; Tier 2 means "guaranteed to build", tests not always run (https://doc.rust-lang.org/rustc/platform-support.html).
- maturin 1.15.0 on PyPI has manylinux ppc64le and s390x wheels and an sdist; **no FreeBSD wheel** (https://pypi.org/pypi/maturin/json). A FreeBSD builder needs the `py-maturin` port or builds maturin from source.
- The official publish action attests "by default" for release files "(source and binary distributions)" (https://docs.pypi.org/attestations/producing-attestations/).

## 4. Approaches

### A. Ruby 3.3 in the Magnus grid

- **A1 (recommended if done): add 3.3 to all platforms except win-arm64, Forge-wide.** Forge: a second compat input (or turn `ruby_compat_version` into a space-separated list looped by the action, keeping the "no matrix" rule), 3.3 added to both step pairs; `RUBY_ABIS` default and `ruby_wasm_abis` default gain 3.3; pack-gems learns per-platform ABI sets because aarch64-mingw-ucrt has none for 3.3 (its gem would carry 3.4/4.0, `required_ruby_version >= 3.4`). Rakefile: `ABIS` plus a per-platform exception, `required_ruby_version` computed per platform. Cost: +1 Magnus build/test/attest per platform in release and weekly builds (not PRs, `inputs.mode != 'pr'`): 6 platforms plus 2 musl = +8 jobs' worth of steps, +1 wasm archive. Gem size: each extra ABI adds about one extension (about 310 KB raw for Linux, `rust/Cargo.toml:129`; gem gzip estimate +170 to +180 KB), so 360 KB becomes about 540 KB per platform gem, about +1.1 MB across the six. Lifetime: 5 months (EOL 2027-03-31).
- **A2: 3.3 only on the platform gems, not the wasm gem.** Needs a pack-gems split of the two ABI lists (wasm uses the same `RUBY_ABIS`, `:155`). Saves one wasm archive and a ruby.wasm 3.3 build, costs more Forge logic. Not recommended: more code for a gem that is 5 months of life.
- **A3: do nothing; document that 3.3 runs Fiddle until EOL.** Zero cost, already documented (`ruby/README.md:349,364`). Honest option: 3.3 is the floor for 5 more months only, the Fiddle loss on Linux/macOS is modest (largest on Windows, 4.6-5.9x, and Windows ARM cannot get 3.3 anyway).

Recommendation: **A1 if the owner wants Ruby 3.3 users fast; otherwise A3.** The payoff window (EOL 2027-03-31) is short and the Forge change is the largest of the three items. If A1: drop date **2027-03-31** (record in CLAUDE.md beside the 3.4 line, the Rakefile, `ci.yml` comment, README table). At the drop, remove 3.3 from the same lists plus the compat input; platform gems for later versions raise `required_ruby_version` back to `>= 3.4`.

### B. Python sdist

Verified locally (scratch dir, nothing in the repo touched): `cd python && maturin sdist --out <scratch>` works as is. It produced `hyperuuid-1.0.0.tar.gz`, 125 KB, with `pyproject.toml` at the root, the whole `rust/` crate (src, tests, benches, examples, `Cargo.lock`, `.cargo/config.toml`) and `src/hyperuuid/*` rewritten to `manifest-path = "rust/Cargo.toml"`. The core is the crate being built, not a path dependency, so nothing outside `python/` is missed. Unpacked, `maturin build --release` compiled it in 6 s and produced a wheel. Negative control not run (a build with `rust/` deleted would fail; not needed since the tarball is self-contained). It does carry `php_ext.rs`/`ruby_ext.rs` (harmless, feature-gated).

Why no sdist: the history says it was a stance, not a bug. 0.6.0 (`ca1474a`) README: "Only wheels are published." The musl commit (`4b721d8`) says Alpine "used to mean no install at all" with no sdist; the CHANGELOG (`:780`) records that no sdist meant a missing wheel had nothing to fall back to, and `pyproject.toml:12` promises "no compiler needed to install". No recorded technical blocker. **Behaviour change:** every interpreter with no matching wheel (PyPy, free-threaded 3.13/3.14, old glibc) stops getting "No matching distribution" and starts a source build that needs Rust >= 1.85 (`rust/Cargo.toml` rust-version, edition 2024). PyPy and free-threaded 3.13/3.14 may then fail late, in the build, because PyO3 abi3 does not apply there: **UNVERIFIED**, must be probed before publishing.

Does the core build on the targets? Local `cargo +stable check --release --features python --target <t>` (rustup targets added under `~`) passed for `x86_64-unknown-freebsd`, `powerpc64le-unknown-linux-gnu`, `s390x-unknown-linux-gnu`. Check only: no link, no run. Code facts: the ChaCha20 pool is `cfg(target_os = "linux")` (`entropy.rs:201`) and takes the generic thread-pointer fallback off x86_64/aarch64 (`:358`) so ppc64le/s390x run the pool; FreeBSD takes plain `getrandom::fill` (`entropy.rs:91-99`, fallback `fill_with`). `timestamp`/`v7.rs:505` use `libc::clock_gettime(CLOCK_REALTIME)`, fine on all three; `CLOCK_REALTIME_COARSE` is only in the Linux pool (`:332-338`), fine on ppc64le/s390x Linux. Endianness: byte conversions are explicit `to_be_bytes`/`from_le_bytes` (`sql.rs:61-78`, `v5.rs:83-89`, `v7.rs:137,260`), no transmute or `from_ne`; a static read finds no big-endian hazard, but **no s390x test has ever run**, so runtime correctness on s390x is UNVERIFIED. 64-bit atomics exist on all three.

- **B1 (recommended): sdist built in the existing wheels workflow, shipped with the 17 wheels as the 18th file.** Forge changes: (i) `hyper-build-wheels.yml`: one `maturin sdist` step on one leg (linux-x64), an install test (`pip install` the sdist into a clean venv, import, run a call) and `attest-build-provenance` on it, upload as `pypi-wheels-sdist`; (ii) `verify-wheels`: also expect and `gh attestation verify` the `.tar.gz`, upload it in `pypi-packages`; (iii) `release_inventory.py:112-117` and `release_package_metadata.py:265-270` accept one `.tar.gz` named `{crate}-{version}.tar.gz` and read its `PKG-INFO` (the wheel METADATA parser at `:116-125` is wheel-specific), gated by a new `hyper.json` key (`sdist: true`, default false so siblings are unaffected); (iv) `release_notes.py:62` wording ("every wheel"). Publish (`release.yml:404`) and registry-plan (`release_inventory.py:300-315`, keyed on filename) need no change: PEP 740 attestations cover the sdist automatically. This is the new artifact shape and the largest risk (first-publish, see 7).
- **B2: sdist uploaded separately by hand or a side workflow.** Avoids touching the inventory, but breaks the "recover by re-dispatching preserved bytes" rule and has no attestation path through the inventory. Rejected.
- **B3: skip; document the platforms as unsupported.** Free, but the ROADMAP's reason (s390x banks) is untested either way.

Open design point for B1: pip with an sdist present also hits the Pyodide wheel and `pyproject.toml` classifiers unchanged, but the build backend needs `maturin` itself on s390x/ppc64le (has wheels, ok) and FreeBSD (no wheel; user installs `py-maturin`). Say so in `python/README.md`.

### C. PHP 8.3 floor

Pure floor change plus one CI default: the matrix already runs 8.5, so no leg disappears; the "8.2" musl floor run becomes "8.3" (`php:8.3-cli-alpine` exists as the same family as the current `php:8.2-cli-alpine`: image availability not fetched, **UNVERIFIED**, trivially checked on CI). No CI leg is dropped, one floor leg changes version. phpunit ^11 needs PHP >= 8.2 (`php/composer.json:21`), so no dependency conflict; `php/composer.lock` must be regenerated (platform block and content-hash) with `composer update --lock` in `php/`. Phase 2's PIE/ext-php-rs plan builds 8.3, 8.4, 8.5 only (ROADMAP Phase 2), so 8.3 is already the floor it assumes: it does not change timing, and doing this now makes the later move conflict-free. Timing question is owner's: 8.2 is supported until 2026-12-31, so moving before then is early by policy 1, moving after is required. Recommend moving in the 1.1.0 that ships near the end of the year or when ready, not later than the first release after 2026-12-31.

## 5. Touch list (recommended: A1 or A3 per owner, B1, C)

Marks: `[own]` only this item, `[shared]` another item might touch.

**A (Ruby, A1):**
- `ruby/Rakefile` `ABIS`, per-platform exception, floor comments [own]
- `ruby/README.md` table `:343-349`, `:358-364`, wasm `:431` [own]
- `ruby/hyperuuid.gemspec` comment `:18-21` (floor sentence unchanged) [own]
- `.github/workflows/ci.yml` `ruby_compat_version` + new input, comment `:107-116`, and `ruby_wasm_abis` if overridden [shared]
- `CLAUDE.md` "Ruby ABIs" list (3.3 EOL 2027-03-31 sentence) [shared]
- `CHANGELOG.md` Unreleased entry (RubyGems) [shared]
- Forge: `hyper-build-native.yml` (new input, 2 step pairs, floor-run comments) [shared]; `hyper_config.py` `RUBY_ABIS` and its test `test_hyper_config.py:44` [shared]; `hyper-build-wasm.yml` `ruby_wasm_abis` default [shared]; `actions/pack-gems/action.yml` per-platform ABI logic [shared]; `actions/ruby-magnus*` (only if 3.3 differs) [shared]
- HyperCast and HyperTabular: `ruby/Rakefile` ABIS (`:53`, `:66`), `ci.yml` compat input (`HyperCast:134`, `HyperTabular:90`), each gem README table, CHANGELOG [shared with each other, own to Ruby]

**B (Python, B1):**
- `python/README.md:40`, root `README.md:229`, `python/pyproject.toml:12` description [own except README, shared]
- `CHANGELOG.md` [shared]; `CLAUDE.md` (add: sdist is built in `hyper-build-wheels.yml`; `sdist` key in hyper.json) [shared]
- `.github/hyper.json` (`sdist: true`) [shared]
- Forge: `hyper-build-wheels.yml` [own to Python]; `actions/verify-wheels/action.yml` [shared]; `release_inventory.py`, `release_package_metadata.py`, `release_notes.py`, `hyper_config.py` (new key) [shared]; their tests (`test_release_inventory.py`, `test_package_inventory_metadata.py`, `test_release_notes.py`, `test_hyper_config.py`) [shared]
- Uuid `.github/scripts/tests/test_workflow_contracts.py` only if the pypi jobs change (they should not) [shared]
- Siblings: nothing forced (default off); see 6.

**C (PHP):**
- `composer.json:18`, `php/composer.json:18`, `php/composer.lock` (regenerate) [own]
- `php/README.md:13,153`; root `README.md:165,404` [own / shared]
- `CLAUDE.md` (add a PHP-floor line to "Lists that move together" so this is not rediscovered) [shared]; `CHANGELOG.md` [shared]
- Forge: `hyper-build-native.yml:364` `php_floor_version` default; comment `suite-musl.sh:9` [shared]
- Siblings: `composer.json`, `php/composer.json`, `php/composer.lock`, README floor sentences (HyperCast `README.md:149,248`, `php/README.md:12,48`; HyperTabular `README.md:179,256`, `php/README.md:11`), CHANGELOG [shared with each other]

## 6. Sibling cascade

- **Ruby:** both siblings have the Ruby gem, same `ABIS = %w[3.4 4.0]`, same `ruby_compat_version: "3.4"`. Changing the Forge `RUBY_ABIS` default hits them at the next Forge merge: pack-gems would then demand `ruby-native-*-3.3` artifacts their CI does not build, so their `ci.yml`, Rakefile and wasm default must change in the same window, or Uuid must override via `.github/hyper.json` `ruby.abis` and an input default instead of changing the Forge default (cleaner for a short-lived ABI, but then the three repos differ, against CLAUDE.md's "Floors match... and move together"; the ABI set is not a floor, so this is defensible). Do not merge Forge changes while a release is in flight.
- **Python:** HyperCast and HyperTabular both have `python/pyproject.toml`. HyperTabular's core likely has dependencies (it needs hypercast?: **UNVERIFIED**, not read); the self-contained-sdist proof here does not transfer. Keep `sdist` default-off and port after Uuid proves the shape, per "Prove in HyperUuid, then cascade".
- **PHP:** both siblings have `php/` and `>=8.2`; the Forge default moves for all three at once, so their composer files and READMEs must follow in the same pass (stale `>=8.2` against an 8.3 floor leg is only cosmetic, nothing breaks).

## 7. Verification

**A.** Local: `cd ruby && for v in 3.3 3.4 4.0: RBENV_VERSION=$v bundle exec rake native:dev && bundle exec rspec` (needs rbenv 3.3, **UNVERIFIED** installed); `rake native:gem[x86_64-linux-gnu]` after staging all three ABIs, then `tar -xOf pkg/*.gem data.tar.gz | tar -tz` shows three `lib/hyperuuid/<abi>/` dirs; negative control: remove a staged 3.3 file, `rake native:gem` must abort "not staged". `python3 -m unittest` in `HyperForge/.github/scripts` and `.github/scripts/tests` for the workflow shapes (not just actionlint). CI only: the 3.3 builds on 7 targets including both Alpine images, the wasm 3.3 archive in Node and Chrome, and the Windows x64 gem load against `x64-ucrt-ruby330.dll`. Release-time: the first fat gem with three ABIs, and install on 3.3 via `gem install` from the attested files.

**B.** Local (done above): `maturin sdist`, extract, `maturin build` OK. Remaining local: `python -m venv v && v/bin/pip install <sdist>` then `python -c "import hyperuuid; print(hyperuuid.uuid4())"` and `pytest` from the sdist's source; negative control: an sdist without `rust/` must fail (confirms the test needs Rust); hyper_config/inventory unit tests with an 18-file layout and with a missing sdist when `sdist: true` (must fail). Probe PyPy and `3.13t` against the sdist to learn the failure mode. CI only: sdist build on a pristine manylinux image, FreeBSD/ppc64le/s390x builds (a QEMU `cargo test` leg for s390x is the only big-endian proof; none exists today). Release-time: first-ever sdist in the inventory (new shape: the 18th file and its `PKG-INFO`), `registry-confirm` on it, and the PEP 740 attestation visible on PyPI. Hazard: PyPI file names are immutable; a bad first sdist cannot be replaced, only yanked.

**C.** Local: `cd php && composer update --lock && composer validate`, `composer test` on 8.3 (`HYPERUUID_NATIVE_LIBRARY` per CLAUDE.md); negative control: `composer install` under an 8.2 CLI must now refuse (platform check). CI only: the Alpine `php:8.3` floor leg. Release-time: Packagist re-reads `composer.json` on the tag.

## 8. Risks, ranked

1. B: first publish of a new artifact shape (sdist) through an inventory built only for wheels, immutable on PyPI. Mitigate with the install test in the wheels workflow and a dry unit-test pass of the inventory first.
2. A: Windows ARM has no Ruby 3.3, so the grid is not uniform; the Forge's "every platform x every ABI" logic must change, and that logic is shared with both siblings.
3. B: sdist turns "no matching distribution" into a late source-build failure on PyPy and free-threaded 3.13/3.14 (UNVERIFIED), and the stance "no compiler needed" changes; s390x big-endian runtime is untested.
4. A: payoff is 5 months (3.3 EOL 2027-03-31) for the biggest Forge change here; cost scales with the siblings.
5. C: early by policy (8.2 supported until 2026-12-31); a 1.1.0 floor bump strands 8.2 users on 1.0.x.

## 9. Owner questions

1. Ruby 3.3: (a) skip it and keep Fiddle until EOL (A3), (b) add it everywhere except win-arm64 (A1), (c) add it, win-arm64 included as 3.4/4.0 only but also the wasm gem (A1 with wasm). Recommend (a) given the 5-month life and the Forge size, unless Ruby 3.3 users on Linux are a stated target.
2. If Ruby 3.3 is added: move the Forge default (all three repos together, recommended) or override in Uuid only via `hyper.json`?
3. Python sdist: ship it as the 18th inventory file via B1 (recommended), or skip? And may siblings stay without one until Uuid has released it?
4. Python sdist wording: keep "no compiler needed to install" for wheel users and add "source builds need Rust >= 1.85" (recommended), or drop the sentence?
5. PHP: move the floor in the 1.1.0 release (recommended; matches Phase 2's 8.3-8.5 set), or hold until the first release after 2026-12-31?
