# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![Packagist](https://img.shields.io/packagist/v/skunkwerkx/hyperuuid.svg)](https://packagist.org/packages/skunkwerkx/hyperuuid)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

**PHP core has no built-in UUID generation at all — nothing beyond the optional PECL `uuid` extension. This is an extension too, installed prebuilt by [PIE](https://github.com/php/pie): no compiler, no Composer dependency, no FFI.**

RFC 9562 UUID v4 (random), v5 (deterministic) and v7 (time-sortable) generation, with a
version 8 SQL Server order for v7, from `hyperuuid`, a Zend extension built from the Rust
core with [ext-php-rs](https://ext-php.rs). The whole API is the extension's: the classes
below exist once it is loaded, with no PHP code in your `vendor/`. PHP 8.3 is the floor, and
every supported platform and PHP minor gets a prebuilt, attested zip (see [Install](#install)).

```php
<?php

use HyperUuid\HyperUuid;
use HyperUuid\Namespaces;
use HyperUuid\UuidLayout;

$id = HyperUuid::newV4();
$id2 = HyperUuid::newV5(Namespaces::dns(), 'example.com');
$id3 = HyperUuid::newV7();
echo 'v4: ' . $id . PHP_EOL;
echo 'v5: ' . $id2 . PHP_EOL;
echo 'v7: ' . $id3 . PHP_EOL;

// recover the embedded UTC DateTimeImmutable
echo 'timestamp: ' . $id3->timestamp()->format(DATE_RFC3339_EXTENDED) . PHP_EOL;
// the same instant as Unix-epoch milliseconds
echo 'unix millis: ' . $id3->unixMillis() . PHP_EOL;
// null instead of throwing if $id3 isn't an RFC 9562 v7
$checked = $id3->timestamp(throwOnMismatch: false);
echo 'checked timestamp: ' . ($checked?->format(DATE_RFC3339_EXTENDED) ?? 'null') . PHP_EOL;
// true: the RFC 9562 variant and version 7, in one native call
echo 'is v7: ' . var_export($id3->isRfc(7), true) . PHP_EOL;

// byte order SQL Server's uniqueidentifier needs to sort by creation order
$sql = $id3->toSqlOrder();
echo 'sql order: ' . $sql . PHP_EOL;
echo 'sql is v7: ' . var_export($sql->isRfc(7, UuidLayout::SqlServer), true) . PHP_EOL;
$sqlCreated = $sql->timestamp(throwOnMismatch: false, layout: UuidLayout::SqlServer); // read in place
echo 'sql timestamp: ' . ($sqlCreated?->format(DATE_RFC3339_EXTENDED) ?? 'null') . PHP_EOL;

// One native call, one random-bytes fetch, one counter reservation for the whole batch:
$batch = HyperUuid::newV7Batch(1000);
echo 'batch: ' . count($batch) . PHP_EOL;
```

Returns `HyperUuid\Uuid`, a minimal value object (`->bytes()`, `->__toString()`,
`->version()`, `->variant()`, `->isRfc()`, `->equals()`; see
[Inspecting a UUID](#inspecting-a-uuid)) — this package has no runtime dependency on
`ramsey/uuid`. It casts to the hyphenated string and `json_encode`s as that same string;
`Uuid::parse()` reads the 8-4-4-4-12 hyphenated form in either letter case and nothing
else, the same rule the Rust core applies. `Namespaces::dns()`/`url()`/`oid()`/`x500()` are RFC 9562 Section 6.6's
well-known namespaces. `->timestamp()` recovers the embedded UTC `DateTimeImmutable` from an
RFC 9562 version 7 UUID; pass `throwOnMismatch: false` to get `null` back for anything else
instead of throwing — a version 6 or 1 from another generator included, since this package
reads no Gregorian timestamp. `newV7()` also accepts a `DateTimeInterface` directly in place of
a raw millisecond count. `->toSqlOrder()`/`->fromSqlOrder()` convert a version 7 UUID to and
from SQL Server order, the byte order a `uniqueidentifier` sorts by creation — computed in the
native Rust core rather than reimplemented in PHP, and verified there (and against the real
`System.Data.SqlTypes.SqlGuid` comparator in the C# binding's test suite). A SQL-ordered value
is an RFC 9562 version 8 UUID: the 8 and the variant sit where SQL Server's text form shows
them, and the version 7's other 122 bits fill the compare order around them, timestamp and
counter first. Every version 7 converts, and both conversions are exact inverses. Each refuses
a value of the wrong version with `\InvalidArgumentException`: `toSqlOrder()` anything but an
RFC 9562 version 7, `fromSqlOrder()` anything without the SQL layout's version 8 and variant
(the same check as `->version(UuidLayout::SqlServer)`). `Uuid::nil()`/`Uuid::max()` are the
RFC 9562 §5.9/§5.10 special-value UUIDs. `HyperUuid::newV7Batch(count)` generates `count`
UUIDs sharing one timestamp capture and one native call, instead of `count` of each.

## Inspecting a UUID

Every read below is one call into the native core; the extension does no bit-reading of its own.

```php
use HyperUuid\HyperUuid;
use HyperUuid\UuidLayout;
use HyperUuid\UuidVariant;

$id = HyperUuid::newV7();
$id->version();                  // the RFC 9562 version nibble, 0-15 (0 for Uuid::nil(), 15 for Uuid::max())
$id->variant();                  // UuidVariant::Rfc9562 for anything this package or ramsey/uuid mints
$id->isRfc(7);                   // the RFC 9562 variant and version 7: the guard before trusting v7 fields
$id->unixMillis();               // the embedded Unix-epoch milliseconds of an RFC 9562 v7, or null

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
`timestamp()` check it too, in the same native call that reads the timestamp: a 7 nibble under
another variant has no RFC version, so it has no timestamp (`null`, or the throw). A `$version`
outside 0-15 is never matched; it's `false`, not an error.

`UuidLayout` says which byte order a `Uuid` holds: `Rfc9562` (1, the default everywhere) or
`SqlServer` (2, what `toSqlOrder()` returns and a `uniqueidentifier` column stores).
`version()`, `isRfc()`, `unixMillis()` and `timestamp()` all take one. SQL Server order holds
converted version 7s only, and reads them as their source version: there `version()` answers
7, or 0 for anything that isn't a SQL-ordered value, `unixMillis()` answers null for the
latter, and `timestamp()` throws (or returns null with `throwOnMismatch: false`). The core
checks the version 8 marker and the variant.

The layout is the caller's to know, not something a value reveals: carry it with the value,
as a column type already does, and pass it at every call site that can see a SQL-ordered
value. Without a layout argument, `version()` and `isRfc()` read RFC order, so
`$sqlOrdered->isRfc(7)` reads the wrong bytes; there is deliberately no layout-agnostic check.
`variant()` is RFC order only, with no layout parameter: a `toSqlOrder()` result or a
`uniqueidentifier` read-back carries the RFC variant at octet 8 too, so it answers `Rfc9562`
there without saying whether it is a SQL-ordered value. `isRfc(7, UuidLayout::SqlServer)` is
the guard for those.

## Comparing and serializing

`$a->equals($b)` is the bytewise check. `==` on two `Uuid`s is PHP's loose property
comparison, not an identity test: PHP compares the private `bytes` strings with `==`, numeric
strings included, so two different UUIDs whose bytes both read as the same number compare
equal, and `<=>` orders by those strings the same way:

```php
use HyperUuid\Uuid;

$a = new Uuid('0000000000000001');
$b = new Uuid('1e00000000000000');
var_dump($a == $b, $a <=> $b, $a->equals($b)); // bool(true), int(0), bool(false)
```

A `Uuid` survives `serialize()`/`unserialize()`, and the payload is 1.0's, byte for byte, so
one written to a session or a cache by 1.0 unserializes under this version and the other way
round. A payload whose bytes are not 16 long is refused with `\InvalidArgumentException`, as
the constructor refuses them. `clone` gives an equal, independent value, and `var_dump`,
`print_r` and `(array)` show the `bytes` property.

## Binding to SQL Server

Bind `->toSqlOrder()`'s result as its raw `->bytes()`, as binary. pdo_sqlsrv 5.13 sends a string
in SQL Server's mixed-endian text order, which stores the first three groups byte-reversed and
loses the creation order ([details](https://github.com/SkunkWerkx/HyperUuid#sql-server-ordering)). Binary lands byte for byte (`$pdo` is a
`PDO` connection through pdo_sqlsrv):

```php
use HyperUuid\HyperUuid;
use HyperUuid\Uuid;

$sql = HyperUuid::newV7()->toSqlOrder();
$insert = $pdo->prepare('INSERT INTO orders (id) VALUES (?)');
$bytes = $sql->bytes();
$insert->bindParam(1, $bytes, PDO::PARAM_LOB, 0, PDO::SQLSRV_ENCODING_BINARY);
$insert->execute();

$row = $pdo->query('SELECT CAST(id AS binary(16)) AS id FROM orders')->fetch();
$sql = new Uuid($row['id']); // read with UuidLayout::SqlServer
```

The binary encoding is what makes it binary: without it pdo_sqlsrv translates the parameter as
UTF-8 text and throws, since the bytes are never valid UTF-8. `bindValue` takes no encoding, so
use `bindParam`. Reading needs the `CAST` too: pdo_sqlsrv returns a `uniqueidentifier` as text
with those groups reversed, while a plain `fetch()` of the `binary(16)` hands back the raw bytes.

## Install

```sh
pie install skunkwerkx/hyperuuid
```

[PIE](https://github.com/php/pie), the PHP Foundation's extension installer, resolves the
package on [Packagist](https://packagist.org/packages/skunkwerkx/hyperuuid), downloads the zip
built for your platform and PHP minor from the GitHub Release, installs it into the
extension directory and enables it. Nothing compiles: the package has no source build, so where
no zip fits, PIE stops with its own error that no matching asset exists. `composer require`
does not install an extension; to have Composer check that it is loaded, put
`"ext-hyperuuid": "*"` in your project's `require`.

PIE itself needs:

- PHP 8.1 or newer to run, with the `iconv` and `openssl` extensions: the phar's requirement
  checker stops without them.
- The `zip` extension or an `unzip` binary, to unpack the download.
- On macOS, the Xcode Command Line Tools (`xcode-select --install`). PIE tells macOS's libc
  from glibc with `otool`; without it, it looks for a glibc zip, which does not exist for
  macOS. Homebrew's PHP needs them already.

### Platforms and PHP versions

Every zip is NTS (non-thread-safe) only, the build every distribution's `php` CLI and FPM
packages ship; a ZTS PHP (FrankenPHP, for one) has no zip yet. 18 zips a release, each one
load-checked into the PHP it is named for, and run through the full suite there, before it is
packed:

| Platform | PHP 8.3 | PHP 8.4 | PHP 8.5 | Zip on the release |
| --- | :---: | :---: | :---: | --- |
| Linux x64, arm64 (glibc 2.34 or newer) | ✅ | ✅ | ✅ | `php_hyperuuid-v<version>_php<minor>-{x86_64,arm64}-linux-glibc-nts.zip` |
| Linux x64, arm64 (musl: Alpine) | ✅ | ✅ | ✅ | `php_hyperuuid-v<version>_php<minor>-{x86_64,arm64}-linux-musl-nts.zip` |
| macOS arm64 | ✅ | ✅ | ✅ | `php_hyperuuid-v<version>_php<minor>-arm64-darwin-bsdlibc-nts.zip` |
| Windows x64 | ✅ | ✅ | ✅ | `php_hyperuuid-v<version>-<minor>-nts-{vs16,vs17}-x86_64.zip` (VS16 for 8.3, VS17 from 8.4) |

glibc 2.34 means Debian 12, Ubuntu 22.04, RHEL 9, Amazon Linux 2023 or newer. An extension is
tied to one PHP minor, so a zip built for 8.4 is refused by 8.5; PIE picks by the PHP it runs
against. Windows on ARM hardware takes the x64 zip, because PHP itself is an x64 process there:
PHP has never shipped a native Windows ARM64 build. Intel macOS has no zip: Homebrew bottles
no PHP for it any more, and nothing here compiles a PHP to build against. A 32-bit PHP is not
supported.

### Checking it is loaded

Without the extension the classes don't exist, so `extension_loaded('hyperuuid')` is the
check for a consumer with a fallback (here `$fallback` is a ramsey/uuid `UuidFactory`):

```php
use HyperUuid\HyperUuid;

$id = extension_loaded('hyperuuid')
    ? (string) HyperUuid::newV7()
    : (string) $fallback->uuid7();
```

`HyperUuid::nativeVersion()` returns the extension's own `"major.minor.patch"`, the same string
as `phpversion('hyperuuid')`, which is the version of the core it links. `php -m` lists
`hyperuuid` when it is loaded.

### Stubs for IDEs and static analysis

An extension's classes have no PHP source for an IDE or an analyser to read, so the repository
carries a stub, [`php/stubs/hyperuuid.stub.php`](stubs/hyperuuid.stub.php): every class,
method, constant and enum case with its types and doc comment, regenerated from the extension
and compared in CI. PIE installs only the extension, so fetch the stub from the release's tag:

```sh
curl -LO https://raw.githubusercontent.com/SkunkWerkx/HyperUuid/v<version>/php/stubs/hyperuuid.stub.php
```

PhpStorm indexes it like any PHP file in the project: copy it in, or add its directory under
*Settings | PHP | Include Path*. PHPStan needs it under `scanFiles`, which is how it discovers
classes that have no source (`stubFiles` alone only overrides classes it has already found, and
leaves these unknown):

```neon
parameters:
    scanFiles:
        - stubs/hyperuuid.stub.php
```

## Differences from 1.0

1.0 was a Composer package over PHP's FFI extension. The API, defaults, exceptions and
messages are the same in this extension, except:

- `HyperUuid::isAvailable()` is gone; use `extension_loaded('hyperuuid')`.
- The private `bytes` property is untyped and not `readonly` in reflection
  (`ReflectionProperty::isReadOnly()` is `false`): ext-php-rs cannot declare a typed readonly
  property. Reflection, `isset` and other property internals of that private property differ;
  comparison, serialization and clone are unchanged.
- A call with more arguments than the method declares is an `ArgumentCountError`; 1.0, as
  userland PHP, ignored the extras. A call with too few is an `ArgumentCountError` as before,
  worded as for a built-in function (`HyperUuid\Uuid::__construct() expects exactly 1
  argument, 0 given`).
- Exceptions, warnings and deprecations are reported at the caller's file and line; 1.0's came
  from inside its own source files.
- A `DateTimeInterface` beyond the 64-bit millisecond range is a
  `TimestampOutOfRangeException` (1.0: `TypeError`).
- `isRfc($version)` with a `$version` outside 0-15 is never matched; 1.0 could truncate a large
  value into that range.
- A v7 timestamp stops at 9999-12-31T23:59:59.999Z (253402300799999 ms); 1.0 minted every value
  of the 48-bit field, to 10889. The `TimestampOutOfRangeException` message names the range,
  `unix_millis must be between 0 and 253402300799999 (9999-12-31T23:59:59.999Z)`; 1.0 said
  `unix_millis must fit within the RFC 9562 48-bit field`. The tests compare the extension
  against 1.0's recordings through exactly these two differences (`tests/Support/V1Differences.php`).

Arguments are taken as 1.0 took them. A caller without `declare(strict_types=1)` gets PHP's
weak-mode coercion of each declared type, with its deprecations (`newV7("1700000000000")`,
`newV7Batch("10")` and `timestamp(0)` work as they did), and a strict caller gets the same
`TypeError`s, worded the same. A `DateTimeInterface` whose `getTimestamp()` returns something
other than an int goes through 1.0's arithmetic, `getTimestamp() * 1000`, with its warnings and
errors. One trace of the extension shows on a debug (`ZEND_DEBUG`) build of PHP 8.5, which
checks a built-in method's arguments before the call: a `NAN` given to `timestamp()`'s
`$throwOnMismatch` in weak mode raises PHP 8.5's "unexpected NAN value was coerced to bool"
warning twice.

## Bulk generation into bytes

`newV7BatchBytes` returns the batch as one binary string of raw RFC 9562-ordered bytes — 16 per UUID — instead of an array of `Uuid` objects:

```php
use HyperUuid\HyperUuid;

$bytes = HyperUuid::newV7BatchBytes(1000);
$first = substr($bytes, 0, 16);   // ready for a BINARY(16) bind parameter
```

**About 16x faster than `newV7Batch`** for a 1000-UUID batch (4.2 µs versus 67.1 µs), and 64x faster than a thousand `newV7()` calls, in the [table below](#benchmarks), which predates the extension. The native call is identical in both; `newV7Batch` simply allocates 1000 `Uuid` objects on top of it.

The catch, and it inverts the advice: **if you need `Uuid` objects, keep using `newV7Batch`.** Slicing these bytes into objects yourself just relocates the identical allocations into your own code, and measures no better — sometimes worse. Reach for the byte form only when bytes are the destination: a bind parameter, a wire format, a bulk load.

Slice it with `substr($bytes, $i * 16, 16)`.

A batch count below 0 is an `InvalidArgumentException` on both batch methods; 0 returns
an empty array or an empty string. A batch takes at most `HyperUuid::MAX_V7_BATCH` (67,108,864);
a larger count is an `InvalidArgumentException` naming the limit, thrown before
anything is allocated.

### Version 7 ordering

Every batch is in strictly increasing order, and every v7 call, single or batch, reserves
from one process-wide stream that is re-seeded at random whenever a timestamp moves it
forward. A single call consumes one of a millisecond's 2^20 steps and a batch of n consumes
⌈n/64⌉. The stream is per process: each FPM worker has its own. For timestamps up to
2^44 − 2 (the year 2527):

- **Ordering.** A call whose timestamp is within 10 s of the newest stream timestamp either
  throws `CounterExhaustedException` (below) or reserves from the stream, and its UUIDs then
  sort strictly after every UUID from a reservation that completed before it began.
  Overlapping reservations are ordered by reservation, which need not be completion order.
- **Embedded timestamp.** At most 11 s ahead of the one supplied: a timestamp up to 11 s
  behind the stream is clamped onto the stream's millisecond, and a millisecond whose steps
  run out splits into the next, so a split's tail carries the millisecond after it (at most
  1000 ms ahead of the supplied timestamp). An out-of-stream call carries its own timestamp
  exactly.
- **Backfill and future-dated calls.** A timestamp more than 11 s behind the stream, or past
  2^44 − 2, is minted at itself, outside the stream: it succeeds, leaves the stream alone and
  is ordered against nothing. One call far in the future therefore advances the stream, and
  until the clock catches up, real-time calls are minted out of stream, unordered among
  themselves.
- **Exhaustion.** A split that would take the stream more than 1000 ms ahead of the supplied
  timestamp, or past 2^44 − 2, throws `HyperUuid\CounterExhaustedException` (a
  `\RuntimeException`) with the message `version 7 counter exhausted: the stream cannot
  advance without running more than 1000 ms ahead of the supplied timestamp or past its last
  millisecond`, before anything is reserved. It is a state error, not an argument error, and
  a call whose timestamp is past the stream's millisecond succeeds. The random source failing
  is `RandomSourceException`; a timestamp before 1970 or after 9999-12-31T23:59:59.999Z
  (253,402,300,799,999 ms, the generation cap) is `TimestampOutOfRangeException`. The 48-bit
  field itself reaches 10889: `timestamp()` and `unixMillis()` read a version 7 another
  generator dated past the cap as they read a version 6, no timestamp.

## Why not `ramsey/uuid`?

`ramsey/uuid` is the de facto PHP standard, and it's genuinely a solid library — it supports v7 with its own monotonic-generation story too. This package isn't claiming to out-generate it; the real differentiators are elsewhere:

1. **Nothing in `vendor/`.** The API is the extension, installed once per PHP — no `ramsey/uuid`, no Composer dependency at all. If you're already pulling in `ramsey/uuid` for something else, that's a fine reason to stick with it; if not, this avoids adding it just for ID generation.
2. **Batch generation.** `newV7Batch(count)` shares one timestamp capture, one random-bytes fetch, and one counter reservation across the whole batch — one native call instead of `count` separate ones.
3. **Cross-language consistency.** The same Rust core mints v5 namespace UUIDs for Python, Go, C#, Ruby, and every other binding in this repo — verified in CI to match Python's own `uuid.uuid5` byte-for-byte. A pure-PHP library, however good, can't structurally guarantee that against a codebase written in a different language.
4. **`timestamp()` isn't tied to how the UUID was minted.** It's a plain RFC 9562 bit-layout read, verified (in this package's own test suite) to correctly extract from a `ramsey/uuid`-generated v7 value too, not just this package's own — so you can keep `ramsey/uuid` for generation and still get this package's (faster, see below) extraction on its output.

## Benchmarks

**These tables predate the extension.** They measured 1.0's FFI package, and are kept
until the extension is measured on the same machine; the benchmarks in `php/bench/` now run
against the extension. Measured with [PHPBench](https://phpbench.readthedocs.io/) on linux-x64
(an Intel Core i9-11900H), PHP 8.5 (`XDEBUG_MODE=off vendor/bin/phpbench run
--report=aggregate --retry-threshold=5`, mode across 5 iterations × 1000 revs each, repeated
until the iterations agree within 5%; a loaded Xdebug inflates everything ~14x uniformly, hence
`XDEBUG_MODE=off`). PHP core has nothing to compare against — the honest baseline here is a
naive inline v4 built from `random_bytes(16)`, with no call into native code at all:

| Call | Time | vs. naive inline |
| --- | --- | --- |
| Naive inline v4 (`random_bytes`, no RFC validation) | 308ns | — |
| `newV4()` | 224ns | **1.4x faster** |
| `newV7()` † | 265ns | **1.2x faster** |
| `newV5()` | 502ns | 1.6x slower — a SHA-1 the baseline has no equivalent of |

† Measured before the per-millisecond re-seed (core change, 2026-10-27).

The full RFC-complete v4 — real entropy, correct version and variant bits, crossing into
native code and back — costs less than the naive pure-PHP three-liner that doesn't validate
anything, and v7 beats it too. How far ahead depends on the machine, because most of the naive
version is `random_bytes` and the operating system prices that: where the system random source
is slow the gap is wider.

Batch generation amortizes the per-call cost — one crossing instead of a thousand — and
the [byte form](#bulk-generation-into-bytes) then skips building a thousand `Uuid` objects,
which is most of what is left:

| Call | Individual × 1000 | Batch → `Uuid[]` | Batch → bytes |
| --- | ---: | ---: | ---: |
| v7 † | 267.0µs | 67.1µs (**4.0x**) | 4.2µs (**64x**) |

† Measured before the per-millisecond re-seed (core change, 2026-10-27).

### Timestamp extraction vs. `ramsey/uuid`'s `getDateTime()`

`ramsey/uuid` has real extraction logic of its own (`UuidInterface::getDateTime()`), so this is a genuine head-to-head, not a strawman — each call
measured against a UUID generated once outside the timed loop, so only the extraction itself
is timed:

| Call | Time | vs. `ramsey/uuid` |
| --- | ---: | ---: |
| `->timestamp()` (v7) | 293ns | **45x faster** |
| `ramsey/uuid`'s `->getDateTime()` (v7) | 13.2µs | baseline |

`ramsey/uuid`'s `getDateTime()` does real work this package's native extraction doesn't have
to: parsing a lazily-decoded UUID string representation and constructing a `DateTimeImmutable`
through its own codec layer, versus one native call and a `DateTimeImmutable` built from exact
integers.

Reproduce, in `php/` of a checkout with the extension built (see [Development](#development)):
`composer install && XDEBUG_MODE=off composer bench -- --report=aggregate --retry-threshold=5`.

## WebAssembly: not shipped, and unproven for these classes

The extension is not built for the browser, and the recipe below has not been run with these
classes: they import `spl_ce_RuntimeException`, `spl_ce_InvalidArgumentException`,
`zend_post_startup_cb` and `php_json_serializable_ce` from PHP, which a side module has to
resolve from WordPress Playground's main module, and that is untested. What is proven is a
raw-byte build of the same crate, functions over 16-byte strings in place of the classes,
loaded as a side module into Playground's prebuilt PHP for the browser (`@php-wasm/web`, and
`@php-wasm/node` for node), with no custom PHP build: verified under node and in headless
Chromium against PHP 8.5.10 (`@php-wasm/*` 3.1.56, October 2026) for the version probe, v4,
the v5 RFC 9562 vector, v7 to and from a millisecond timestamp, a 1000-UUID batch and the
out-of-range error. Shipping it would cost one module per supported PHP minor (each must
match its PHP version exactly), a ~4 GB Docker image in CI to build them, and a distribution
channel still to choose.

What it runs on, and the two upstream problems found on the way:

- **Playground's JSPI builds only.** Its Asyncify build cannot load third-party extensions.
- **A 64-bit `zend_long` only.** Playground builds PHP with `-DZEND_ENABLE_ZVAL_LONG64
  -D__x86_64__`, so `PHP_INT_SIZE` is 8 and millisecond timestamps are ints. On a 32-bit PHP
  ext-php-rs does not compile ([ext-php-rs#800](https://github.com/extphprs/ext-php-rs/issues/800),
  five one-line fixes), and the extension's 64-bit millisecond arguments would not fit a PHP
  int there anyway.
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

   That writes `hyperuuid-php8.5-jspi.so` and `manifest.json`.

5. Load it into the runtime and call it:

   ```js
   import { loadNodeRuntime } from '@php-wasm/node';   // or @php-wasm/web in a page
   import { PHP } from '@php-wasm/universal';
   const php = new PHP(await loadNodeRuntime('8.5', {
     emscriptenOptions: { processId: 1 },
     extensions: [{ source: { format: 'manifest', manifestUrl: 'dist/manifest.json' } }],
   }));
   const r = await php.run({ code: '<?php echo HyperUuid\\HyperUuid::newV4();' });
   ```

Rolling it out would mean a CI job per PHP minor (steps 1–5, about ten minutes each cold, then
the same calls in headless Chrome) and somewhere to publish the modules and manifests. The
alternative measured beside it — a ~90-line C extension over the `no_std` archive the C# Blazor
build already links, instead of ext-php-rs — also worked, at 12 KB a module and on 32-bit PHP
too, but it is a second native implementation in a language the repo does not otherwise use.

**Running the core as wasm inside PHP**: there is no maintained wasm engine PHP can embed, so
there is nothing to stand that on. The
[WebAssembly table](../docs/platforms.md#webassembly) tracks both directions for every
binding.

## Verifying provenance

PIE does not check attestations on what it downloads, so verify the zip yourself before you
trust it. Each one is named by the release attestation, signed by this repository's
`release.yml` at the tag, with the release inventory as its predicate:

```sh
gh release download v<version> --repo SkunkWerkx/HyperUuid \
  --pattern 'php_hyperuuid-v<version>_php8.5-x86_64-linux-glibc-nts.zip'
gh attestation verify php_hyperuuid-v<version>_php8.5-x86_64-linux-glibc-nts.zip \
  --repo SkunkWerkx/HyperUuid --source-ref refs/tags/v<version> --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
```

Each zip also carries build provenance from `hyper-build-php-extension.yml` in
`SkunkWerkx/.github`, the workflow that built, tested and packed it at the release's
preparation commit, so that check needs `--signer-repo` beside `--repo`:

```sh
gh attestation verify <zip> --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
```

Get the signer wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`, which
reads like a bad signature but is only an identity mismatch. The root README's
[Provenance](../README.md#provenance) section has the whole picture: the inventory, which
artifacts are signed from where, and the gates every byte passes before publication.

## Development

The Composer package Packagist serves is the root [`composer.json`](../composer.json), a
`php-ext` package that PIE reads for its `php-ext` block alone: Packagist requires the manifest
at the top of the git repository it watches. This directory's `composer.json` is the
development project, never published: PHPUnit, PHPBench, PHP_CodeSniffer and `ramsey/uuid` for
the comparisons. To build the extension from a checkout and run the suite against it (a Rust
toolchain and PHP's development headers, `php-config` on `PATH`):

```sh
git clone https://github.com/SkunkWerkx/HyperUuid
cd HyperUuid/rust
cargo php-ext                  # target/php/release/libhyperuuid.so (.dylib on macOS)
cd ../php
composer install
composer test                  # PHPUnit with -d extension= pointing at that build
composer bench                 # PHPBench, the same way
```

To load the build by hand, name the file for your platform:

```sh
# Linux
php -d extension=$PWD/../rust/target/php/release/libhyperuuid.so -r 'echo HyperUuid\HyperUuid::newV4(), PHP_EOL;'
# macOS
php -d extension=$PWD/../rust/target/php/release/libhyperuuid.dylib -r 'echo HyperUuid\HyperUuid::newV4(), PHP_EOL;'
```

`HYPERUUID_PHP_EXTENSION` names another build for `composer test` and `composer bench`.
`HYPERUUID_CHECK_STUBS=1 composer test` also regenerates the stub and compares it with the
committed one (`php tools/generate-stubs.php` rewrites it; it needs the cargo-php version the
script names, on Linux or macOS). `vendor/bin/phpcs` checks the tests and the benchmarks.

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
