# hyperuuid

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
```

Returns `HyperUuid\Uuid`, a minimal value object (`->bytes()`, `->__toString()`,
`->version()`, `->variant()`, `->equals()`) — this package has no runtime dependency on
`ramsey/uuid`. `Namespaces::dns()`/`url()`/`oid()`/`x500()` are RFC 9562 Section 6.6's
well-known namespaces. `->timestamp()` recovers the embedded UTC `DateTimeImmutable` from a
version 6 or 7 UUID. `Uuid::nil()`/`Uuid::max()` are the RFC 9562 §5.9/§5.10 special-value
UUIDs.

Requires `ext-ffi` enabled (built into PHP by default when compiled `--with-ffi`; check with
`php -m | grep -i ffi`). PHP's CLI SAPI runs FFI unrestricted regardless of the `ffi.enable`
ini setting — the `preload`-only default only matters for non-CLI SAPIs like FPM.

Not yet published to Packagist under a registered `skunkwerkx` presence — for now this is
proven by CI building and testing the native core plus this package on real hardware for
every platform leg. Consume via a direct
`"repositories": [{"type": "vcs", "url": "https://github.com/SkunkWerkx/HyperUuid"}]` VCS
Composer repository in the meantime.
