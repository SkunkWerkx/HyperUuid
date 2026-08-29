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

### ext-php-rs spike (not shipped)

The numbers above establish PHP's `ext-ffi` crossing as cheap (~105ns) relative to
ctypes/Fiddle — the reason the Python and Ruby bindings got a second, native-extension
backend (PyO3, Magnus) and PHP didn't. That reasoning was asserted, not measured, so
[`native/`](native/) spikes the same move for PHP — the Rust core linked straight into a
Zend extension via [`ext-php-rs`](https://ext-php.rs) — and measures it against `Runtime.php`
at the same layer (raw 16-byte strings, no `Uuid` value-object construction on either side).

Measured on linux-arm64, PHP 8.5, `XDEBUG_MODE=off`, same 5-iterations × 1000-revs shape as
the table above (min of the 5 iteration means; see [`native/bench_compare.php`](native/bench_compare.php)):

| Call | `ext-ffi` (`Runtime.php`) | `ext-php-rs` native | Speedup |
| --- | ---: | ---: | ---: |
| `newV4` | 223ns | 135ns | **1.65x** |
| `newV5` | 244ns | 175ns | 1.40x |
| `newV6` | 207ns | 113ns | **1.83x** |
| `newV7` | 210ns | 107ns | **1.98x** |
| `newV6Batch(1000)` | 19.6µs | 19.3µs | 1.02x |
| `newV7Batch(1000)` | 16.0µs | 15.8µs | 1.02x |

So the answer turns out to be: worth it for single-item calls (the ~105ns FFI floor is real,
but so is a further ~100ns of PHP-level `Runtime::` call overhead around it that a native
extension skips entirely — nearly 2x on `newV6`/`newV7`), and not worth it for batch calls,
where 1000 UUIDs' worth of native computation dwarfs the one-time crossing cost either way.

Kept as a working reference for whoever wants to chase the last bit of single-call
performance, not folded into `skunkwerkx/hyperuuid` — see [`native/README.md`](native/README.md)
for why (Composer has no way to deliver a compiled Zend extension the way it delivers the
FFI `.so`; this would need real platform/ABI packaging work to ship for real).

## Install

Not yet published to Packagist under a registered `skunkwerkx` presence — for now this is
proven by CI building and testing the native core plus this package on real hardware for
every platform leg. Consume via a direct
`"repositories": [{"type": "vcs", "url": "https://github.com/SkunkWerkx/HyperUuid"}]` VCS
Composer repository in the meantime.

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.
