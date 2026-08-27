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

Real numbers, measured with [PHPBench](https://phpbench.readthedocs.io/) on linux-arm64
(`vendor/bin/phpbench run --report=aggregate`, mode across 5 iterations × 1000 revs each).
PHP core has nothing to compare against — the honest baseline here is a naive inline v4
built from `random_bytes(16)` with no FFI call at all, to isolate what the FFI boundary
itself actually costs:

| Call | Time | vs. naive inline (no FFI) |
| --- | --- | --- |
| Naive inline v4 (`random_bytes`, no RFC validation) | 6.43µs | — |
| `newV4()` | 8.22µs | +1.79µs |
| `newV5()` | 18.66µs | +12.23µs |
| `newV6()` | 8.12µs | +1.69µs |
| `newV7()` | 7.89µs | +1.46µs |

That +1.5-1.8µs delta for v4/v6/v7 is the real, honest cost of PHP's `FFI` call boundary
itself — small in absolute terms, but it's there, and this isn't hiding it. `newV5()` costs
more because it marshals a variable-length name buffer across FFI in addition to the fixed
16-byte namespace, not because SHA-1 hashing is expensive.

Batch generation amortizes that per-call FFI cost the same way it does in every other
binding in this repo:

| Call | Batch (1000 items) | Individual × 1000 | Speedup |
| --- | --- | --- | --- |
| v6 | 2.55ms | 7.65ms | 3.0x |
| v7 | 2.63ms | 7.99ms | 3.0x |

### Timestamp extraction vs. `ramsey/uuid`'s `getDateTime()`

`ramsey/uuid` has real extraction logic of its own (`UuidInterface::getDateTime()`, works for
both `UuidV6` and `UuidV7`), so this is a genuine head-to-head, not a strawman — each call
measured against a UUID generated once outside the timed loop, so only the extraction itself
is timed. Unlike generation, where PHP's `FFI` boundary was the honest cost, extraction
flips the result:

| Call | Time | vs. `ramsey/uuid` |
| --- | ---: | ---: |
| `->timestamp()` (v6) | 14.98µs | **32.7x faster** |
| `ramsey/uuid`'s `->getDateTime()` (v6) | 489.92µs | baseline |
| `->timestamp()` (v7) | 14.64µs | **16.6x faster** |
| `ramsey/uuid`'s `->getDateTime()` (v7) | 243.43µs | baseline |

`ramsey/uuid`'s `getDateTime()` does real work this package's native extraction doesn't have
to: parsing a lazily-decoded UUID string representation and constructing a `DateTimeImmutable`
through its own codec layer, versus this package's single FFI call plus a direct
`DateTimeImmutable::createFromFormat`. The one FFI crossing that costs this package ~1.5µs on
generation (see above) is comfortably paid for here — an honest reversal worth stating plainly
rather than only reporting the numbers where this package wins by default.

Reproduce: `composer require --dev phpbench/phpbench ramsey/uuid && vendor/bin/phpbench run --report=aggregate`.

## Install

Not yet published to Packagist under a registered `skunkwerkx` presence — for now this is
proven by CI building and testing the native core plus this package on real hardware for
every platform leg. Consume via a direct
`"repositories": [{"type": "vcs", "url": "https://github.com/SkunkWerkx/HyperUuid"}]` VCS
Composer repository in the meantime.

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.
