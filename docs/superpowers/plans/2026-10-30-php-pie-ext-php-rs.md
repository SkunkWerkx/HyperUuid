# PHP: ext-php-rs extension installed by PIE — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `ext-ffi` Composer package with an ext-php-rs Zend extension that holds the whole
`HyperUuid\` API, is shipped as 21 attested PIE zips, and leaves no PHP binary in git.

**Architecture:** `rust/src/php_ext/` defines every public class, enum and exception, and calls the
core's Rust API directly. The phpunit suite in `php/` runs against the built extension. The Forge
builds, names (checked against PIE's own matcher), tests and attests one zip per platform and minor at
A, carries them through the inventory as a `php/` group, and attaches them to the GitHub Release.
Afterwards, a PIE install matrix proves the real install path.

**Tech Stack:** Rust stable, ext-php-rs 0.16.3, cargo-php (stubs), PHP 8.3/8.4/8.5 NTS, PHPUnit 11,
PIE (pinned phar), GitHub Actions in the Forge (`../HyperForge`), Python 3 Forge scripts and their
unittest suites.

**Spec:** `docs/superpowers/specs/2026-10-30-php-pie-ext-php-rs-design.md`. Read it first. This plan
argues from it and does not repeat its rationale.

## Global Constraints

- **The owner drives git** (the owner is the person this plan is written for; the executor is the
  agent). The executor never commits, pushes, tags, opens a PR or dispatches a workflow unless the
  owner names that step. These stops are deliberate (`CLAUDE.md`, "Working with the owner"). Every
  "Checkpoint" below means "leave it in the working tree, verified, and report it". Checks therefore
  read the working tree, never the index, until the owner stages the work.
  Before each task, `git fetch` both repos and compare with origin.
- **Outward-facing steps** (the scratch repository, its releases, any dispatch) wait for the owner's
  explicit go, named per step.
- **Order (owner, 2026-10-30):** this plan lands **before** the Go conversion (PHP PR, then CI,
  then Go). So `stage-native-binaries`, `hyper_config` and `release_inventory` still carry Go's
  `go/staticlib` payloads. Task 10 removes only the PHP entries and leaves every Go entry exactly
  as it is.
- PHP floor `>=8.3`. Minors built: `8.3`, `8.4`, `8.5`. NTS only (`support-zts: false`).
- Extension name `hyperuuid`. Composer name stays `skunkwerkx/hyperuuid`, type `php-ext`, with
  `download-url-method: ["pre-packaged-binary"]` only, and
  `os-families: ["linux", "darwin", "windows"]`.
- Unix asset name: `php_hyperuuid-<v>_php<m>-<x86_64|arm64>-<linux|darwin>-<glibc|musl|bsdlibc>-nts.zip`,
  holding `hyperuuid.so`. Windows asset name: `php_hyperuuid-<v>-<m>-nts-<vs16|vs17>-x86_64.zip`,
  holding a DLL named the zip's stem plus `.dll`. `<v>` is the release tag (`v1.1.0` expected; the
  rehearsal proves it).
- **Two version values, never mixed.**
  - `package_version` is bare `X.Y.Z`. It is read at A from `rust/Cargo.toml`'s `[package] version`
    (the value `prepare-release.yml` bumped, and what the staging commit carries). It is what
    `phpversion('hyperuuid')` and `nativeVersion()` return, what `release_inventory.py`'s identity
    and `php_asset_names(version)` take, and what `hyper-pie-acceptance.yml` asserts `phpversion`
    against.
  - `release_tag` is `"v" + package_version`. It is the zip-name version segment (PIE's pretty
    version, to be proved by the rehearsal), the `--version` the packer takes, and the
    `pie install <package>:<release_tag>` constraint.
  - `release_tag` is derived from `package_version`, never typed by hand. The Forge workflow derives
    both in one step and exports them as job outputs. `php_asset_names(version)` adds the `v` itself.
- `RUSTC_BOOTSTRAP=1` is set on the Windows extension build step **only**, with a comment naming
  `abi_vectorcall` and rust-lang/rust#124485. It is never set more widely.
- Public API, defaults and exception messages are byte-for-byte today's (`php/src/*.php` at the
  commit Task 1 starts from). The only removal is `HyperUuid::isAvailable()`.
- `Uuid` keeps PHP's **loose** object comparison over a private readonly `bytes` property. No
  custom compare handler.
- `HyperUuid::newV7(null)` keeps today's "now": `(int) round(microtime(true) * 1000)`, i.e. the
  wall clock rounded to the nearest millisecond, not floored.
- Corpus vectors are add-only. Never hand-edit `include/hyperuuid.h`, versions or `corpus/`.
- No sibling work and no `hyper.json` switches for siblings.
- A verification counts only once its negative control has been seen failing.
- Prove locally first (macOS arm64 natively; Linux glibc and musl in Docker). Windows, osx-x64,
  attestation and release steps are CI's, and the final report says which parts those were.
- If ext-php-rs can't carry an object behaviour (comparison, serialization, clone, the surface),
  **stop and report to the owner**. Don't weaken a test.

## Review Focus

1. **Non-UTF-8 bytes.** `new Uuid(random_bytes(16))` with high bytes, and `newV5(ns, "\xff\xfe\0x")`
   must work and match today. ext-php-rs `String`/`&str` parameters reject invalid UTF-8, so every
   byte parameter must take the zend string's raw bytes. Owner: Task 3 (constructor) and Task 5 (v5).
2. **A fork after first use** (opcache preload in the FPM master, `pcntl_fork`). Two processes
   forked after a v7 was generated must not produce equal v7s in the same millisecond. Owner: Task 5.
3. **DateTimeInterface edges.** Sub-millisecond microseconds, non-UTC zones and pre-1970 instants
   must give today's millis or today's exception. Owner: Task 1 captures the expectations, Task 5
   asserts them.
4. **Hostile `unserialize` payloads.** A wrong value type, a missing key, extra keys, 15 or 17 bytes.
   Each must throw `\InvalidArgumentException` (or PHP's own error) and never crash or leave a
   half-built `Uuid`. Owner: Task 4.
5. **Mutating a `Uuid` from outside.** `$u->bytes = 'x'`, `unset($u->bytes)` and `ReflectionProperty::setValue`
   must fail as today's readonly property does, and `new Uuid()` with no argument must be an
   `ArgumentCountError`. Owner: Task 3.

---

## File Structure

**HyperUuid**

| Path | Responsibility |
| --- | --- |
| `php/tools/capture-v1-fixtures.php` (new) | One-shot script run against today's classes. Writes the parity fixtures. |
| `php/tests/fixtures/v1/` (new) | `surface.json`, `comparisons.json`, `serialized.txt`, `datetimes.json` |
| `rust/src/php_ext/mod.rs` | Module registration, `guard()`, module version, startup hook |
| `rust/src/php_ext/errors.rs` | The three exception classes, and mapping core errors to PHP exceptions |
| `rust/src/php_ext/enums.rs` | `UuidLayout`, `UuidVariant` |
| `rust/src/php_ext/uuid.rs` | The `Uuid` class (inspection, SQL order, text, JSON, serialization) |
| `rust/src/php_ext/generate.rs` | `HyperUuid` (generation, version) and `Namespaces` |
| `rust/src/php_ext.rs` | Deleted (replaced by the directory) |
| `php/tests/*` | Suite against the extension, plus new `SurfaceTest`, `ObjectBehaviourTest`, `SerializationTest`, `AvailabilityTest` |
| `php/tests/bootstrap.php` | Rewritten: requires the extension loaded, nothing else |
| `php/composer.json` | Dev-only project `skunkwerkx/hyperuuid-dev`, with the `composer test` script |
| `composer.json` (root) | `php-ext` package |
| `php/stubs/hyperuuid.stub.php` (new) | Generated by cargo-php, committed |
| `.github/scripts/check_exports.py` (+ its test) | Drop the FFI site; add the `php_guard` rule |
| `.github/scripts/tests/test_workflow_contracts.py` | New PHP pins |
| `.github/workflows/ci.yml`, `release.yml` | Wiring: PR-mode extension legs, post-publish acceptance |
| `.gitattributes`, docs, `CLAUDE.md`, `ROADMAP.md`, `CHANGELOG.md` | Task 12 |
| `php/src/` | Deleted (Task 2) |

**HyperForge**

| Path | Responsibility |
| --- | --- |
| `.github/scripts/php_ext_package.py` (new) + `test_php_ext_package.py` | Name, zip and self-check one extension build |
| `.github/scripts/pie_asset_names.php` (new) | Asks the pinned PIE phar's own matcher for candidate names |
| `.github/workflows/hyper-build-php-extension.yml` | 7 platforms × 3 minors; PR and release modes; pack and attest |
| `.github/workflows/hyper-pie-acceptance.yml` (new) | Reusable PIE install matrix (rehearsal and post-publish) |
| `.github/workflows/hyper-build-native.yml`, `.github/actions/musl-suite/suite-musl.sh` | Drop the ext-ffi suites |
| `.github/scripts/hyper_config.py`, `release_inventory.py`, `release_guard.py`, `release_notes.py` (+ tests) | The `php` asset group; staging stops carrying `php/src/native` |
| `docs/release-runbook.md` | PHP assets, and the patch-release rule |

---

### Task 1: Capture the 1.0 parity fixtures from today's classes

This must run **before** `php/src/` is deleted. These fixtures are the reference for every later
parity test.

**Files:**
- Create: `php/tools/capture-v1-fixtures.php`, and `php/tests/fixtures/v1/{surface.json,comparisons.json,serialized.txt,datetimes.json}`

**Interfaces:**
- Produces: the four fixture files, read by Tasks 3, 4, 5 and 7, plus `provenance.json` (Step 1).
  - `surface.json`: for each class or enum in `HyperUuid\` except `Runtime` and `NativePlatform`,
    `{kind, final, readonly, interfaces[], parent, constants{}, cases{}, methods{name: {static, params:[{name, type, optional, default}], return}}}`.
    The `default` value is the string `ReflectionParameter::getDefaultValueConstantName()`, or
    `var_export(getDefaultValue(), true)`.
  - `comparisons.json`: a list of `{a_hex, b_hex, eq, cmp, equals}`.
  - `serialized.txt`: `serialize(new Uuid(hex2bin('0123456789abcdeffedcba9876543210')))`, raw.
  - `datetimes.json`: a list of `{input, millis | exception_class, message}` for the DateTime cases
    below.

- [ ] **Step 1: Write the capture script.** It autoloads `php/vendor/autoload.php` and loads through
  FFI. `Runtime.php` prefers the committed `php/src/native/` library (the 1.0.0 core) over a fresh
  build, so the script **refuses to run** unless `HYPERUUID_NATIVE_LIBRARY` names an existing file.
  It writes `php/tests/fixtures/v1/provenance.json` with:
  - `library` (that path) and its sha256;
  - `nativeVersion()`;
  - `source_sha` (`git rev-parse HEAD`) and `rust_tree_clean` (whether `git status --porcelain rust/`
    is empty);
  - the PHP version.

  It records:
  - **Surface:** via Reflection. Drop `HyperUuid::isAvailable` and record that it was dropped as
    `"removed": ["HyperUuid::isAvailable"]`.
  - **Comparison pairs:**
    - equal bytes;
    - two different fixed random values;
    - `"0000000000000001"` vs `"1e00000000000000"`;
    - `Uuid::nil()` vs `Uuid::max()`;
    - a v7 vs the same v7 `toSqlOrder()`.
  - **DateTime cases**, each passed to `HyperUuid::newV7`, recording the returned `unixMillis()` or
    the exception:
    - `2026-10-30T12:34:56.789999Z`;
    - `2026-10-30T12:34:56.000500+05:30`;
    - `1970-01-01T00:00:00.000Z`;
    - `1969-12-31T23:59:59.999Z`;
    - `@281474976710` with microseconds `.655000` (the 48-bit top);
    - `@281474976710` with microseconds `.656000` (one past).
- [ ] **Step 2: Run it.** Run `cd rust && cargo cdylib && cd ../php &&
  HYPERUUID_NATIVE_LIBRARY="$PWD/../rust/target/release/libhyperuuid.dylib" php tools/capture-v1-fixtures.php`.
  Expected: the four fixtures plus `provenance.json` are written; `provenance.json` has
  `rust_tree_clean: true` and the library path under `rust/target/release`; and `comparisons.json`
  has the row
  `{"a_hex":"30303030303030303030303030303031","b_hex":"31653030303030303030303030303030","eq":true,"cmp":0,"equals":false}`.
- [ ] **Step 3: Negative controls.**
  - Edit `php/src/Uuid.php`'s `equals` to return `true` and rerun into a scratch directory. The
    numeric pair's `equals` must flip to `true`. Revert, and confirm `git diff php/src` is empty.
  - Run without `HYPERUUID_NATIVE_LIBRARY`. The script must exit nonzero, naming the variable.
- [ ] **Checkpoint.**

### Task 2: Extension skeleton, test harness and the switch-over

**Files:**
- Delete: `rust/src/php_ext.rs`, all of `php/src/`, `php/tests/NativePlatformTest.php`, `php/tests/fixtures/probe.php`
- Create: `rust/src/php_ext/{mod.rs,errors.rs,enums.rs}`, `php/tests/AvailabilityTest.php`, `php/tests/VersionTest.php`
- Modify: `rust/src/lib.rs:144-145` (`mod php_ext;` now a directory), `php/composer.json`, `php/tests/bootstrap.php`, `php/tests/HyperUuidTest.php` (remove the `isAvailable` block and the `probe()` helper, about lines 570–640)

**Interfaces:**
- Produces:
  - `pub(crate) fn guard<T>(f: impl FnOnce() -> PhpResult<T> + UnwindSafe) -> PhpResult<T>` in
    `php_ext/mod.rs`. It wraps `std::panic::catch_unwind` and turns a panic into a
    `\RuntimeException` with the message `"hyperuuid: internal panic: <payload>"`. **Every** PHP-callable
    body in Tasks 3–5 is `guard(|| …)`.
  - `errors.rs`:
    - `#[php_class] #[php(name = "HyperUuid\\CounterExhaustedException", extends(ce = ce::runtime_exception, stub = "\\RuntimeException"))]`,
      likewise `TimestampOutOfRangeException` and `RandomSourceException`, each final.
    - `pub(crate) fn v7_error(e: v7::NewV7Error, op: &str) -> PhpException`, with today's messages
      (from `Runtime.php`):
      - `TimestampOutOfRange` → `TimestampOutOfRangeException("unix_millis must fit within the RFC 9562 48-bit field")`;
      - `CounterExhausted` → `CounterExhaustedException(COUNTER_EXHAUSTED)`;
      - `Random` → `RandomSourceException("{op} failed with code {n}")`, with today's code numbers.
  - `enums.rs`: `UuidLayout` (int-backed, `Rfc9562 = 1`, `SqlServer = 2`) and `UuidVariant`
    (`Ncs = 1`, `Rfc9562 = 2`, `Microsoft = 3`, `Future = 4`).
  - The module version is `env!("CARGO_PKG_VERSION")`, so `phpversion('hyperuuid')` returns it.
  - `php/composer.json`:
    - name `skunkwerkx/hyperuuid-dev`, `require.php` `>=8.3`, no `autoload`;
    - `autoload-dev` for `tests/` and `bench/`;
    - script `"test": "@php tools/run-tests.php"`. That script resolves
      `../rust/target/php/release/libhyperuuid.{so,dylib}` (or `$HYPERUUID_PHP_EXTENSION` when set,
      which CI uses), exports it as `HYPERUUID_PHP_EXTENSION`, and runs
      `PHP_BINARY -d extension=<path> vendor/bin/phpunit` with the remaining arguments, passing the
      exit code through. There's no shell substitution, so it works on Windows.
  - `php/tests/Support/ChildProcess.php`: trait `ChildProcess` with
    `child(array $phpFlags, string $code): string` (runs `PHP_BINARY` with the flags and `-r`, returns
    trimmed stdout, and fails the test on a nonzero exit) and `ext(): string` (reads
    `HYPERUUID_PHP_EXTENSION`). Tasks 4 and 5 use it too.

- [ ] **Step 1: Write the failing tests.**

```php
// AvailabilityTest
public function testLoadedExtensionDefinesTheApi(): void {
    self::assertSame("1 1", self::child(['-d', 'extension=' . self::ext()],
        'echo (int) extension_loaded("hyperuuid"), " ", (int) class_exists("HyperUuid\\HyperUuid");'));
}
public function testWithoutTheExtensionNothingIsDefined(): void {
    self::assertSame("0 0", self::child(['-n'],
        'echo (int) extension_loaded("hyperuuid"), " ", (int) class_exists("HyperUuid\\HyperUuid");'));
}
// VersionTest
public function testPhpversionMatchesNativeVersion(): void {
    self::assertSame(phpversion('hyperuuid'), \HyperUuid\HyperUuid::nativeVersion());
    self::assertMatchesRegularExpression('/^\d+\.\d+\.\d+$/', phpversion('hyperuuid'));
}
public function testEnumsCarryTheCoreCodes(): void {
    self::assertSame(2, \HyperUuid\UuidLayout::SqlServer->value);
    self::assertSame(3, \HyperUuid\UuidVariant::Microsoft->value);
}
```

  Both tests `use ChildProcess`.

  Bootstrap fails fast with `"the hyperuuid extension is not loaded; run composer test"` when
  `!extension_loaded('hyperuuid')`. It requires `vendor/autoload.php` and nothing else.
- [ ] **Step 2: Run them to verify they fail.** Run `cd rust && cargo php-ext && cd ../php && composer test -- --filter 'Availability|Version'`.
  Expected: FAIL, because `HyperUuid\HyperUuid` isn't defined (only the spike functions exist).
- [ ] **Step 3: Implement the skeleton**, plus a `HyperUuid` class with `nativeVersion()` only
  (Task 5 fills the rest).
- [ ] **Step 4: Run them to verify they pass.** Same command. Expected: PASS.
- [ ] **Step 5: Negative control.** Change the module version to a literal `"0.0.0"`. `VersionTest`
  must fail. Revert.
- [ ] **Checkpoint.** The rest of the suite is red until Tasks 3–5 land.

### Task 3: The `Uuid` class (everything except serialization)

**Files:**
- Create: `rust/src/php_ext/uuid.rs`, `php/tests/ObjectBehaviourTest.php`
- Test: `php/tests/InspectionTest.php`, `php/tests/CorpusTest.php` (unchanged), `ObjectBehaviourTest`

**Interfaces:**
- Consumes: `guard`, the enums and the exceptions from Task 2.
- Produces: `HyperUuid\Uuid`, final, implementing `\JsonSerializable` and `\Stringable`, with:
  - `#[php(prop)]` private readonly `bytes: string`, a zend binary string (it backs comparison and
    `var_dump`);
  - `pub(crate) fn from_bytes(b: [u8; 16]) -> Uuid` and `pub(crate) fn as_array(&self) -> [u8; 16]`
    for Tasks 4 and 5.
  - Methods and signatures exactly as in `surface.json` (`version`, `variant`, `isRfc`, `unixMillis`,
    `timestamp(bool $throwOnMismatch = true, UuidLayout $layout = UuidLayout::Rfc9562)`, `toSqlOrder`,
    `fromSqlOrder`, `nil`, `max`, `parse`, `bytes`, `__toString`, `jsonSerialize`, `equals`).
  - Byte parameters are read as raw zend-string bytes (`Binary<u8>` or `&Zval` → bytes), never as
    `String`/`&str`. Messages are copied verbatim from the deleted `Uuid.php` and
    `Runtime::SQL_ORDER_WRONG_VERSION`.

- [ ] **Step 1: Write the failing `ObjectBehaviourTest`.**
  - `testComparisonsMatchV1`: loop over `fixtures/v1/comparisons.json`, and assert `$a == $b`,
    `$a <=> $b` and `$a->equals($b)` equal the recorded `eq`, `cmp` and `equals`.
  - `testCloneIsEqualAndIndependent`: `$c = clone $u; self::assertTrue($c->equals($u)); self::assertNotSame($c, $u);`
  - `testVarDumpShowsBytes`: the `var_dump($u)` output contains `["bytes":"HyperUuid\Uuid":private]`
    and the 16 raw bytes.
  - `testNonUtf8BytesRoundTrip`: `$b = "\xff\xfe\x00\x80" . str_repeat("\xc3", 12); self::assertSame($b, (new Uuid($b))->bytes());`
  - `testReadonlyFromOutside`: `$u->bytes = 'x'`, `unset($u->bytes)` and
    `(new \ReflectionProperty(Uuid::class, 'bytes'))->setValue($u, str_repeat('a', 16))` each throw
    `\Error` (readonly, or "Cannot modify"). `new Uuid()` throws `\ArgumentCountError`.
  - `testWrongLengthMessage`: `new Uuid('short')` throws `\InvalidArgumentException` with today's
    message from `Uuid.php`.
- [ ] **Step 2: Run to verify failure.** Run `composer test -- --filter ObjectBehaviour`.
  Expected: FAIL, because `HyperUuid\Uuid` isn't defined. (`InspectionTest` and `CorpusTest` call
  `newV4`, `newV5`, `newV7`, batches and `Namespaces`, so they gate at Task 5, not here.)
- [ ] **Step 3: Implement `uuid.rs`.** Inspection calls `crate::uuid`, `crate::sql` and
  `crate::timestamp` directly. `timestamp()` builds a `DateTimeImmutable` exactly as the deleted
  `Uuid.php` did (same format string and UTC zone; copy that construction).
- [ ] **Step 4: Run to verify they pass.** Same command. Expected: PASS.
- [ ] **Step 5: Negative control.** Make `bytes` a Rust-only field (drop `#[php(prop)]`). The
  different-bytes rows of `testComparisonsMatchV1` must fail. Restore it. (The corpus negative
  control moves to Task 5.)
- [ ] **Checkpoint.**

### Task 4: `Uuid` serialization

**Files:**
- Modify: `rust/src/php_ext/uuid.rs`, `rust/src/php_ext/mod.rs` (startup hook)
- Create: `php/tests/SerializationTest.php`

**Interfaces:**
- Consumes: `Uuid::from_bytes`, `as_array` (Task 3).
- Produces: `Uuid::__serialize(): array`, returning `["\0HyperUuid\\Uuid\0bytes" => <16 bytes>]`,
  and `Uuid::__unserialize(array $data): void`. At module startup, the `NotSerializable` bit is
  cleared on `Uuid`'s class entry, after ext-php-rs registers the class.

- [ ] **Step 1: Write the failing tests.**
  - `testV1PayloadUnserializes`: `unserialize(file_get_contents(fixtures/v1/serialized.txt))->bytes() === hex2bin('0123456789abcdeffedcba9876543210')`.
  - `testPayloadIsByteIdenticalToV1`: `serialize(new Uuid(hex2bin('0123…3210'))) === file contents`.
  - `testRoundTrip` with random bytes.
  - `testHostilePayloads`: each of these throws `\InvalidArgumentException` or `\TypeError`, and
    never fatals:
    - the bytes key holding an int;
    - the key missing;
    - an extra key;
    - 15 bytes;
    - 17 bytes;
    - `O:14:"HyperUuid\Uuid":0:{}`.

    Run every case in a child process (`ChildProcess::child()` from Task 2), so a crash shows up as a failure,
    not a dead suite.
- [ ] **Step 2: Run to verify failure.** Run `composer test -- --filter Serialization`.
  Expected: FAIL with `Serialization of 'HyperUuid\Uuid' is not allowed`.
- [ ] **Step 3: Implement it.** If `unserialize` can't construct the Rust-backed object before
  `__unserialize` runs (ext-php-rs's `create_object` for this class), **stop and report to the
  owner** with the exact error. Don't ship a workaround that changes the payload.
- [ ] **Step 4: Run to verify they pass.** Same command. Expected: PASS.
- [ ] **Step 5: Negative control.** Change one byte of the fixture in a scratch copy and point the
  test at it via `HYPERUUID_V1_SERIALIZED`. The two V1 tests must fail.
- [ ] **Checkpoint.**

### Task 5: `HyperUuid` generation and `Namespaces`

**Files:**
- Create: `rust/src/php_ext/generate.rs`
- Test: `php/tests/HyperUuidTest.php` (as edited in Task 2), plus new cases in it

**Interfaces:**
- Consumes: `Uuid::from_bytes`, `v7_error`, `guard`.
- Produces: `HyperUuid` (`MAX_V7_BATCH = 1 << 26`, `nativeVersion`, `newV4`,
  `newV5(Uuid $namespace, string $name)`, `newV7`, `newV7Batch`, `newV7BatchBytes`) and `Namespaces`
  (`dns`, `url`, `oid`, `x500`). Signatures are as in `surface.json`.
  - The timestamp union `\DateTimeInterface|int|null` is converted exactly as today:
    - `null` → `round(now_ms_float)`;
    - a `DateTimeInterface` → `getTimestamp() * 1000 + intdiv((int) format('u'), 1000)`, by calling
      those PHP methods on the object;
    - an int passes through.

- [ ] **Step 1: Write the failing new cases.**
  - `testDateTimesMatchV1`: loop over `fixtures/v1/datetimes.json`, asserting the same millis or the
    same exception class and message.
  - `testV5AcceptsNonUtf8Names`: `newV5(Namespaces::dns(), "\xff\xfe\0x")` equals the value computed
    by `uuid_new_v5` through the C ABI. Precompute it with `cargo test` in `rust/` (or the Python
    binding), and write it into the test as a hex literal with a comment saying how it was produced.
  - `testForkAfterFirstUseGivesDistinctV7s` (skipped when `!function_exists('pcntl_fork')`; the CI
    legs install pcntl):
    1. Generate one v7.
    2. `pcntl_fork()`.
    3. Parent and child each generate 1000 v7s at a fixed millisecond `$ms = 1_800_000_000_000`
       and write them to a pipe.
    4. Assert there are no duplicates across the two processes.
- [ ] **Step 2: Run to verify failure.** Run `composer test -- --filter HyperUuidTest`.
  Expected: FAIL, because `newV4` and the others are undefined.
- [ ] **Step 3: Implement `generate.rs`.** Batch counts are checked before allocation with today's
  `v7BatchLimitMessage`, and `count === 0` returns `[]` or `''`.
- [ ] **Step 4: Run the whole suite.** Run `composer test`. Expected: every test PASSES, including
  `InspectionTest`, and `CorpusTest` with all 194 vectors (their first green gate).
- [ ] **Step 5: Negative controls.**
  - Have `newV7` ignore the DateTime microseconds. `testDateTimesMatchV1` must fail. Revert.
  - Flip one byte in one corpus vector (a temporary edit, reverted immediately, with
    `git diff corpus` empty afterwards). `CorpusTest` must fail.
- [ ] **Step 6: Linux proof.** Run the full suite in Docker for glibc (`php:8.3-cli` and
  `php:8.5-cli`) and musl (`php:8.5-cli-alpine`). Build `cargo php-ext` inside each container,
  with phpize deps installed. Expected: PASS on all three.
- [ ] **Checkpoint.**

### Task 6: Panic guard enforcement and the export check

**Files:**
- Modify: `.github/scripts/check_exports.py` (lines 12–13 docstring; remove the `PHP (FFI cdef)` site at about 209–210; add `php_guard`), `.github/scripts/tests/test_check_exports.py`

**Interfaces:**
- Produces: `php_guard(dir: Path)`. Every `fn` inside a `#[php_impl]` block, and every
  `#[php_function]`, in `rust/src/php_ext/*.rs`, must have a body whose first expression is
  `guard(`. Otherwise it exits with
  `check_exports: php_ext has PHP-callable functions without guard: <names>`. It is called from
  `main()` beside `napi_catch_unwind`.

- [ ] **Step 1: Write failing tests** in `test_check_exports.py`, mirroring the napi tests: a
  temporary file with one guarded and one unguarded method → exits naming the unguarded one; an
  all-guarded file → passes; and `main()`'s list of rules includes `php_guard`.
- [ ] **Step 2: Run them.** `python3 -m unittest .github/scripts/tests/test_check_exports.py`. Expected: FAIL.
- [ ] **Step 3: Implement it**, and drop the FFI site.
- [ ] **Step 4: Run.** Run the unittest, then `python3 .github/scripts/check_exports.py`.
  Expected: PASS, and the check passes on the real tree.
- [ ] **Step 5: Negative control.** Remove `guard(` from one method in `uuid.rs`. The real check
  must fail naming it. Restore.
- [ ] **Checkpoint.**

### Task 7: The surface test and the stubs

**Files:**
- Create: `php/tests/SurfaceTest.php`, `php/stubs/hyperuuid.stub.php`
- Test: `php/tests/StubTest.php` (regenerate-and-diff, Linux and macOS only)

**Interfaces:**
- Consumes: `fixtures/v1/surface.json`.
- Produces: the committed stub. The command that regenerates it is recorded in the stub's header
  comment, as cargo-php's `stubs` subcommand on the built library (confirm the flags with
  `cargo php stubs --help`).

- [ ] **Step 1: Write `SurfaceTest::testSurfaceMatchesV1`.** It rebuilds the same JSON shape as
  Task 1 from Reflection on the loaded extension, and asserts it equals `surface.json` **after** two
  declared edits in the test, no others:
  - `HyperUuid::isAvailable` removed;
  - `Uuid::__serialize` (`public`, not static, no params, return `array`) and
    `Uuid::__unserialize` (`public`, not static, one param `array $data`, return `void`) added
    (Task 4).

  Any other addition or removal fails, naming the member. Write `StubTest::testStubIsCurrent`: regenerate into a temporary file and
  `assertFileEquals`. It is skipped on Windows (cargo-php can't run there).
- [ ] **Step 2: Run them.** Run `composer test -- --filter 'Surface|Stub'`.
  Expected: Surface PASSES or names the differing members; Stub FAILS (no file yet).
- [ ] **Step 3: Fix any surface differences in the Rust attributes**: parameter names, defaults,
  `final`, nullable returns. If ext-php-rs can't express a recorded default (for example an enum
  default in arginfo), **stop and report to the owner** with the member. Generate the stub.
- [ ] **Step 4: Run.** Same command. Expected: PASS.
- [ ] **Step 5: Negative controls.**
  - Rename `$throwOnMismatch` in Rust. Surface must fail. Revert.
  - Add a stray public method `Uuid::debug()` in Rust. Surface must fail, naming it. Remove it.
  - Edit one docblock line in the stub. Stub must fail. Regenerate.
- [ ] **Checkpoint.**

### Task 8: The package metadata, the archive and the contract tests (HyperUuid)

**Files:**
- Modify: `composer.json` (root), `.gitattributes`, `.github/scripts/tests/test_workflow_contracts.py` (the PHP floor test at about lines 227–234, plus new pins)

**Interfaces:**
- Produces:
  - The root `composer.json` exactly as in the spec's "The package" block, keeping description (with
    "FFI" wording replaced by "a PHP extension"), keywords (drop `ffi`, add `extension`), license,
    authors, homepage and support.
  - `.gitattributes` export-ignores every top-level path except `composer.json`, `LICENSE`,
    `README.md` and `go/`, and its header comment is rewritten (spec: The package).

- [ ] **Step 1: Write the failing contract tests.**
  - `test_root_composer_is_a_pie_extension` asserts:
    - `type == 'php-ext'`;
    - `php-ext.extension-name == 'hyperuuid'`;
    - `download-url-method == ['pre-packaged-binary']`;
    - `support-zts is False`;
    - `os-families == ['linux','darwin','windows']`;
    - no `autoload` and no `require.ext-ffi`.
  - `test_git_archive_is_metadata_only`: build a real archive,
    `git archive --worktree-attributes --format=tar HEAD`, and read its members with Python's
    `tarfile`. Git applies directory exclusion itself (an export-ignored `rust` drops `rust/Cargo.toml`,
    which a per-file `check-attr` would miss), and `--worktree-attributes` applies the edited
    `.gitattributes` without a commit. Assert that the set of top-level member names is
    `{composer.json, LICENSE, README.md, go}`. Today the set also has `.claude`, `.editorconfig`,
    `CHANGELOG.md`, `CLAUDE.md`, `CRAN_CHECKLIST.md`, `ROADMAP.md`, `c`, `corpus`, `docs`, `include`
    and `php`, so the deny-list must add all of them, plus any other top-level path the test names.
  - `test_no_php_sources_in_working_tree`: `(ROOT / 'php' / 'src').exists()` is false. This reads
    the working tree, since deletions stay in the index until the owner stages them. The staged-tree
    form is not a test; it is the owner's `git status` at commit time.
  - The "no `php/src/native` reference in any workflow" pin is **not** added here. It lands in
    Task 12, after Tasks 10–11 have removed the staging references (Forge), and after this repo's
    `stage-native-binaries.yml` header is rewritten.
  - The existing floor test keeps passing (both composer files keep `require.php >=8.3`).
- [ ] **Step 2: Run them.** `python3 -m unittest discover .github/scripts/tests`. Expected: the new tests FAIL.
- [ ] **Step 3: Edit `composer.json` and `.gitattributes`.**
- [ ] **Step 4: Run.** Same command, and `composer validate composer.json`. Expected: PASS.
- [ ] **Step 5: Negative control.** Remove `/rust export-ignore`. The archive test must fail, naming
  `rust`. Restore.
- [ ] **Checkpoint.**

### Task 9: Forge — name, zip and self-check, against PIE's own matcher

**Files:**
- Create: `../HyperForge/.github/scripts/php_ext_package.py`, `../HyperForge/.github/scripts/pie_asset_names.php`, `../HyperForge/.github/scripts/test_php_ext_package.py`

**Interfaces:**
- Produces:
  - `pie_asset_names.php <pie.phar>`. It is run with the target PHP, `require`s the phar's autoloader,
    builds a `TargetPlatform` for the running PHP, and prints the candidate names one per line from
    `Php\Pie\Platform\PrePackagedBinaryAssetName::packageNames()` (and, on Windows, PIE's Windows
    DLL-name class) for a stub `Package` named `hyperuuid` at the version given in `argv[2]`. If a
    class can't be loaded, it exits 2 with `"pie_asset_names: PIE internals changed: <class>"`.
  - `php_ext_package.py pack --library <built .so|.dll> --php <php binary> --pie <phar> --version <tag> --out <dir>`.
    It computes the name (Unix: from `php -r` facts; Windows: `PHP Extension Build` from `php -i` for
    the compiler) and writes the zip with exactly one entry. It then runs `verify`, and prints the
    zip path.
  - `php_ext_package.py verify <zip> --php <php> --pie <phar> --version <tag>`. It fails unless:
    1. there is exactly one entry;
    2. the entry is `hyperuuid.so` on Unix, or `<zip stem>.dll` on Windows;
    3. the zip name is in `pie_asset_names.php`'s output;
    4. the version segment equals `--version`.
  - The PIE pin is a constant `PIE_VERSION = '<x.y.z>'` (the latest at implementation time) with
    the phar's sha256 beside it. The phar's attestation is verified by
    `gh attestation verify --owner php` in the workflow, not here.

- [ ] **Step 1: Write the failing unit tests** (pure Python, PIE stubbed by a fake
  `pie_asset_names` output):
  - a correct Unix zip passes;
  - a musl build named `glibc` → fails, naming check 3;
  - a Windows zip with inner DLL `php_hyperuuid.dll` → fails, naming check 2;
  - a two-entry zip → fails, naming check 1;
  - a version segment `1.1.0` against tag `v1.1.0` → fails, naming check 4.
- [ ] **Step 2: Run them.** `cd ../HyperForge && python3 -m unittest .github/scripts/test_php_ext_package.py`. Expected: FAIL (no module).
- [ ] **Step 3: Implement both scripts.**
- [ ] **Step 4: Run.** Same command. Expected: PASS.
- [ ] **Step 5: Real PIE, locally.** Download the pinned PIE phar to the scratchpad, verify its sha256
  and attestation, and run `pack` on the macOS arm64 build from Task 5 against PHP 8.5. Expected:
  `php_hyperuuid-v1.1.0_php8.5-arm64-darwin-bsdlibc-nts.zip` (or the name PIE actually lists, which
  goes in the report), and verify passes. Rename it to `…-glibc-…` and run `verify`. It must fail.
- [ ] **Checkpoint.**

### Task 10: Forge — the build matrix, and dropping the ext-ffi suites

**Files:**
- Modify: `../HyperForge/.github/workflows/hyper-build-php-extension.yml`, `hyper-build-native.yml` (PHP ext-ffi sections around 1255–1330 and the header notes at 14–125; `build-php-extension` call at about 1973), `.github/actions/musl-suite/suite-musl.sh`, `.github/actions/musl-native` if it stages PHP, `.github/scripts/hyper_config.py` (staged native payloads: drop `php/src/native/*`), `hyper-stage-natives.yml`, and the Forge contract tests that pin any of these

**Interfaces:**
- Consumes: `php_ext_package.py` (Task 9).
- Produces: workflow `hyper-build-php-extension.yml` with inputs `crate`, `mode`
  (`pr`|`release`), `php_versions` (default `"8.3 8.4 8.5"`), `php_floor_version`, `attest_native`.
  A first job, `version`, reads `package_version` from `rust/Cargo.toml` at the checked-out commit A
  (`cargo metadata --no-deps --format-version 1` → `.packages[] | select(.name == crate) | .version`).
  It fails unless the value is bare `X.Y.Z`, and outputs `package_version` and
  `release_tag = "v" + package_version`. Every leg `needs: version`. The load check asserts
  `phpversion('hyperuuid') == package_version`. The packer gets `--version ${{ needs.version.outputs.release_tag }}`.
  Per leg it uploads artifact `php-ext-<rid>-<minor>` holding one zip.
  - Matrix:
    - `pr`: every platform × newest minor, plus linux-x64 × floor.
    - `release`: 7 platforms × 3 minors.
  - Platforms and how each gets PHP:
    - linux-x64, linux-arm64: Ubuntu 26.04 and setup-php;
    - linux-musl-x64, linux-musl-arm64: Alpine `php8x-dev`, following the Magnus musl pattern;
    - osx-x64, osx-arm64: the `shivammathur/php` tap `php@8.x`;
    - win-x64: setup-php with an NTS build, and ext-php-rs's devel pack.
  - Each leg runs these steps in order:
    1. build (`cargo rustc --release --crate-type cdylib --features php`);
    2. the glibc floor guard (Linux glibc);
    3. the load check, which records `PHP Extension Build` and the API number;
    4. `composer install` and phpunit in `php/` with `-d extension=`;
    5. stubs diff (Linux x64 newest only);
    6. `php_ext_package.py pack`;
    7. attest (`release` mode);
    8. upload.
  - Windows build step only: `RUSTC_BOOTSTRAP: "1"` with the comment from Global Constraints.

- [ ] **Step 1: Write failing Forge contract tests** (in the Forge's existing workflow-contract
  test module):
  - the matrix yields 21 `(rid, minor)` release pairs and the expected PR pairs;
  - the packer's `--version` comes only from `needs.version.outputs.release_tag`, and the load check
    compares against `package_version`;
  - `RUSTC_BOOTSTRAP` appears exactly once in this workflow, on a step whose `if` is Windows-only;
  - phpunit runs on every leg;
  - `hyper-build-native.yml` and `suite-musl.sh` contain no `ext-ffi`/`ffi.enable`/`phpunit`;
  - `hyper_config`'s staged payloads contain no `php/src/native`.
- [ ] **Step 2: Run them.** Run the Forge test discovery the repo uses
  (`python3 -m unittest discover .github/scripts`). Expected: FAIL.
- [ ] **Step 3: Edit the workflows and `hyper_config.py`.** Run `actionlint` on every changed
  workflow.
- [ ] **Step 4: Run.** Same commands. Expected: PASS, and actionlint is clean (Mac llvm noise is
  environmental, per the memory note).
- [ ] **Step 5: Negative controls.**
  - Add `RUSTC_BOOTSTRAP` to the Linux build step. The contract test must fail. Revert.
  - The load check must refuse a foreign minor. Locally in Docker, build the extension in
    `php:8.4-cli`, then run the leg's load-check command in `php:8.5-cli` against it. It must exit
    nonzero with PHP's "Module compiled with module API=…" message. Record the output in the report.
    This is what makes the per-leg load check worth trusting.
- [ ] **Checkpoint.**

### Task 11: Forge and HyperUuid — the `php/` inventory group, the release attach, and post-publish acceptance

**Files:**
- Modify: `../HyperForge/.github/scripts/release_inventory.py` (`RELEASE_ASSET_GROUPS`, `BUILT_AT_SOURCE`, the group validation at about 145–150), `release_guard.py` (collect the `php-ext-*` artifacts from the CI run into `php/`, as it collects the Swift assets), `release_notes.py` (list the PHP assets), their tests, `docs/release-runbook.md`
- Create: `../HyperForge/.github/workflows/hyper-pie-acceptance.yml`
- Modify: `.github/workflows/ci.yml` (pass `mode` and the PHP inputs through), `.github/workflows/release.yml` (new last job `pie-acceptance`, `needs: finalize-release`), `.github/RELEASING.md`, `.github/scripts/tests/test_workflow_contracts.py`

**Interfaces:**
- Consumes: the artifacts `php-ext-<rid>-<minor>` (Task 10).
- Produces:
  - In `release_inventory.py`:
    - `RELEASE_ASSET_GROUPS = ('github', 'c', 'php')`;
    - `BUILT_AT_SOURCE` gains `'php/'`;
    - a function `php_asset_names(version) -> list[str]`. It takes the identity's bare `version`,
      builds the 21 names with `release_tag = 'v' + version` in the version segment (Global
      Constraints formulas; compiler `vs16` for 8.3, `vs17` otherwise), and the validation requires
      `groups['php']` to equal it exactly.
  - Workflow `hyper-pie-acceptance.yml`, with **both** `on: workflow_call` (for `release.yml`) and
    `on: workflow_dispatch` (for the rehearsal), declaring the same inputs under each:
    - `package` (Composer name);
    - `package_version` (bare `X.Y.Z`; the legs derive `release_tag` from it the same way);
    - `repository` (empty means Packagist; otherwise the `vcs` URL);
    - `minors`.

    Its matrix is 7 platforms × minors. Each leg starts from a clean container or runner with PHP
    only, then:
    1. fetches the pinned PIE and verifies its attestation;
    2. adds the repository when it's given;
    3. runs `pie install <package>:<release_tag>`;
    4. in a fresh `php` (no `-d`), asserts `extension_loaded`, `phpversion('hyperuuid') === package_version`, a v4, the v5
       vector `newV5(Namespaces::dns(), 'example.com')` as the corpus records it, a v7 and a SQL order
       round trip.
  - `release.yml`'s `pie-acceptance` calls it with `package: skunkwerkx/hyperuuid` and
    `package_version` from `find-ci-run`'s release identity.

- [ ] **Step 1: Write failing tests.**
  - `test_release_inventory.py`: a payload missing one PHP zip → `ValueError` naming it; an extra
    `php/` file → `ValueError`; a zip named with a bare `1.1.0` segment → `ValueError`; the exact 21
    (`v1.1.0` segments) → passes.
  - The Forge contract test: `hyper-pie-acceptance.yml` declares both `workflow_call` and
    `workflow_dispatch`, with identical input names.
  - `test_release_guard.py`: the PHP artifacts are copied into `php/`.
  - The HyperUuid contract test: `release.yml` has a `pie-acceptance` job needing
    `finalize-release`, calling `hyper-pie-acceptance.yml@master`.
- [ ] **Step 2: Run them.** Run both repos' unittest discovery. Expected: FAIL.
- [ ] **Step 3: Implement it**, and write the runbook paragraph: a PIE acceptance failure after
  publish is fixed with a patch release, never a rename, repack or re-attest (spec, Testing).
- [ ] **Step 4: Run.** Same commands, plus actionlint. Expected: PASS.
- [ ] **Step 5: Negative control.** Put `vs17` for 8.3 in `php_asset_names`. The exact-21 test must
  fail against a payload built with the correct names. Revert.
- [ ] **Checkpoint.**

### Task 12: Docs, receipts and the changelog

**Files:**
- Modify: `CHANGELOG.md` (Unreleased), `php/README.md` (rewrite), `README.md` ('Platform support', install table), `docs/platforms.md`, `docs/benchmarks.md`, `.github/RELEASING.md`, `CLAUDE.md`, `ROADMAP.md`, `.github/workflows/stage-native-binaries.yml` (header comment), `php/bench/*` (benchmarks run against the extension)

- [ ] **Step 1: CHANGELOG.** Add the breaking entry with the spec's content (Docs and receipts).
  Leave a marked slot, `<!-- owner's words -->`, for the owner's apology; never write it for them.
- [ ] **Step 2: PHP README** (current state only). It covers:
  - `pie install skunkwerkx/hyperuuid` (no version pinned in the snippet; `<version>` where one is
    shown);
  - the platform × minor table;
  - `gh attestation verify <zip> --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github`;
  - stubs for PhpStorm and PHPStan `stubFiles`;
  - NTS only;
  - `extension_loaded('hyperuuid')`;
  - "`==` on two `Uuid`s is PHP's loose property comparison; use `equals()`";
  - serialization compatibility with 1.0;
  - the WebAssembly recipe pointing at the shipped extension.

  Remove `ffi.enable`, preload, `HYPERUUID_NATIVE_LIBRARY`, the spike section and its unreproducible
  table. Re-run every example by hand against the built extension.
- [ ] **Step 3: CLAUDE.md.**
  - "Never hand-edit" drops `php/src/native/`.
  - The PHP floor list gains `php_versions` (Forge default) and the extension matrix.
  - The local loop becomes `cargo php-ext`, then `composer test` in `php/`.
  - The core freshness check and `HYPERUUID_NATIVE_LIBRARY` lines go.
  - "Toolchains and floors" names the Windows PHP `RUSTC_BOOTSTRAP=1` exception beside Blazor's.
  - The export list drops the PHP FFI cdef site, and the `php_guard` rule goes in beside napi/JNI.
- [ ] **Step 4: ROADMAP.** Mark Phase 2 PHP done (dated), update the summary table's PHP row, and
  update `docs/platforms.md` and `RELEASING.md` (21 PHP assets on the Release, PIE acceptance).
- [ ] **Step 5: Benchmarks (owner's machine).** On the XPS 15 9510 in WSL2, run phpbench in `php/`
  against the extension (`XDEBUG_MODE=off`, the existing 5 × 1000 shape), and replace the PHP tables
  in `php/README.md` and `docs/benchmarks.md`, dated and machine-labelled, with losses printed. If
  the XPS isn't available, leave the old tables in place, marked "predates the extension". Never
  publish numbers from another machine.
- [ ] **Step 6a: The deferred pin.** Add `test_no_php_native_references`: no file under
  `.github/workflows/` or `.github/scripts/` (this repo) mentions `php/src/native`, except the test's
  own source file, which is excluded by path. Run it; expected
  PASS. Negative control: add the string to a workflow comment, it must fail, then remove it.
- [ ] **Step 6: Verify the docs.** Scan only the maintained, user-facing docs: `README.md`,
  `php/README.md`, `docs/platforms.md`, `docs/benchmarks.md`, `.github/RELEASING.md`, `CLAUDE.md` and
  the Forge's `docs/release-runbook.md`. Running
  `grep -n "php/src\|ext-ffi\|ffi.enable\|HYPERUUID_NATIVE_LIBRARY\|isAvailable"` over those files
  must return nothing. One exception: the PHP README's single sentence saying `isAvailable()` was
  replaced by `extension_loaded('hyperuuid')`, if the rewrite keeps one; list any hit like that in
  the report, with its reason. The scan deliberately leaves out evidence and history:
  - `CHANGELOG.md` and `r/NEWS.md`;
  - `docs/superpowers/` (specs, plans, spikes);
  - `php/tools/capture-v1-fixtures.php` (it needs `HYPERUUID_NATIVE_LIBRARY`);
  - `php/tests/fixtures/v1/` (the surface records `isAvailable` as removed);
  - test sources.
- [ ] **Checkpoint.** Report the whole tree to the owner: what passed locally, what only CI can
  prove (Windows, osx-x64, attestation, the inventory at A/B), and the negative controls seen failing.

### Task 13: The installation rehearsal (owner-gated, before 1.1.0 is tagged)

Every step that creates or publishes something waits for the owner's named go.

- [ ] **Step 1 (owner's go: "create the scratch repo").** Create a public `SkunkWerkx/hyperuuid-pie-rehearsal`
  holding only a `composer.json`: the root block with name `skunkwerkx/hyperuuid-pie-rehearsal` and
  the same `php-ext` section.
- [ ] **Step 2 (owner's go: "dispatch release-mode CI on the PR branch").** Run
  `gh workflow run ci.yml --repo SkunkWerkx/HyperUuid --ref <PR branch>`. `ci.yml`'s
  `workflow_dispatch` takes no inputs; only `pull_request` runs are `pr` mode, so a dispatched run is
  release mode. Its `version` job reports `package_version` and
  `release_tag`, and the 21 zips come out as artifacts named with `release_tag`. The Forge is
  called at master, so the Forge half of this work (Tasks 9–11) must be **merged to Forge master**
  first; that merge is the owner's.
- [ ] **Step 3 (owner's go: "rehearsal release").** Tag the scratch repo with exactly that
  `release_tag` (lightweight, unsigned), and attach the 21 zips unchanged from the run's artifacts
  (`gh release create <release_tag> --repo SkunkWerkx/hyperuuid-pie-rehearsal <zips>`).
- [ ] **Step 4 (owner's go: "dispatch PIE acceptance").** Run `gh workflow run hyper-pie-acceptance.yml
  --repo SkunkWerkx/.github --ref master -f package=skunkwerkx/hyperuuid-pie-rehearsal
  -f package_version=<package_version> -f repository=https://github.com/SkunkWerkx/hyperuuid-pie-rehearsal
  -f minors="8.3 8.4 8.5"`. That's the reviewed workflow at the Forge commit merged in Step 2; record
  that SHA in the report. Expected: all 21 legs green.
- [ ] **Step 5.** Record the names PIE actually matched (from the leg logs) against Task 9's pinned
  expectations. On any difference, fix Task 9, 10 or 11 and repeat from Step 2. Never rename assets
  by hand.
- [ ] **Step 6.** Report to the owner, with links to the run.
