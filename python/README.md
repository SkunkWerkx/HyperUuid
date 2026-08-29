# hyperuuid

**Python 3.14 finally added `uuid.uuid7()` to stdlib, with a real monotonic counter — genuinely well done. If you're stuck on 3.9-3.13 like most production code still is, stdlib has no v6/v7 at all, and this package gives you both today without waiting for a runtime upgrade — and on any version, the native backend below outruns stdlib outright.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation. A
native extension built with [PyO3](https://pyo3.rs) — the Rust core linked directly into the
CPython extension module, no `dlopen`, no C-ABI hop, no `ctypes` marshalling, no runtime
bridge.

Ships as real platform-specific wheels — linux/macOS/Windows, x64/arm64, six in total, one
`abi3` build covering every supported CPython 3.9+ — so `pip install hyperuuid` lands at
native speed with nothing to compile, the same way numpy or cryptography does.

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

Returns stdlib `uuid.UUID` objects — built through the
[`fastuuid`](https://github.com/thejcannon/fastuuid)-style fast path (`UUID.__new__` plus
`object.__setattr__` of the `int`/`is_safe` slots), since `UUID.__init__`'s own validation
costs more than the entire native call; the test suite pins constructor indistinguishability
so this can't silently drift from a real `UUID(bytes=...)` construction. For v5's namespace
argument, use the RFC 9562
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

Measured with [`pyperf`](https://github.com/psf/pyperf) (linux-arm64, CPython 3.14.5, `python bench_uuid.py --fast`; see `bench_uuid.py`). Linking the core directly into the extension module — no `ctypes` boundary to cross — turns every one of these into a win against stdlib's own C-accelerated implementations:

| Call | hyperuuid | vs. closest stdlib equivalent |
|---|---|---|
| `hyperuuid.new_v4()` | 647 ns | `uuid.uuid4()`: 1.03 µs — **1.6x faster** |
| `hyperuuid.new_v5(...)` | 811 ns | `uuid.uuid5(...)`: 2.0 µs — **2.5x faster** |
| `hyperuuid.new_v6(...)` | ~650 ns | `uuid.uuid6()` (3.14+): 2.85 µs — **4.2x faster** |
| `hyperuuid.new_v7(...)` | ~685 ns | `uuid.uuid7()` (3.14+): 2.69 µs — **4.2x faster** |

Batch generation amortizes per-call cost, though it's now construction-bound (1.27x over the
loop) rather than FFI-bound — the next tuning target:

| | Mean | vs. individual calls |
|---|---|---|
| `new_v6_batch(1000)` | 1.03 ms ± 0.06 ms | vs. 1000x `new_v6()`: 2.25 ms ± 0.20 ms — **2.2x** |
| `new_v7_batch(1000)` | 1.05 ms ± 0.10 ms | vs. 1000x `new_v7()`: 2.21 ms ± 0.18 ms — **2.1x** |

### Timestamp extraction vs. stdlib's `.time` property

CPython 3.14's `uuid.UUID.time` has real version-aware extraction logic of its own (branches on version, computes the right thing for v6/v7, not just a v1-only stub), so this is a genuine head-to-head — each call measured against a UUID generated once outside the timed loop, so only the extraction itself is timed:

| Call | hyperuuid | vs. stdlib `.time` |
|---|---|---|
| `hyperuuid.v6_timestamp(...)` | ~248 ns | `UUID.time` (v6): ~665 ns — **2.7x faster** |
| `hyperuuid.v7_timestamp(...)` | ~248 ns | `UUID.time` (v7): ~665 ns — **2.7x faster** |

Worth noting: stdlib's `.time` for v6 returns raw Gregorian-epoch 100ns ticks, not Unix
milliseconds like `hyperuuid.v6_timestamp` — different units if you actually need the value,
but a fair timing comparison of "the cost of pulling the embedded time out" either way.

Reproduce: `maturin develop --release --manifest-path native/Cargo.toml` (from `python/`) to
build the release extension — `pip install -e ".[bench]"` alone builds debug by default and
will understate every number above — then `pip install pyperf` and
`python bench_uuid.py --fast -o results.json`.
