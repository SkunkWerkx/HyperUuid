# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![Packagist](https://img.shields.io/packagist/v/skunkwerkx/hyperuuid.svg)](https://packagist.org/packages/skunkwerkx/hyperuuid)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

**PHP core has no built-in UUID generation at all — nothing beyond the optional PECL `uuid` extension. This package needs zero Composer dependency, not even `ramsey/uuid` — just PHP's own built-in `FFI` extension.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, calling
directly into the native `libhyperuuid` shared library via PHP's built-in
[`FFI`](https://www.php.net/manual/en/book.ffi.php) extension — dlopen/dlsym plus a raw C-ABI
call, no runtime bridge, no Composer dependency beyond `ext-ffi` itself. PHP 8.2 is the
floor. Bundles a native build for every supported platform (see
[Requirements](#requirements)) and picks the right one at runtime — Composer has no
per-platform native selection, so the package carries them all and resolves at load.

```php
use HyperUuid\HyperUuid;
use HyperUuid\Namespaces;

$id = HyperUuid::newV4();
$id2 = HyperUuid::newV5(Namespaces::dns(), 'example.com');
$id3 = HyperUuid::newV6();
$id4 = HyperUuid::newV7();

$id4->timestamp(); // recover the embedded UTC DateTimeImmutable
$id4->timestamp(throwOnMismatch: false); // null instead of throwing if $id4 isn't an RFC 9562 v6/v7
$id4->isRfc(7); // true: the RFC 9562 variant and version 7, in one native call
$id4->toSqlOrder(); // byte order SQL Server's uniqueidentifier needs to sort by creation order

// One native call, one random-bytes fetch, one counter reservation for the whole batch:
$batch = HyperUuid::newV7Batch(1000);
```

Returns `HyperUuid\Uuid`, a minimal value object (`->bytes()`, `->__toString()`,
`->version()`, `->variant()`, `->isRfc()`, `->equals()`; see
[Inspecting a UUID](#inspecting-a-uuid)) — this package has no runtime dependency on
`ramsey/uuid`. It casts to the hyphenated string and `json_encode`s as that same string;
`Uuid::parse()` reads the 8-4-4-4-12 hyphenated form in either letter case and nothing
else, the same rule the Rust core applies. `Namespaces::dns()`/`url()`/`oid()`/`x500()` are RFC 9562 Section 6.6's
well-known namespaces. `->timestamp()` recovers the embedded UTC `DateTimeImmutable` from an
RFC 9562 version 6 or 7 UUID; pass `throwOnMismatch: false` to get `null` back for anything
else instead of throwing. `newV6()`/`newV7()` also accept a `DateTimeInterface` directly in place
of a raw millisecond count. `->toSqlOrder()`/`->fromSqlOrder()` convert a version 6 or 7 UUID to and
from the byte order SQL Server's `uniqueidentifier` needs on the wire to sort by creation
order (`toSqlOrder()` dispatches on the UUID's own version, matching `timestamp()`'s
convention) — computed once in the native Rust core rather than reimplemented in PHP, and
verified there (and independently against the real `System.Data.SqlTypes.SqlGuid` comparator
in the C# binding's test suite). Same-millisecond v6 UUIDs aren't guaranteed to sort correctly
afterward — v6 has no counter, so `clock_seq`/`node` (not the timestamp) decide ties, the same
pre-existing RFC 9562 v6 limitation plain order already has. `fromSqlOrder()` reads
which version to invert from the SQL-ordered bytes in the native core (the same check as
`->version(UuidLayout::SqlServer)`, which never confuses the two), or takes an explicit
`$version` argument when you already know it. `Uuid::nil()`/`Uuid::max()`
are the RFC 9562 §5.9/§5.10 special-value UUIDs.
`HyperUuid::newV6Batch(count)`/`newV7Batch(count)` generate `count` UUIDs sharing one
timestamp capture and one native call, instead of `count` of each.

## Inspecting a UUID

Every read below is one call into the native core; the binding does no bit-reading of its own.

```php
use HyperUuid\UuidLayout;
use HyperUuid\UuidVariant;

$id->version();                  // the RFC 9562 version nibble, 0-15 (0 for Uuid::nil(), 15 for Uuid::max())
$id->variant();                  // UuidVariant::Rfc9562 for anything this package or ramsey/uuid mints
$id->isRfc(7);                   // the RFC 9562 variant and version 7: the guard before trusting v7 fields
$id->unixMillis();               // the embedded Unix-epoch milliseconds of an RFC 9562 v6/v7, or null

$sql = $id->toSqlOrder();
$sql->version(UuidLayout::SqlServer);           // 7, read in place from the SQL Server bytes
$sql->isRfc(7, UuidLayout::SqlServer);          // true
$sql->unixMillis(UuidLayout::SqlServer);        // the same milliseconds, no conversion back first
$sql->timestamp(layout: UuidLayout::SqlServer); // the same DateTimeImmutable
```

`UuidVariant` is an int-backed enum of RFC 9562 §4.1's four variants, `Ncs` (1, includes
Nil), `Rfc9562` (2), `Microsoft` (3) and `Future` (4, includes Max); the values are the native
core's codes, so `Rfc9562`'s is the `0b10` the field's top two bits hold. `version()` says
nothing about the variant, which is what `isRfc($version)` adds. `unixMillis()` and
`timestamp()` check it too, in the same native call that reads the timestamp: a 6 or 7 nibble
under another variant has no RFC version, so it has no timestamp (`null`, or the throw). A `$version` outside 0-15 is
never matched; it's `false`, not an error.

`UuidLayout` says which byte order a `Uuid` holds: `Rfc9562` (1, the default everywhere) or
`SqlServer` (2, what `toSqlOrder()` returns and a `uniqueidentifier` column stores).
`version()`, `isRfc()`, `unixMillis()` and `timestamp()` all take one. SQL Server order is
defined for versions 6 and 7 only: there `version()` answers 6, 7, or 0 for anything that
isn't a SQL-ordered RFC 9562 v6 or v7, `unixMillis()` answers null, and `timestamp()` throws
(or returns null with `throwOnMismatch: false`). The version nibble lands at a different byte for
each, and the other version's random bits can mimic it there, so the core checks the variant
bits too, where each version puts them, and never confuses the two.

The layout is the caller's to know, not something a value reveals: carry it with the value,
as a column type already does, and pass it at every call site that can see a SQL-ordered
value. Without a layout argument, `version()` and `isRfc()` read RFC order, so
`$sqlOrdered->isRfc(7)` reads the wrong bytes; there is deliberately no layout-agnostic check.
`variant()` is RFC order only, with no layout parameter, because in SQL Server order the
variant sits at a different byte for each version: don't feed it a `toSqlOrder()` result or a
`uniqueidentifier` read-back. `isRfc($version, UuidLayout::SqlServer)` is the guard for those;
it checks the variant where that version puts it.

## Requirements

- **PHP 8.2 or later**, 64-bit.
- **`ext-ffi`**, loaded and permitted for your SAPI — see [Enabling FFI](#enabling-ffi)
  below. No other extension and no Composer dependency.
- **A supported platform.** The package bundles one native library per platform and picks
  at load:

  | Platform | Bundled library |
  | --- | --- |
  | Linux x64 / arm64, glibc 2.34 or newer | `linux-x64`, `linux-arm64` |
  | Linux x64 / arm64, musl (Alpine) | `linux-musl-x64`, `linux-musl-arm64` |
  | macOS x64 / arm64 | `osx-x64`, `osx-arm64` |
  | Windows x64 | `win-x64` |

  glibc 2.34 means Debian 12, Ubuntu 22.04, RHEL 9, Amazon Linux 2023 or newer; an older
  glibc fails when the library loads. musl is detected from the running process, so an
  Alpine image needs nothing extra. Windows on ARM hardware loads the x64 library, because
  PHP itself is an x64 process there — PHP has never shipped a native Windows ARM64 build.
  Anything else (a 32-bit PHP, another architecture, another OS family) is a clear
  unsupported-platform error rather than a wrong-library load.

### Enabling FFI

`ext-ffi` ships with PHP, but the `ffi.enable` ini setting decides who may use it, and its
default is `preload`:

| `ffi.enable` | CLI | Web SAPIs (FPM, Apache, `php -S`) |
| --- | --- | --- |
| `preload` (the default) | works | works only from preloaded code |
| `1` | works | works |
| `0` | refused | refused |

So the CLI works out of the box, and a web SAPI needs one of two things in `php.ini`
(`ffi.enable` is a system-level setting — `ini_set()` and per-directory overrides cannot
change it):

- `ffi.enable=1`, which permits FFI to every script the server runs; or
- keep the default and preload this package, which permits FFI to it alone:

  ```ini
  opcache.preload=/path/to/your/preload.php
  ; opcache.preload_user=www-data   ; required when the server starts as root
  ```

  ```php
  // preload.php
  foreach (glob(__DIR__ . '/vendor/skunkwerkx/hyperuuid/php/src/*.php') as $file) {
      opcache_compile_file($file);
  }
  ```

  PHP has no preloading on Windows; use `ffi.enable=1` there.

Without either, the first call throws `FFI\Exception: FFI API is restricted by "ffi.enable"
configuration directive`. Under a web SAPI the library is bound once per request — PHP's
statics reset between requests — while the operating system keeps it mapped for the
worker's lifetime.

## Checking availability

`HyperUuid::isAvailable()` answers whether the native library can be used at all, and never
throws: a missing `ext-ffi`, an `ffi.enable` that restricts FFI for this SAPI, a missing or
unloadable library, an unsupported platform, and a stale library lacking a symbol this
binding declares all answer `false`. It attempts the same load every generator makes and
caches the answer for the request, so it is what a consumer with a fallback gates on:

```php
$id = HyperUuid::isAvailable()
    ? (string) HyperUuid::newV7()
    : $fallback->uuid7();
```

`HyperUuid::nativeVersion()` returns the loaded library's own `"major.minor.patch"` — a
zero-argument probe the core exports, so a host can prove the `libhyperuuid` it resolved is
the one this binding was written against before minting the first UUID. It throws exactly
where `isAvailable()` answers `false`. If you catch instead of asking, catch `\Throwable`:
ext-ffi reports its own failures as `\Error`s (`FFI\Exception` extends `\Error`, and a
missing extension is a plain `Error: Class "FFI" not found`), which `catch (\Exception)`
does not see.

Setting the `HYPERUUID_NATIVE_LIBRARY` environment variable to a path makes the binding load
that library in place of the bundled one. It is there for running the suite against a core
built from a checkout of this repository (`.github/scripts/local-core.sh` builds one and
prints the command), and `nativeVersion()` reports whichever library answered.

## Bulk generation into bytes

`newV6BatchBytes` and `newV7BatchBytes` return the batch as one binary string of raw RFC 9562-ordered bytes — 16 per UUID — instead of an array of `Uuid` objects:

```php
$bytes = HyperUuid::newV7BatchBytes(1000);
$first = substr($bytes, 0, 16);   // ready for a BINARY(16) bind parameter
```

**About 16x faster than `newV7Batch`** for a 1000-UUID batch (4.2 µs versus 67.1 µs), and 64x faster than a thousand `newV7()` calls. The native call is identical in both; `newV7Batch` simply allocates 1000 `Uuid` objects and 1000 substrings on top of it.

The catch, and it inverts the advice: **if you need `Uuid` objects, keep using `newV7Batch`.** Slicing these bytes into objects yourself just relocates the identical allocations into your own code, and measures no better — sometimes worse. Reach for the byte form only when bytes are the destination: a bind parameter, a wire format, a bulk load.

Slice it with `substr($bytes, $i * 16, 16)` — which is exactly what `newV7Batch` does internally.

A batch count below 0 is an `InvalidArgumentException` on all four batch methods; 0 returns
an empty array or an empty string. Version 6 takes up to 4294967295. A version 7 batch takes
at most `HyperUuid::MAX_V7_BATCH` (67,108,864, the 26-bit counter that orders UUIDs within a
millisecond); a larger count is an `InvalidArgumentException` naming the limit, thrown before
anything is allocated. Every version 7 batch is in strictly increasing order: the counter is
one process-wide sequence, so a batch can straddle the point where it wraps back to 0, and
the UUIDs from there on carry the supplied timestamp plus one millisecond rather than sorting
before the ones ahead of them. That orders one batch, not the stream: the next batch or
`newV7()` call in the same real millisecond starts its counter just past the wrap and carries
the supplied timestamp, so it sorts before the previous batch's tail, stamped a millisecond
later. Two single `newV7()` calls either side of the wrap in one millisecond sort in reverse
the same way. It happens at most once per 67,108,864 UUIDs the process mints.

## Why not `ramsey/uuid`?

`ramsey/uuid` is the de facto PHP standard, and it's genuinely a solid library — it supports v6 and v7 with its own monotonic-generation story too. This package isn't claiming to out-generate it; the real differentiators are elsewhere:

1. **Zero Composer dependency.** This package needs nothing beyond PHP's own built-in `FFI` extension — no `ramsey/uuid`, no `composer require` at all beyond this package itself. If you're already pulling in `ramsey/uuid` for something else, that's a fine reason to stick with it; if not, this avoids adding it just for ID generation.
2. **Batch generation.** `newV6Batch(count)`/`newV7Batch(count)` share one timestamp capture, one random-bytes fetch, and (v7) one counter reservation across the whole batch — one native call instead of `count` separate ones.
3. **Cross-language consistency.** The same Rust core mints v5 namespace UUIDs for Python, Go, C#, Ruby, and every other binding in this repo — verified in CI to match Python's own `uuid.uuid5` byte-for-byte. A pure-PHP library, however good, can't structurally guarantee that against a codebase written in a different language.
4. **`timestamp()` isn't tied to how the UUID was minted.** It's a plain RFC 9562 bit-layout read, verified (in this package's own test suite) to correctly extract from a `ramsey/uuid`-generated v6 or v7 value too, not just this package's own — so you can keep `ramsey/uuid` for generation and still get this package's (faster, see below) extraction on its output.

## Benchmarks

Real numbers, measured with [PHPBench](https://phpbench.readthedocs.io/) on linux-x64 (an
Intel Core i9-11900H), PHP 8.5 (`XDEBUG_MODE=off vendor/bin/phpbench run --report=aggregate
--retry-threshold=5`, mode across 5 iterations × 1000 revs each, repeated until the
iterations agree within 5%; a loaded Xdebug inflates everything ~14x uniformly, hence
`XDEBUG_MODE=off`). PHP core has
nothing to compare against — the honest baseline here is a naive inline v4 built from
`random_bytes(16)` with no FFI call at all, to isolate what the FFI boundary itself
actually costs:

| Call | Time | vs. naive inline (no FFI) |
| --- | --- | --- |
| Naive inline v4 (`random_bytes`, no RFC validation) | 308ns | — |
| `newV4()` | 224ns | **1.4x faster** |
| `newV6()` | 255ns | **1.2x faster** |
| `newV7()` | 265ns | **1.2x faster** |
| `newV5()` | 502ns | 1.6x slower — a SHA-1 the baseline has no equivalent of |

Read that top row again: the full RFC-complete v4 — real entropy, correct version and
variant bits, crossing into native code and back — costs **less than the naive pure-PHP
three-liner that doesn't even validate anything**, and the time-ordered versions beat it
too. How far ahead depends on the machine, because most of the naive version is
`random_bytes` and the operating system prices that: where the system random source is
slow the gap is wider. The calls are this cheap because there is no wrapper left around
the boundary: a single static out-buffer and zero-copy `const char *` string passes
(inputs cross as plain PHP strings, no copy at all).

Batch generation amortizes the per-call cost — one crossing instead of a thousand — and
the [byte form](#bulk-generation-into-bytes) then skips building a thousand `Uuid` objects,
which is most of what is left:

| Call | Individual × 1000 | Batch → `Uuid[]` | Batch → bytes |
| --- | ---: | ---: | ---: |
| v6 | 280.9µs | 67.5µs (**4.2x**) | 4.5µs (**62x**) |
| v7 | 267.0µs | 67.1µs (**4.0x**) | 4.2µs (**64x**) |

### Timestamp extraction vs. `ramsey/uuid`'s `getDateTime()`

`ramsey/uuid` has real extraction logic of its own (`UuidInterface::getDateTime()`, works for
both `UuidV6` and `UuidV7`), so this is a genuine head-to-head, not a strawman — each call
measured against a UUID generated once outside the timed loop, so only the extraction itself
is timed. Unlike generation, where PHP's `FFI` boundary was the honest cost, extraction
flips the result:

| Call | Time | vs. `ramsey/uuid` |
| --- | ---: | ---: |
| `->timestamp()` (v6) | 274ns | **88x faster** |
| `ramsey/uuid`'s `->getDateTime()` (v6) | 24.0µs | baseline |
| `->timestamp()` (v7) | 293ns | **45x faster** |
| `ramsey/uuid`'s `->getDateTime()` (v7) | 13.2µs | baseline |

`ramsey/uuid`'s `getDateTime()` does real work this package's native extraction doesn't have
to: parsing a lazily-decoded UUID string representation and constructing a `DateTimeImmutable`
through its own codec layer, versus this package's single zero-copy FFI call plus a
`DateTimeImmutable` built from exact integers (`createFromTimestamp`/`setMicrosecond` on PHP
8.4+; a `createFromFormat` fallback keeps older PHP correct).

Reproduce: `composer require --dev phpbench/phpbench ramsey/uuid && XDEBUG_MODE=off vendor/bin/phpbench run --report=aggregate --retry-threshold=5`.

### The native extension spike

**The `skunkwerkx/hyperuuid` Composer package (see Install below) is `ext-ffi` only** —
everything above (`HyperUuid`, `Uuid`, `Namespaces`) — chosen because it needs zero
compilation to install and already benchmarks level with or ahead of a naive pure-PHP v4 (see above). It
is not the fastest thing this repo can produce.

The same Rust core also links straight into a real Zend extension via
[`ext-php-rs`](https://ext-php.rs) (`rust/src/php_ext.rs`, gated behind the crate's `php`
Cargo feature) — the same move Python (PyO3) and Ruby (Magnus) get a shipped native backend
for. PHP's didn't ship, for two reasons. The mechanism was never the bottleneck here the way
ctypes and Fiddle were: the `ext-ffi` crossing measured ~105ns, so what a Zend extension
removes is the PHP-level wrapper around the call, not the call. And a Zend extension is
pinned to one PHP ABI per build — the API number plus NTS or ZTS — with no Windows build on
stable Rust, so shipping it means a binary per PHP version where the `ext-ffi` package ships
one library per platform. CI builds the extension on every Linux and macOS leg, load-checks
it, and uploads and attests the result, so it cannot silently bit-rot; no `phpunit` runs
against it and nothing in the Composer package loads it. HyperCast carries the same spike on
the same terms. If you want to chase the last bit of single-call latency anyway, here's how
to build and load it yourself:

1. **Prerequisites:** a Rust toolchain ([rustup](https://rustup.rs)) and PHP's development
   headers (the `php-dev` / `php8.5-dev` / `php-devel` package for your distro — `ext-php-rs`'s
   build script needs these to link against `libphp`).
2. **Build it** with the `php` feature, not the plain default build — that produces the
   `ext-ffi` binding's cdylib, a different entry point from the same crate; don't load both
   at once. `cargo php-ext` is an alias in `rust/.cargo/config.toml` that builds into its own
   `target/php/` directory, so it can't overwrite the plain cdylib the other bindings load:
   ```sh
   git clone https://github.com/SkunkWerkx/HyperUuid
   cd HyperUuid/rust
   cargo php-ext
   ```
   Produces `target/php/release/libhyperuuid.so` (`.dylib` on macOS; Windows isn't supported —
   `ext-php-rs`'s Windows path needs a nightly-only Rust feature, confirmed via a real E0554
   build failure on stable, so every CI leg here builds Linux/macOS only).
3. **Load it** — either add `extension=/absolute/path/to/target/php/release/libhyperuuid.so` to
   `php.ini`, or pass it ad hoc: `php -d extension=/absolute/path/to/target/php/release/libhyperuuid.so your_script.php`.
   Verify with `php -m | grep hyperuuid`.
4. **Call it.** This extension is a benchmark spike, not a polished second backend, so it
   exposes flat functions taking/returning raw 16-byte binary strings — not this package's
   `Uuid` value object. Wrap the bytes yourself if you want `->__toString()`/`->timestamp()`/etc.:
   ```php
   $bytes = hyperuuid_native_new_v4();   // 16 raw RFC-9562-ordered bytes
   $id = new \HyperUuid\Uuid($bytes);    // wrap it to get the Uuid API back

   // v6 and v7 take the timestamp; there is no "now" default, as there is none at the C ABI
   $v7 = hyperuuid_native_new_v7((int) floor(microtime(true) * 1000));
   ```
   See [`rust/src/php_ext.rs`](../rust/src/php_ext.rs) for the full function list —
   `hyperuuid_native_new_v5`/`_new_v6`(`_batch`)/`_new_v7`(`_batch`)/`_v6_unix_millis`/
   `_v7_unix_millis`, same signatures as `Runtime.php`'s own internal FFI calls, plus
   `hyperuuid_native_version`, which returns the packed integer (`major << 16 | minor << 8 |
   patch`) rather than `nativeVersion()`'s string. The SQL-order conversions are not part of
   the spike.

Last measured on linux-arm64, PHP 8.5, `XDEBUG_MODE=off`, same 5-iterations × 1000-revs shape
as the table above (min of the 5 iteration means). The comparison script that produced these
numbers (`php/native/bench_compare.php`) was removed when the three language extensions
consolidated into one Rust crate — these are the last real measurement taken, not something
you can currently re-run from this repo as-is:

| Call | `ext-ffi` (`Runtime.php`) | `ext-php-rs` native | Speedup |
| --- | ---: | ---: | ---: |
| `newV4` | 223ns | 135ns | **1.65x** |
| `newV5` | 244ns | 175ns | 1.40x |
| `newV6` | 207ns | 113ns | **1.83x** |
| `newV7` | 210ns | 107ns | **1.98x** |
| `newV6Batch(1000)` | 19.6µs | 19.3µs | 1.02x |
| `newV7Batch(1000)` | 16.0µs | 15.8µs | 1.02x |

Worth it for single-item calls (the ~105ns FFI floor is real, but so is a further ~100ns of
PHP-level `Runtime::` call overhead around it that a native extension skips entirely — nearly
2x on `newV6`/`newV7`); not worth it for batch calls, where 1000 UUIDs' worth of native
computation dwarfs the one-time crossing cost either way — which, with the per-PHP-version
packaging cost above, is why this stays a spike rather than a second shipped backend. See
[`php_ext.rs`](../rust/src/php_ext.rs)'s own module doc comment for the full reasoning.

The same extension is also the route to PHP in the browser, proven and documented under
[WebAssembly](#webassembly).

## WebAssembly

**In the browser: proven, not shipped.** The [native extension spike](#the-native-extension-spike)
runs inside WordPress Playground's prebuilt PHP for the browser (`@php-wasm/web`, and
`@php-wasm/node` for node) as a side module — no custom PHP build — verified under node and in
headless Chromium against PHP 8.5.10 (`@php-wasm/*` 3.1.56, October 2026): the version probe,
v4, the v5 RFC 9562 vector, v7 to and from a millisecond timestamp, a 1000-UUID batch and the
out-of-range error. It is not shipped because of what shipping it costs, not because it does
not work: one module per supported PHP minor (each must match its PHP version exactly), a
~4 GB Docker image in CI to build them, and a distribution channel still to choose, for a
binding whose Composer package is FFI only. If someone asks for it, the recipe below is
everything that was needed.

What it runs on, and the two upstream problems found on the way:

- **Playground's JSPI builds only.** Its Asyncify build cannot load third-party extensions.
- **A 64-bit `zend_long` only.** Playground builds PHP with `-DZEND_ENABLE_ZVAL_LONG64
  -D__x86_64__`, so `PHP_INT_SIZE` is 8 and millisecond timestamps are ints. On a 32-bit PHP
  ext-php-rs does not compile ([ext-php-rs#800](https://github.com/extphprs/ext-php-rs/issues/800),
  five one-line fixes), and the binding's `u64` millisecond arguments would not fit a PHP int
  there anyway.
- **Playground's `@php-wasm/compile-extension` drops four flags for Rust's C code**
  ([wordpress-playground#4377](https://github.com/WordPress/wordpress-playground/issues/4377)):
  without them the module fails to load with `bad export type for '__THREW__'`. The recipe
  passes them itself. Its README's "nightly and `-Zbuild-std`" requirement is stale: stable
  Rust (1.99 here) works.

**The recipe** (Rust stable with `wasm32-unknown-emscripten`, Docker, node):

1. Build Playground's extension image for each PHP minor, and copy its PHP headers out:

   ```sh
   npm i @php-wasm/compile-extension @php-wasm/node @php-wasm/universal
   node node_modules/@php-wasm/compile-extension/cli.js --prepare-image --php-versions 8.5
   docker cp <that image's container>:/usr/local/include/php ./php-include
   ```

2. ext-php-rs runs `php -i` and `php-config` to find the PHP it builds for, and the host PHP
   cannot stand in for the wasm one, so give it two scripts that describe the target:

   ```sh
   # fake-php-i.sh
   printf 'PHP Version => 8.5.10\nPHP API => 20250925\nDebug Build => no\nThread Safety => disabled\n'
   # fake-php-config.sh — DIR is ./php-include from step 1, absolute
   case "$1" in
     --includes) echo "-I$DIR -I$DIR/main -I$DIR/TSRM -I$DIR/Zend -I$DIR/ext -I$DIR/ext/date/lib";;
     --version) echo 8.5.10;; *) exit 1;; esac
   ```

3. Build the extension as a static library for the side module to wrap, from `rust/`, with
   Emscripten's environment sourced (the toolchain the image uses is fine):

   ```sh
   SYSROOT="$EMSDK/upstream/emscripten/cache/sysroot"
   PHP=./fake-php-i.sh PHP_CONFIG=./fake-php-config.sh \
   BINDGEN_EXTRA_CLANG_ARGS_wasm32_unknown_emscripten="--sysroot=$SYSROOT -DZEND_ENABLE_ZVAL_LONG64 -D__x86_64__" \
   CFLAGS_wasm32_unknown_emscripten="-fPIC -DZEND_ENABLE_ZVAL_LONG64 -D__x86_64__ -sSUPPORT_LONGJMP=wasm -fwasm-exceptions" \
   RUSTFLAGS="-C relocation-model=pic -C panic=abort" \
   cargo rustc --release --target wasm32-unknown-emscripten --crate-type staticlib --features php
   ```

   Not `EXT_PHP_RS_STATIC_EXT` — that is for linking into PHP itself, not a side module.

4. Wrap it as a side module. The extension directory needs only a `config.m4` and an empty C
   file, since `get_module` comes from the Rust archive:

   ```m4
   PHP_ARG_ENABLE([hyperuuid], [whether to enable hyperuuid], [AS_HELP_STRING([--enable-hyperuuid], [Enable hyperuuid])], [no])
   if test "$PHP_HYPERUUID" != "no"; then
     PHP_NEW_EXTENSION([hyperuuid], [hyperuuid_stub.c], [$ext_shared])
   fi
   ```

   ```sh
   node node_modules/@php-wasm/compile-extension/cli.js --source ./ext --name hyperuuid \
     --php-versions 8.5 --extra-ldflags /build/libhyperuuid.a --out dist
   ```

   That writes `hyperuuid-php8.5-jspi.so` (128 KB; 50 KB gzipped) and `manifest.json`.

5. Load it into the runtime and call it:

   ```js
   import { loadNodeRuntime } from '@php-wasm/node';   // or @php-wasm/web in a page
   import { PHP } from '@php-wasm/universal';
   const php = new PHP(await loadNodeRuntime('8.5', {
     emscriptenOptions: { processId: 1 },
     extensions: [{ source: { format: 'manifest', manifestUrl: 'dist/manifest.json' } }],
   }));
   const r = await php.run({ code: '<?php echo bin2hex(hyperuuid_native_new_v4());' });
   ```

   The functions are the spike's `hyperuuid_native_*` raw-byte API; a browser rollout would
   wrap them the way `Uuid`/`HyperUuid` wrap the FFI calls.

Rolling it out would mean a CI job per PHP minor (steps 1–5, about ten minutes each cold, then
the same calls in headless Chrome) and somewhere to publish the modules and manifests. The
alternative measured beside it — a ~90-line C extension over the `no_std` archive the C# Blazor
build already links, instead of ext-php-rs — also worked, at 12 KB a module and on 32-bit PHP
too, but it is a second native implementation in a language the repo does not otherwise use.

**Running the core as wasm inside PHP**, the way the Java binding does with GraalWasm: there
is no maintained wasm engine PHP can embed, so there is nothing to stand that on. The root
README's [WebAssembly section](../README.md#webassembly) tracks both directions for every
binding.

## Verifying provenance

Packagist has nothing of its own to attest — there's no packed artifact, just a git tag it
resolves against this repo. What's actually worth checking is the native binaries
`stage-native-binaries.yml` committed into `php/src/native/`, each individually signed
by `hyper-build-native.yml` when it was built — that workflow physically lives in
`SkunkWerkx/.github`, so verifying needs `--signer-repo` alongside `--repo`, or `gh` reports
a bare `verifying with issuer "sigstore.dev"` that reads like a bad signature but is only an
identity mismatch:

```sh
composer require skunkwerkx/hyperuuid:X.Y.Z
gh attestation verify vendor/skunkwerkx/hyperuuid/php/src/native/linux-x64/libhyperuuid.so \
  --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
```

The staging commit's own message records the exact `ci.yml` run ID and source SHA the
binary came from (e.g. `chore: stage native binaries from ci.yml run 33523131897`), so
you can cross-check the attested commit against that message directly. See
[csharp/README.md's provenance section](../csharp/README.md#native-binary-provenance) for
more on why `--signer-repo` is needed for some artifacts here and not others.

## Install

```sh
composer require skunkwerkx/hyperuuid
```

Published to [Packagist](https://packagist.org/packages/skunkwerkx/hyperuuid) — no extra
repository configuration needed. See [Requirements](#requirements) for the PHP floor,
`ffi.enable` and the supported platforms.

There are two `composer.json` files in this repo: this directory's own (what CI actually
`composer install`s/tests against) and a second one at [the repo root](../composer.json),
which exists purely because Packagist requires `composer.json` at the top of the git
repository it's watching, with no subdirectory support — confirmed against Packagist's own
submission docs, not assumed. Its `autoload` PSR-4 mapping points into `php/src/`. A symlink
from the root to this file was tried first and rejected: Composer resolves a symlinked
`composer.json`'s relative autoload paths against where the symlink itself sits, not the real
file's directory, so `"src/"` silently resolved to a nonexistent `<repo-root>/src/` instead of
`php/src/`. Keep both in sync by hand when `require`/`autoload` change here.

The native libraries under `src/native/{rid}/` are committed to git, not built by Packagist —
unlike NuGet/Maven Central/PyPI/crates.io, Packagist has no packing step of its own, so
whatever's literally in the git tree at a tagged commit is what a real `composer require`
ships. (Found the hard way: an earlier tag published cleanly but threw a real
`RuntimeException` on install because the native lib wasn't actually in git — see
`src/native/README.md`.) Verified for real end to end since the fix: a fresh scratch project's
`composer require skunkwerkx/hyperuuid` pulling straight from Packagist, generating real UUIDs.

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
