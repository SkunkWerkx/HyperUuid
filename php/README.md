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

// One native call, one random-bytes fetch, one counter reservation for the whole batch:
$batch = HyperUuid::newV7Batch(1000);
```

Returns `HyperUuid\Uuid`, a minimal value object (`->bytes()`, `->__toString()`,
`->version()`, `->variant()`, `->equals()`) — this package has no runtime dependency on
`ramsey/uuid`. `Namespaces::dns()`/`url()`/`oid()`/`x500()` are RFC 9562 Section 6.6's
well-known namespaces. `->timestamp()` recovers the embedded UTC `DateTimeImmutable` from a
version 6 or 7 UUID. `Uuid::nil()`/`Uuid::max()` are the RFC 9562 §5.9/§5.10 special-value
UUIDs. `HyperUuid::newV6Batch(count)`/`newV7Batch(count)` generate `count` UUIDs sharing one
timestamp capture and one native call, instead of `count` of each.

## Why not `ramsey/uuid`?

`ramsey/uuid` is the de facto PHP standard, and it's genuinely a solid library — it supports v6 and v7 with its own monotonic-generation story too. This package isn't claiming to out-generate it; the real differentiators are elsewhere:

1. **Zero Composer dependency.** This package needs nothing beyond PHP's own built-in `FFI` extension — no `ramsey/uuid`, no `composer require` at all beyond this package itself. If you're already pulling in `ramsey/uuid` for something else, that's a fine reason to stick with it; if not, this avoids adding it just for ID generation.
2. **Batch generation.** `newV6Batch(count)`/`newV7Batch(count)` share one timestamp capture, one random-bytes fetch, and (v7) one counter reservation across the whole batch — one native call instead of `count` separate ones.
3. **Cross-language consistency.** The same Rust core mints v5 namespace UUIDs for Python, Go, C#, Ruby, and every other binding in this repo — verified in CI to match Python's own `uuid.uuid5` byte-for-byte. A pure-PHP library, however good, can't structurally guarantee that against a codebase written in a different language.

Requires `ext-ffi` enabled (built into PHP by default when compiled `--with-ffi`; check with
`php -m | grep -i ffi`). PHP's CLI SAPI runs FFI unrestricted regardless of the `ffi.enable`
ini setting — the `preload`-only default only matters for non-CLI SAPIs like FPM.

## Install

Not yet published to Packagist under a registered `skunkwerkx` presence — for now this is
proven by CI building and testing the native core plus this package on real hardware for
every platform leg. Consume via a direct
`"repositories": [{"type": "vcs", "url": "https://github.com/SkunkWerkx/HyperUuid"}]` VCS
Composer repository in the meantime.

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.
