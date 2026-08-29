# hyperuuid

**Python 3.14 finally added `uuid.uuid7()` to stdlib, with a real monotonic counter — genuinely well done. If you're stuck on 3.9-3.13 like most production code still is, stdlib has no v6/v7 at all, and this package gives you both today without waiting for a runtime upgrade — and on any version, the native backend below outruns stdlib outright.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, with
two backends sharing one public surface. The fast path is a native extension built with
[PyO3](https://pyo3.rs) — the Rust core linked directly into the CPython extension module,
auto-selected when importable, no `dlopen`, no C-ABI hop, no `ctypes` marshalling. The
universal fallback calls the native `libhyperuuid` shared library via stdlib
[`ctypes`](https://docs.python.org/3/library/ctypes.html) — dlopen/dlsym plus a raw C-ABI
call, no runtime bridge, nothing to compile on install, and the same code path Pyodide runs
in the browser. Set `HYPERUUID_PURE=1` to force the `ctypes` backend; `hyperuuid.BACKEND`
reports which one is live.

Ships `linux-arm64` only for now, for both backends. The same `ctypes.CDLL` code also runs
under [Pyodide](https://pyodide.org/) in the browser given an Emscripten-built
`libhyperuuid.so` "side module" (Pyodide has shipped real `ctypes` support since 0.18) —
that additional build just isn't included here yet.

```python
import uuid
import hyperuuid

hyperuuid.new_v4()
hyperuuid.new_v5(uuid.NAMESPACE_DNS, "example.com")
hyperuuid.new_v6()
id4 = hyperuuid.new_v7()

hyperuuid.v7_timestamp(id4) # recover the embedded UTC datetime.datetime
hyperuuid.v7_to_sql_order(id4) # byte order SQL Server's uniqueidentifier needs to sort by creation order

# One native call, one random-bytes fetch, one counter reservation for the whole batch:
batch = hyperuuid.new_v7_batch(1000)
```

Returns stdlib `uuid.UUID` objects — the native backend builds them through the
[`fastuuid`](https://github.com/thejcannon/fastuuid)-style fast path (`UUID.__new__` plus
`object.__setattr__` of the `int`/`is_safe` slots), since `UUID.__init__`'s own validation
costs more than the entire native call; the test suite pins constructor indistinguishability
and cross-backend agreement so this can't silently drift from a real `UUID(bytes=...)`
construction. For v5's namespace argument, use the RFC 9562
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
native call, instead of `count` of each. `hyperuuid.v7_to_sql_order(id)`/
`v7_from_sql_order(id)` convert a version 7 UUID to and from the byte order SQL
Server's `uniqueidentifier` needs on the wire to sort by creation order — computed
once in the native Rust core rather than reimplemented in Python, and verified
there (and independently against the real `System.Data.SqlTypes.SqlGuid`
comparator in the C# binding's test suite). `v6_to_sql_order(id)`/
`v6_from_sql_order(id)` do the same for version 6, though same-millisecond v6
UUIDs aren't guaranteed to sort correctly afterward — v6 has no counter, so
`clock_seq`/node (not the timestamp) decide ties, the same pre-existing RFC 9562
v6 limitation plain order already has.

## Why not stdlib `uuid`?

This is the one binding where the honest answer genuinely depends on which Python you're running — this package supports 3.9+, and stdlib's own v6/v7 story changed dramatically partway through that range:

- **Python 3.9-3.13:** stdlib has `uuid1`/`uuid3`/`uuid4`/`uuid5` — no v6, no v7, at all. This package is the only way to get either without a third-party dependency, and the native backend outruns stdlib's v4/v5 on top of that.
- **Python 3.14+:** stdlib added `uuid.uuid6()`/`uuid7()`/`uuid8()`, and `uuid7()` genuinely implements RFC 9562 §6.2's monotonic counter (42 bits of it) — this isn't a naive random-bits implementation, it's a real, well-built addition, and it's what the benchmarks below measure against. This package still wins across the board there — see Benchmarks — plus:
  1. **Cross-language consistency.** The same Rust core mints v5 namespace UUIDs for Go, C#, Ruby, and every other binding in this repo — verified in CI to match stdlib's own `uuid.uuid5` byte-for-byte. If your stack isn't Python-only, or you need every service minting IDs from the literal same engine rather than N independent (if individually correct) implementations, that's not something stdlib can offer regardless of version.
  2. **Batch generation.** `new_v7_batch(count)` shares one timestamp capture, one random-bytes fetch, and one counter reservation across the whole batch — stdlib's `uuid7()` has no bulk-generation entry point, so a loop of individual calls is the only option there.
  3. **One behavior across your whole supported range.** If your package needs to run on 3.9 *and* 3.14, this avoids `sys.version_info`-gated code paths for v6/v7 support.

## Benchmarks

Measured with [`pyperf`](https://github.com/psf/pyperf) (linux-arm64, CPython 3.14.5, `python bench_uuid.py --fast`; see `bench_uuid.py`). The native (PyO3) backend is the default and what these numbers reflect — where CPython's `ctypes` boundary used to cost real, measurable overhead against stdlib's C-accelerated `uuid4`, linking the core directly into the extension module turned every one of those into a win:

| Call | Native (PyO3) | vs. closest stdlib equivalent |
|---|---|---|
| `hyperuuid.new_v4()` | 647 ns | `uuid.uuid4()`: 1.03 µs — **1.6x faster** |
| `hyperuuid.new_v5(...)` | 811 ns | `uuid.uuid5(...)`: 2.0 µs — **2.5x faster** |
| `hyperuuid.new_v6(...)` | ~650 ns | `uuid.uuid6()` (3.14+): 2.85 µs — **4.2x faster** |
| `hyperuuid.new_v7(...)` | ~685 ns | `uuid.uuid7()` (3.14+): 2.69 µs — **4.2x faster** |

The `ctypes` fallback (`HYPERUUID_PURE=1`, and the only backend under Pyodide) doesn't get
this for free — CPython's `ctypes` boundary prices every call at real, measurable interpreted-marshalling
overhead, so v4/v5 there run behind stdlib's C-accelerated equivalents, the trade this
backend makes for zero-compile installs and the WASM path. The native backend removes that
boundary entirely rather than dieting around it.

Batch generation amortizes per-call cost regardless of backend, though it's now
construction-bound on the native path (1.27x over the loop) rather than FFI-bound — the next
tuning target:

| | Mean | vs. individual calls |
|---|---|---|
| `new_v6_batch(1000)` | 1.03 ms ± 0.06 ms | vs. 1000x `new_v6()`: 2.25 ms ± 0.20 ms — **2.2x** |
| `new_v7_batch(1000)` | 1.05 ms ± 0.10 ms | vs. 1000x `new_v7()`: 2.21 ms ± 0.18 ms — **2.1x** |

### Timestamp extraction vs. stdlib's `.time` property

CPython 3.14's `uuid.UUID.time` has real version-aware extraction logic of its own (branches on version, computes the right thing for v6/v7, not just a v1-only stub), so this is a genuine head-to-head — each call measured against a UUID generated once outside the timed loop, so only the extraction itself is timed:

| Call | Native (PyO3) | vs. stdlib `.time` |
|---|---|---|
| `hyperuuid.v6_timestamp(...)` | ~248 ns | `UUID.time` (v6): ~665 ns — **2.7x faster** |
| `hyperuuid.v7_timestamp(...)` | ~248 ns | `UUID.time` (v7): ~665 ns — **2.7x faster** |

Removing the `ctypes` boundary flips this one too — the `ctypes` fallback still loses to
stdlib's `.time` here (a pure Python property reading bytes already in the process, zero FFI
boundary to cross), but the native backend's direct extension call now undercuts it instead.
Worth noting: stdlib's `.time` for v6 returns raw Gregorian-epoch 100ns ticks, not Unix
milliseconds like `hyperuuid.v6_timestamp` — different units if you actually need the value,
but a fair timing comparison of "the cost of pulling the embedded time out" either way.

Reproduce: `pip install -e ".[bench]"` then `python bench_uuid.py --fast -o results.json`.
