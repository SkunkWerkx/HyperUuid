# hyperuuid

**PHP core has no built-in UUID generation at all — nothing beyond the optional PECL `uuid` extension. This package needs zero Composer dependency, not even `ramsey/uuid` — just PHP's own built-in `FFI` extension.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, calling
directly into the native `libhyperuuid` shared library via PHP's built-in
[`FFI`](https://www.php.net/manual/en/book.ffi.php) extension — dlopen/dlsym plus a raw C-ABI
call, no runtime bridge, no Composer dependency beyond `ext-ffi` itself. Bundles a native
build for every supported platform (linux/darwin/windows × x64/arm64) and picks the right one
at runtime, the same trick the Go/Java bindings use since Composer has no per-platform
native selection.

```php
use HyperUuid\HyperUuid;
use HyperUuid\Namespaces;

$id = HyperUuid::newV4();
$id2 = HyperUuid::newV5(Namespaces::dns(), 'example.com');
$id3 = HyperUuid::newV6();
$id4 = HyperUuid::newV7();

$id4->timestamp(); // recover the embedded UTC DateTimeImmutable
$id4->toSqlOrder(); // byte order SQL Server's uniqueidentifier needs to sort by creation order

// One native call, one random-bytes fetch, one counter reservation for the whole batch:
$batch = HyperUuid::newV7Batch(1000);
```

Returns `HyperUuid\Uuid`, a minimal value object (`->bytes()`, `->__toString()`,
`->version()`, `->variant()`, `->equals()`) — this package has no runtime dependency on
`ramsey/uuid`. `Namespaces::dns()`/`url()`/`oid()`/`x500()` are RFC 9562 Section 6.6's
well-known namespaces. `->timestamp()` recovers the embedded UTC `DateTimeImmutable` from a
version 6 or 7 UUID. `->toSqlOrder()`/`->fromSqlOrder()` convert a version 6 or 7 UUID to and
from the byte order SQL Server's `uniqueidentifier` needs on the wire to sort by creation
order (`toSqlOrder()` dispatches on the UUID's own version, matching `timestamp()`'s
convention) — computed once in the native Rust core rather than reimplemented in PHP, and
verified there (and independently against the real `System.Data.SqlTypes.SqlGuid` comparator
in the C# binding's test suite). Same-millisecond v6 UUIDs aren't guaranteed to sort correctly
afterward — v6 has no counter, so `clock_seq`/`node` (not the timestamp) decide ties, the same
pre-existing RFC 9562 v6 limitation plain order already has. `fromSqlOrder()` auto-detects
which version to invert (checking a field that's provably collision-free between the two), or
takes an explicit `$version` argument when you already know it. `Uuid::nil()`/`Uuid::max()`
are the RFC 9562 §5.9/§5.10 special-value UUIDs. `HyperUuid::newV6Batch(count)`/`newV7Batch(count)` generate `count` UUIDs sharing one
timestamp capture and one native call, instead of `count` of each.

## Why not `ramsey/uuid`?

`ramsey/uuid` is the de facto PHP standard, and it's genuinely a solid library — it supports v6 and v7 with its own monotonic-generation story too. This package isn't claiming to out-generate it; the real differentiators are elsewhere:

1. **Zero Composer dependency.** This package needs nothing beyond PHP's own built-in `FFI` extension — no `ramsey/uuid`, no `composer require` at all beyond this package itself. If you're already pulling in `ramsey/uuid` for something else, that's a fine reason to stick with it; if not, this avoids adding it just for ID generation.
2. **Batch generation.** `newV6Batch(count)`/`newV7Batch(count)` share one timestamp capture, one random-bytes fetch, and (v7) one counter reservation across the whole batch — one native call instead of `count` separate ones.
3. **Cross-language consistency.** The same Rust core mints v5 namespace UUIDs for Python, Go, C#, Ruby, and every other binding in this repo — verified in CI to match Python's own `uuid.uuid5` byte-for-byte. A pure-PHP library, however good, can't structurally guarantee that against a codebase written in a different language.
4. **`timestamp()` isn't tied to how the UUID was minted.** It's a plain RFC 9562 bit-layout read, verified (in this package's own test suite) to correctly extract from a `ramsey/uuid`-generated v6 or v7 value too, not just this package's own — so you can keep `ramsey/uuid` for generation and still get this package's (faster, see below) extraction on its output.

Requires `ext-ffi` enabled (built into PHP by default when compiled `--with-ffi`; check with
`php -m | grep -i ffi`). PHP's CLI SAPI runs FFI unrestricted regardless of the `ffi.enable`
ini setting — the `preload`-only default only matters for non-CLI SAPIs like FPM.

## Benchmarks

Real numbers, measured with [PHPBench](https://phpbench.readthedocs.io/) on linux-arm64,
PHP 8.5 (`XDEBUG_MODE=off vendor/bin/phpbench run --report=aggregate`, mode across 5
iterations × 1000 revs each — an earlier edition of this table was measured with Xdebug
loaded, which inflates everything ~14x uniformly; these numbers are clean). PHP core has
nothing to compare against — the honest baseline here is a naive inline v4 built from
`random_bytes(16)` with no FFI call at all, to isolate what the FFI boundary itself
actually costs:

| Call | Time | vs. naive inline (no FFI) |
| --- | --- | --- |
| Naive inline v4 (`random_bytes`, no RFC validation) | 595ns | — |
| `newV4()` | 308ns | **1.9x faster** |
| `newV5()` | 466ns | 1.3x faster |
| `newV6()` | 413ns | 1.4x faster |
| `newV7()` | 303ns | **2.0x faster** |

Read that top row again: the full RFC-complete v4 — real entropy, correct version and
variant bits, crossing into native code and back — is **faster than the naive pure-PHP
three-liner that doesn't even validate anything**. The FFI crossing itself costs ~105ns;
what used to make these calls look expensive was wrapper, not boundary — per-call `CData`
allocations and `memcpy`s that are now a single static out-buffer and zero-copy
`const char *` string passes (inputs cross as plain PHP strings, no copy at all). The
naive inline version, meanwhile, pays PHP-level `chr`/`ord`/string-index fiddling that
costs more than the entire native round trip.

Batch generation still amortizes the remaining per-call cost, though the diet shrank the
gap it has to amortize:

| Call | Batch (1000 items) | Individual × 1000 | Speedup |
| --- | --- | --- | --- |
| v6 | 0.64ms | 0.87ms | 1.4x |
| v7 | 0.66ms | 0.87ms | 1.3x |

### Timestamp extraction vs. `ramsey/uuid`'s `getDateTime()`

`ramsey/uuid` has real extraction logic of its own (`UuidInterface::getDateTime()`, works for
both `UuidV6` and `UuidV7`), so this is a genuine head-to-head, not a strawman — each call
measured against a UUID generated once outside the timed loop, so only the extraction itself
is timed. Unlike generation, where PHP's `FFI` boundary was the honest cost, extraction
flips the result:

| Call | Time | vs. `ramsey/uuid` |
| --- | ---: | ---: |
| `->timestamp()` (v6) | 451ns | **74x faster** |
| `ramsey/uuid`'s `->getDateTime()` (v6) | 33.3µs | baseline |
| `->timestamp()` (v7) | 380ns | **48x faster** |
| `ramsey/uuid`'s `->getDateTime()` (v7) | 18.3µs | baseline |

`ramsey/uuid`'s `getDateTime()` does real work this package's native extraction doesn't have
to: parsing a lazily-decoded UUID string representation and constructing a `DateTimeImmutable`
through its own codec layer, versus this package's single zero-copy FFI call plus a
`DateTimeImmutable` built from exact integers (`createFromTimestamp`/`setMicrosecond` on PHP
8.4+; a `createFromFormat` fallback keeps older PHP correct).

Reproduce: `composer require --dev phpbench/phpbench ramsey/uuid && XDEBUG_MODE=off vendor/bin/phpbench run --report=aggregate`.

### Maximum performance: build the native extension yourself

**The `skunkwerkx/hyperuuid` Composer package (see Install below) is `ext-ffi` only** —
everything above (`HyperUuid`, `Uuid`, `Namespaces`) — chosen because it needs zero
compilation to install and already benchmarks faster than a naive pure-PHP v4 (see above). It
is not the fastest thing this repo can produce.

The same Rust core also links straight into a real Zend extension via
[`ext-php-rs`](https://ext-php.rs) (`rust/src/php_ext.rs`, gated behind the crate's `php`
Cargo feature) — the same move Python (PyO3) and Ruby (Magnus) get a shipped native backend
for. PHP's didn't ship because the `ext-ffi` crossing measured cheap enough (~105ns) that a
second backend wasn't obviously worth the packaging cost — but if you want to chase the last
bit of single-call latency anyway, here's how to build and load it yourself:

1. **Prerequisites:** a Rust toolchain ([rustup](https://rustup.rs)) and PHP's development
   headers (the `php-dev` / `php8.5-dev` / `php-devel` package for your distro — `ext-php-rs`'s
   build script needs these to link against `libphp`).
2. **Build it**, with the `php` feature (not the plain default build — that produces the
   `ext-ffi` binding's cdylib, a different entry point from the same crate; don't load both
   at once):
   ```sh
   git clone https://github.com/SkunkWerkx/HyperUuid
   cd HyperUuid/rust
   cargo build --release --features php
   ```
   Produces `target/release/libhyperuuid.so` (`.dylib` on macOS; Windows isn't supported —
   `ext-php-rs`'s Windows path needs a nightly-only Rust feature, confirmed via a real E0554
   build failure on stable, so every CI leg here builds Linux/macOS only).
3. **Load it** — either add `extension=/absolute/path/to/target/release/libhyperuuid.so` to
   `php.ini`, or pass it ad hoc: `php -d extension=/absolute/path/to/target/release/libhyperuuid.so your_script.php`.
   Verify with `php -m | grep hyperuuid`.
4. **Call it.** This extension is a benchmark spike, not a polished second backend, so it
   exposes flat functions taking/returning raw 16-byte binary strings — not this package's
   `Uuid` value object. Wrap the bytes yourself if you want `->toString()`/`->timestamp()`/etc.:
   ```php
   $bytes = hyperuuid_native_new_v4();   // 16 raw RFC-9562-ordered bytes
   $id = new \HyperUuid\Uuid($bytes);    // wrap it to get the Uuid API back
   ```
   See [`rust/src/php_ext.rs`](../rust/src/php_ext.rs) for the full function list —
   `hyperuuid_native_new_v5`/`_new_v6`(`_batch`)/`_new_v7`(`_batch`)/`_v6_unix_millis`/
   `_v7_unix_millis`, same signatures as `Runtime.php`'s own internal FFI calls.

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
computation dwarfs the one-time crossing cost either way — which is exactly why this stays a
spike rather than a second shipped backend. CI compile-checks the `php` feature on every PR
(Linux/macOS) so it can't silently bit-rot, but there's no `phpunit` run against it — see
[`php_ext.rs`](../rust/src/php_ext.rs)'s own module doc comment for the full reasoning.

## Install

Not yet published to Packagist under a registered `skunkwerkx` presence — for now this is
proven by CI building and testing the native core plus this package on real hardware for
every platform leg. Consume via a direct
`"repositories": [{"type": "vcs", "url": "https://github.com/SkunkWerkx/HyperUuid"}]` VCS
Composer repository in the meantime.

There are two `composer.json` files in this repo: this directory's own (what CI actually
`composer install`s/tests against) and a second one at [the repo root](../composer.json),
which exists purely because Packagist requires `composer.json` at the top of the git
repository it's watching, with no subdirectory support — confirmed against Packagist's own
submission docs, not assumed. Its `autoload` PSR-4 mapping points into `php/src/` (verified
end to end with a real `composer install` from a separate scratch consumer project — real
classes autoload, real UUIDs generate). A symlink from the root to this file was tried first
and rejected: Composer resolves a symlinked `composer.json`'s relative autoload paths against
where the symlink itself sits, not the real file's directory, so `"src/"` silently resolved to
a nonexistent `<repo-root>/src/` instead of `php/src/` — confirmed with the same scratch-project
test, not assumed either. Keep both in sync by hand when `require`/`autoload` change here.

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.
