# PHP: an ext-php-rs extension installed by PIE (design, 2026-10-30)

Step 4 of the owner's v1.1 sequence (after the Go conversion, before the TypeScript wasm
backend), and the ROADMAP's Phase 2 PHP entry. One PR, CI green before the next step.

## Goal

`pie install skunkwerkx/hyperuuid` installs an attested, prebuilt Zend extension on every
supported platform, with no git checkout, no compiler and no `ext-ffi`. No PHP binary is left in
git. The suite and the corpus pass against the extension on every leg.

## Decisions (owner, 2026-10-30)

- **The break is accepted.** `skunkwerkx/hyperuuid` becomes `type: php-ext`. `composer require`
  stops working in 1.1, and no deprecated FFI package is kept. The CHANGELOG and the PHP README say
  so plainly; the user base is thought to be near zero.
- **The whole API lives in the extension** (approach A). A thin extension plus a PHP library
  package (approach B) was rejected: it means two installs, two version streams that must match,
  and it keeps the ~100 ns PHP-level wrapper the spike measured.
- **Windows is kept** with `RUSTC_BOOTSTRAP=1` on the Windows extension build step only.
  ext-php-rs needs the unstable `abi_vectorcall` on Windows
  ([rust-lang/rust#124485](https://github.com/rust-lang/rust/issues/124485), open), and PIE
  requires prebuilt DLLs there. This is the second named exception beside the Blazor archive, on
  the same terms: commented where it is set, never set more widely, removed when the ABI
  stabilizes. Today's FFI package works on win-x64, so dropping Windows would have been a
  regression.
- **No source fallback.** `download-url-method` is `["pre-packaged-binary"]` alone. With no
  matching zip (FreeBSD, ZTS, a PHP minor not built yet) PIE stops with its own "no matching
  asset" error, and `pie build` fails too. There is no `config.m4` shim driving cargo.

## Facts this rests on (checked 2026-10-30)

- PIE 1.5.5 (default branch `1.6.x`) matches assets in `Platform/PrePackagedBinaryAssetName.php`:
  `php_{ext}-{version}_php{X.Y}-{arch}-{os}-{libc}[-debug][-zts|-nts].{zip|tgz}`. `{arch}` is
  `x86_64` or `arm64`, `{os}` is `linux`, `darwin` or `windows`, and `{libc}` is `glibc`, `musl` or
  `bsdlibc`. Linux also accepts `anylibc`. On Windows the name is
  `php_{ext}-{tag}-{X.Y}-{ts|nts}-{compiler}-{arch}.zip`, holding a DLL of the same name.
- `{version}` is Composer's `getPrettyVersion()`. With our `vX.Y.Z` tags that is expected to be
  `v1.1.0`, the same string PIE uses to look up the Release by tag. **The installation rehearsal
  (Testing) proves it** before 1.1.0 is tagged.
- PIE verifies only its own phar (`src/SelfManage/Verify/`). It does not check attestations on
  extension downloads (issue #565 was closed with no change). Verification is the user's
  `gh attestation verify`, which the README documents.
- A `php-ext` package installs the binary only, and PIE adds no PHP sources to any autoloader.
  Hence approach A.
- ext-php-rs 0.16.3 supports classes, enums, interfaces, and final and readonly classes. It
  generates stubs with `cargo php stubs`, which does not work on Windows.

## The package

**Root `composer.json`** (what Packagist serves; same name, same repo root):

```json
{
    "name": "skunkwerkx/hyperuuid",
    "type": "php-ext",
    "require": { "php": ">=8.3" },
    "php-ext": {
        "extension-name": "hyperuuid",
        "support-nts": true,
        "support-zts": false,
        "download-url-method": ["pre-packaged-binary"],
        "os-families": ["linux", "darwin", "windows"]
    }
}
```

It keeps the description, keywords, license, authors, homepage and support fields. `ext-ffi`,
`autoload`, `autoload-dev` and `require-dev` go. The `ffi` keyword goes, and `extension` comes in.

**`php/composer.json`** becomes the dev project only. It is never published, and its name
(`skunkwerkx/hyperuuid-dev`) can't collide. It holds phpunit, phpbench, phpcs and `ramsey/uuid`
for the comparison benchmarks, autoloads `tests/` and `bench/`, has no `src/`, and has a
`composer test` script that runs phpunit with `-d extension=` pointing at the local build.

**`.gitattributes`:** PIE still fetches the dist zipball for metadata, so it should be small. The
deny-list grows to cover `/php`, `/c`, `/corpus`, `/docs`, `/include` and every other top-level
path except `composer.json`, `LICENSE`, the READMEs and `go/`. `go/` stays off the list for the
reason the file already gives (the Go module proxy archives it). The header comment changes to
say what Composer no longer needs. The `*.so`/`*.dll` lines stay for anything binary that
remains; the sentence about committed PHP libraries goes.

## The extension (`rust/src/php_ext.rs`)

The spike's six `hyperuuid_native_*` functions go. The module defines, in the `HyperUuid\`
namespace, with today's names, signatures, defaults and messages:

- `HyperUuid` (final, static only): `MAX_V7_BATCH = 1 << 26`, `nativeVersion()`, `newV4()`,
  `newV5(Uuid, string)`, `newV7(DateTimeInterface|int|null = null)`,
  `newV7Batch(int, DateTimeInterface|int|null = null): array`, and
  `newV7BatchBytes(int, DateTimeInterface|int|null = null): string`.
- `Uuid` (final, `JsonSerializable`, `Stringable`), holding 16 bytes on the Rust side:
  `__construct(string)`, `parse`, `bytes`, `version(UuidLayout = Rfc9562)`, `variant`,
  `isRfc(int, UuidLayout = Rfc9562)`, `__toString`, `jsonSerialize`, `equals`,
  `unixMillis(UuidLayout = Rfc9562)`,
  `timestamp(bool throwOnMismatch = true, UuidLayout layout = Rfc9562)`, `toSqlOrder`,
  `fromSqlOrder`, `nil`, and `max`.
- `Namespaces` (final): `dns`, `url`, `oid`, and `x500`.
- `UuidLayout: int` (`Rfc9562 = 1`, `SqlServer = 2`), and
  `UuidVariant: int` (`Ncs = 1`, `Rfc9562 = 2`, `Microsoft = 3`, `Future = 4`).
- `CounterExhaustedException`, `TimestampOutOfRangeException` and `RandomSourceException`, each
  a final `\RuntimeException`.

It calls the core's Rust API directly, not the C ABI, as PyO3 and Magnus do. The module version is
the crate version, so `phpversion('hyperuuid')` and `nativeVersion()` agree. It splits into
`rust/src/php_ext/` (module, `Uuid`, generation, errors) if one file stops being readable.

**Removed from the public API:** `HyperUuid::isAvailable()`, since without the extension the class
doesn't exist; `extension_loaded('hyperuuid')` replaces it. Also removed: `HYPERUUID_NATIVE_LIBRARY`
and every `ext-ffi`, `ffi.enable` and preload concern.

**Errors:** the classes and messages stay. `\InvalidArgumentException` stays for wrong-length bytes,
a malformed `parse()`, an out-of-range batch count and a wrong-version SQL order conversion. Wrong
argument types become PHP's own `TypeError` from the declared signature. Negative millis or counts
are refused with today's messages. A 32-bit PHP is unsupported, as today. Every PHP-callable body
runs inside `catch_unwind`, so a Rust panic becomes an exception instead of killing the worker.
`check_exports.py` enforces that, as it does for napi and JNI.

**`Uuid` as a PHP object behaves as it does today.** Today's `Uuid` is a plain PHP object with one
`private readonly string $bytes`, so PHP's generic object machinery works on it, and the extension
keeps all of it:

- `==`, `!=`, `<=>`, `<` and `>` keep PHP's standard object comparison over the `bytes`
  property, exactly as today. That comparison is **loose**: PHP compares property values with `==`,
  numeric-string coercion included, so `new Uuid("0000000000000001") == new Uuid("1e00000000000000")`
  is true today and `<=>` gives 0. It is kept by the parity rule, with no custom compare handler;
  strict bytewise comparison would be a new behaviour change, and the owner can choose it instead.
  `equals()` remains the bytewise check. The README lists `equals()` today but never says why. The
  rewrite adds that `==` on two `Uuid`s is PHP's loose property comparison and not an identity test.
  ext-php-rs's `compare` handler refreshes only fields exposed as PHP properties, then runs the
  standard compare (`zend/handlers.rs`). A Rust-only field would make every two UUIDs `==`, so the
  16 bytes are exposed as a private readonly property named `bytes`. That alone preserves today's
  semantics, and it keeps `var_dump`, `print_r` and `(array)` output as it is.
- `serialize`/`unserialize` round-trip, and a payload written by 1.0
  (`O:14:"HyperUuid\Uuid":1:{s:21:"\0HyperUuid\Uuid\0bytes";s:16:"…";}`, which sits in sessions and
  caches today) unserializes under 1.1, and vice versa. ext-php-rs marks every Rust-backed class
  `NotSerializable` (`builders/class.rs`, "disable serialization if the class has an associated
  object"). The extension clears that flag on `Uuid`'s class entry after every module's startup
  (a chained `zend_post_startup_cb`), with a `dl()` RINIT fallback, and implements
  `__serialize`/`__unserialize` with the same mangled key. `__unserialize` validates the length,
  refusing anything but 16 bytes as the constructor does.
- `clone` gives an equal, independent value.

If ext-php-rs cannot carry one of these (for example, if `unserialize` cannot build the Rust object
before `__unserialize` runs), the implementation stops and the owner decides between a workaround
and a documented break. None of them breaks silently.

**State:** the v7 counter is per process. Each FPM worker has its own, exactly as with FFI.

**Stubs:** `php/stubs/hyperuuid.stub.php` is generated by `cargo php stubs` and committed.
CI regenerates it on Linux and diffs, the way `include/hyperuuid.h` is checked. The README tells
PhpStorm and PHPStan (`stubFiles`) users where to point.

**Deleted:** all of `php/src/` (11 PHP files, and `native/` with its README), and
`php/tests/NativePlatformTest.php`. The `PHP (FFI cdef)` site in `check_exports.py` goes, and its
docstring stops calling the extension a spike that is not checked.

## Build, attest and release (the Forge)

**Assets** (21 for 1.1.0, NTS only):

| Platform | Name (`<v>` is the pretty version, `<m>` 8.3, 8.4 or 8.5) | Holds |
| --- | --- | --- |
| linux-x64, linux-arm64 | `php_hyperuuid-<v>_php<m>-{x86_64,arm64}-linux-glibc-nts.zip` | `hyperuuid.so` |
| linux-musl-x64, linux-musl-arm64 | `php_hyperuuid-<v>_php<m>-{x86_64,arm64}-linux-musl-nts.zip` | `hyperuuid.so` |
| osx-x64, osx-arm64 | `php_hyperuuid-<v>_php<m>-{x86_64,arm64}-darwin-bsdlibc-nts.zip` | `hyperuuid.so` |
| win-x64 | `php_hyperuuid-<v>-<m>-nts-{vs16 for 8.3, vs17 for 8.4 and 8.5}-x86_64.zip` | `php_hyperuuid-<same>.dll` |

The darwin libc string and the Windows compiler tags are confirmed by the installation rehearsal
(Testing) too.

**`hyper-build-php-extension.yml`** grows from three legs to seven platforms × three minors:

- glibc: Ubuntu runners with setup-php, under the same "nothing newer than GLIBC_2.34" guard the
  core has.
- musl: Alpine with `php8x-dev`, following the Magnus musl pattern.
- macOS x64 and arm64: `shivammathur/php`'s `php@8.x` tap, since Homebrew core carries only the
  newest.
- Windows: ext-php-rs's devel-pack fetch, with `RUSTC_BOOTSTRAP=1` on the build step alone and the
  comment saying why.

Each leg builds, load-checks, records the PHP API number it is pinned to, runs phpunit against
itself, zips under the PIE name, and attests build provenance (as the extension already is).

**Pack-time name check, before anything is attested.** A Forge script (tested like
`c_package.py`) is the only thing that names and zips the extension, and it checks its own output
before the attest step:

- Each zip has exactly one entry: `hyperuuid.so` on Unix, or on Windows a DLL whose name is the zip's
  stem plus `.dll`.
- The zip name is one PIE itself would accept on that leg. The expected names come from PIE's own
  code, never from the helper that wrote the zip, so a naming bug can't vouch for itself. The leg
  fetches a pinned PIE release, verifies the phar's attestation, and asks PIE's own matcher for the
  candidate names for the PHP it built against. That is `PrePackagedBinaryAssetName::packageNames()`
  on Unix, and the Windows asset-name equivalent, both loaded from the phar. The packed name must be
  in that list.
  - These classes are marked `@internal`, so a PIE upgrade can break the probe. The PIE pin moves
    by hand, and every move reruns the probe and its negative controls. A probe that can't load the class
    fails loudly and never passes by default.
  - Where loading from the phar turns out impossible, the fallback is a committed table of names
    captured from real PIE resolves during the rehearsal, never names re-derived by our code.
- The version segment equals the release tag.

A mismatch fails the leg, so a misnamed zip never reaches the attestation or the inventory.
Negative controls, each seen failing before the check is trusted:
- a zip renamed with the wrong libc (`glibc` given to a musl build);
- a Windows zip whose inner DLL name differs from its stem;
- a zip holding two entries.

**Modes:** in `pr` mode every leg builds and tests the newest PHP, and one Linux leg also tests the
8.3 floor, so phpunit runs against the extension on every PR. In release mode the full matrix
builds and packs all 21 zips.

**Release:** a `php` group joins `RELEASE_ASSET_GROUPS` in `release_inventory.py`, built at A in the
ci.yml run like `github/` (`BUILT_AT_SOURCE`). The release attestation covers the 21 files, and the
finalizer attaches them to the GitHub Release, where PIE looks. Packagist sees the tag at once, so
`pie install` fails between the tag push and `release.yml` finalizing, the same window Swift's
remote zips have.

**Removed:** `stage-native-binaries.yml` stops staging `php/src/native/`; after the Go step it writes
only `Package.swift`. The ext-ffi suite sections in `hyper-build-native.yml` (desktop and musl
suite list) and the macOS Homebrew `--with-ffi` check go. `php_floor_version` stays as the floor
minor. There are no sibling knobs (siblings are frozen and get the full conversion later).

**Contract tests:** `test_workflow_contracts.py`'s PHP floor test keeps checking both composer
files' `require.php`. New pins cover the 21-name matrix, the single `RUSTC_BOOTSTRAP=1` site per
exception, and the absence of any `php/src/native` path.

## Testing

- `InspectionTest` and `CorpusTest` (194 vectors) run unchanged against the extension, loaded with
  `-d extension=`. `HyperUuidTest` runs unchanged except for its availability block. The four
  `isAvailable` tests, the FFI-disabled, no-FFI and missing-library subprocesses, the `probe()`
  helper and `tests/fixtures/probe.php` are replaced by two subprocess checks:
  - With the extension loaded (`-d extension=<build>`), `extension_loaded('hyperuuid')` is true and
    `HyperUuid\HyperUuid` exists.
  - Without it (`-n`, with no ini), `extension_loaded('hyperuuid')` is false and
    `class_exists('HyperUuid\HyperUuid')` is false: what the README tells users to probe.
  `NativePlatformTest` is deleted with `NativePlatform`.
- New tests:
  - Object behaviour, pinned to today's class rather than to an ideal. Before `php/src/` is
    deleted, a script run against today's classes writes a committed fixture of `==`, `<=>` and
    `equals()` results for a fixed set of pairs. The set includes equal bytes, different random
    bytes, and the numeric-looking pair `0000000000000001`/`1e00000000000000` (`==` true, `<=>` 0,
    `equals()` false). The extension must reproduce every row. The test also covers `clone` being
    equal, `var_dump` showing the bytes, and a `serialize`/`unserialize` round trip. A committed fixture holding a 1.0-format serialized
    `Uuid` (made with today's PHP classes, before they are deleted) unserializes to the same bytes,
    and the 1.1 payload is byte-identical to it. `unserialize` of a payload with 15 bytes throws.
  - `phpversion('hyperuuid') === HyperUuid::nativeVersion()`.
  - A reflection test that pins the public surface (classes, methods, parameter names, types and
    defaults, constants, enum cases), so the extension can't drift from the README.
  - The stub regenerate-and-diff check.
- Negative controls, each seen failing before it is trusted:
  - `CorpusTest` with one vector byte flipped.
  - The load check given an extension built for another minor (8.4 into 8.5 must be refused).
  - The reflection test with one parameter renamed.
  - The stub diff with one docblock edited.
  - The comparison fixture with the `bytes` property made Rust-only (the different-bytes rows must
    then fail).
  - The 1.0 serialized fixture with one byte changed (it must then fail).
- **Installation acceptance: PIE itself installs the packaged zips.** The tests above load a build
  directly, and the naming proof only shows that PIE picks a name. Neither shows that PIE downloads,
  extracts, installs and enables the finished zip. PIE finds binaries only through GitHub's
  `/releases/tags/{version}` API (`Downloading/GithubPackageReleaseAssets.php`), so this test needs a
  real Release. It runs twice, on every platform and minor:
  1. **Rehearsal, before 1.1.0 is tagged.** The PR's release-mode packing produces the 21 zips under
     a rehearsal version. A dispatchable workflow attaches them to a release of a scratch public
     repository holding only `composer.json` (the same `php-ext` block, a rehearsal name). Each
     platform leg then starts from a clean environment: a fresh container on Linux, and a fresh
     runner with only PHP on macOS and Windows. Each leg runs these steps:
     - Fetch the PIE phar and verify its attestation.
     - `pie repository:add vcs <scratch repo>`.
     - `pie install <rehearsal package>`.
     - In a fresh `php` process with no `-d` flags, assert that `extension_loaded('hyperuuid')` is
       true, `phpversion('hyperuuid')` equals the rehearsal version, and that a v4, a v5 vector, a
       v7 and a SQL order round trip work.

     Creating the scratch repository and its releases publishes, so each step takes the owner's go.
     This rehearsal is also where the naming proof's strings (pretty version, `bsdlibc`, `vs16` and
     `vs17`) are confirmed.
  2. **After every publish.** `release.yml` ends with the same matrix against Packagist and the real
     Release (`pie install skunkwerkx/hyperuuid:<version>`). A failure goes red. **The fix is a
     patch release, never a repair in place.** Renaming an asset would leave the inventory's
     recorded paths wrong. A wrong name inside a Windows zip can't be fixed without repacking,
     which changes the digest the release attestation covers. Rebuilding or re-attesting a
     published version is what `RELEASING.md` already forbids. Naming errors are meant to be
     caught before that point, by the rehearsal and by the pack-time check under "Build, attest
     and release".
- Formatters: phpcs on `php/tests` and `php/bench`, and rustfmt and clippy on `php_ext`
  (features `php`).
- Local proof first: macOS arm64 native, Linux glibc and musl in Docker. Windows, osx-x64 and the
  attest and release steps are CI's. The PR report says which parts those were.

## Docs and receipts

- **CHANGELOG (Unreleased):** a plain breaking entry. The package is now an extension installed with
  `pie install`; `composer require`, `ext-ffi`, `ffi.enable`/preload, `HYPERUUID_NATIVE_LIBRARY` and
  `isAvailable()` are gone (use `extension_loaded('hyperuuid')`); NTS only, PHP 8.3 to 8.5, seven
  platforms. The apology goes in the owner's own words, supplied at drafting time.
- **PHP README:** rewritten to the current state. It covers `pie install` with `<version>`, the
  platform and minor table, verifying a zip with `gh attestation verify`, stubs, and ZTS not shipped.
  It has the new benchmark tables with losses printed (batch was a wash in the spike). The WebAssembly
  recipe points at the shipped extension. Every example is re-run by hand against the built
  extension.
- **Elsewhere:**
  - The root README ('Platform support', the install table).
  - `docs/platforms.md`, `.github/RELEASING.md` and the Forge's `docs/release-runbook.md`, where
    they mention staged PHP binaries.
  - `stage-native-binaries.yml`'s header comment.
  - `.gitattributes`.
- **CLAUDE.md:**
  - "Never hand-edit" loses `php/src/native/`.
  - The PHP floor list gains the extension matrix's minors.
  - The local loop's PHP line becomes `cargo php-ext`, then `composer test` in `php/`; the core
    freshness check and `HYPERUUID_NATIVE_LIBRARY` go.
  - "Toolchains and floors" names the Windows PHP `RUSTC_BOOTSTRAP=1` exception beside Blazor's.
- **ROADMAP:** the Phase 2 PHP entry is marked done, and Phase 2's end state is reached once Go's
  step has landed. The PHP row in the summary table changes.
- **Benchmarks:** `docs/benchmarks.md` and the PHP README tables are re-measured on the XPS 15 9510
  (i9-11900H) in WSL2, the machine the other tables use, and dated. The spike's comparison script was
  never committed, so a phpbench harness against the extension is committed in `php/bench/`.

## Out of scope

ZTS builds (added when someone asks; FrankenPHP is the likely first), a source-build fallback,
PECL, a deprecated FFI package, Windows ARM64 (PHP ships none), 32-bit PHP, and any sibling work.

## Risks

- **The pretty-version string in asset names.** If Packagist's pretty version and the asset name
  disagree, every install fails. PIE uses the same string to look up the Release by tag
  (`/releases/tags/{version}`), which suggests it is the tag name itself, `v1.1.0`. The rehearsal
  proves it, and a contract test pins it afterwards.
- **ext-php-rs and PHP object semantics.** Clearing `NotSerializable` and exposing `bytes` as a
  property work around library defaults. A later ext-php-rs release could change either. The object
  behaviour tests are the guard.
- **macOS 8.3 and 8.4 from a third-party tap.** If `shivammathur/php` lags or breaks, those legs go
  red. The fallback is building PHP headers from source for the osx legs.
- **`RUSTC_BOOTSTRAP` on Windows.** No stability promise. Every stable bump is re-proven by the
  Windows legs, as with Blazor.
- **Load-time ABI pinning.** A zip built against one minor is refused by another. The load check
  in every leg is the guard, and its negative control proves it.
