# hyperuuid

**Python 3.14 finally added `uuid.uuid7()` to stdlib, with a real monotonic counter — genuinely well done. If you're stuck on 3.9-3.13 like most production code still is, stdlib has no v6/v7 at all, and this package gives you both today without waiting for a runtime upgrade.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation,
calling directly into the native `libhyperuuid` shared library via stdlib `ctypes` —
no runtime bridge, no extra dependency.

Ships `linux-arm64` only for now. The same `ctypes.CDLL` code also runs under
[Pyodide](https://pyodide.org/) in the browser given an Emscripten-built
`libhyperuuid.so` "side module" (Pyodide has shipped real `ctypes` support since
0.18) — that additional build just isn't included here yet.

```python
import uuid
import hyperuuid

hyperuuid.new_v4()
hyperuuid.new_v5(uuid.NAMESPACE_DNS, "example.com")
hyperuuid.new_v6()
id4 = hyperuuid.new_v7()

hyperuuid.v7_timestamp(id4) # recover the embedded UTC datetime.datetime

# One native call, one random-bytes fetch, one counter reservation for the whole batch:
batch = hyperuuid.new_v7_batch(1000)
```

Returns stdlib `uuid.UUID` objects. For v5's namespace argument, use the RFC 9562
Section 6.6 well-known namespaces already in the standard library —
`uuid.NAMESPACE_DNS`, `NAMESPACE_URL`, `NAMESPACE_OID`, `NAMESPACE_X500` — no need
for this package to redefine them. `hyperuuid.NIL`/`MAX` are the RFC 9562
§5.9/§5.10 special-value UUIDs. `hyperuuid.v7_timestamp(id)` recovers the embedded
UTC `datetime.datetime` from a version 7 UUID (raises `OverflowError` past year
9999 — the RFC's 48-bit field holds values up to year 10889, but
`datetime.datetime` cannot); `hyperuuid.v6_timestamp(id)` does the same for version
6, and can never raise that way — v6's 60-bit tick count, offset from the 1582 UUID
epoch, tops out around the year 5236. `hyperuuid.new_v6_batch(count)`/
`new_v7_batch(count)` generate `count` UUIDs sharing one timestamp capture and one
native call, instead of `count` of each.

## Why not stdlib `uuid`?

This is the one binding where the honest answer genuinely depends on which Python you're running — this package supports 3.9+, and stdlib's own v6/v7 story changed dramatically partway through that range:

- **Python 3.9-3.13:** stdlib has `uuid1`/`uuid3`/`uuid4`/`uuid5` — no v6, no v7, at all. This package is the only way to get either without a third-party dependency.
- **Python 3.14+:** stdlib added `uuid.uuid6()`/`uuid7()`/`uuid8()`, and `uuid7()` genuinely implements RFC 9562 §6.2's monotonic counter (42 bits of it) — this isn't a naive random-bits implementation, it's a real, well-built addition. The case for this package narrows there to:
  1. **Cross-language consistency.** The same Rust core mints v5 namespace UUIDs for Go, C#, Ruby, and every other binding in this repo — verified in CI to match stdlib's own `uuid.uuid5` byte-for-byte. If your stack isn't Python-only, or you need every service minting IDs from the literal same engine rather than N independent (if individually correct) implementations, that's not something stdlib can offer regardless of version.
  2. **Batch generation.** `new_v7_batch(count)` shares one timestamp capture, one random-bytes fetch, and one counter reservation across the whole batch — stdlib's `uuid7()` has no bulk-generation entry point, so a loop of individual calls is the only option there.
  3. **One behavior across your whole supported range.** If your package needs to run on 3.9 *and* 3.14, this avoids `sys.version_info`-gated code paths for v6/v7 support.

If you're already on 3.14+ and only need v6/v7 in a Python-only codebase, stdlib is genuinely the simpler, dependency-free choice — that's not a close call, and this package isn't pretending otherwise.
