# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/hyperuuid.svg)](https://pypi.org/project/hyperuuid/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

**Python 3.14 finally added `uuid.uuid7()` to stdlib, with a real monotonic counter — genuinely well done. If you're stuck on 3.11-3.13 like most production code still is, stdlib has no v6/v7 at all, and this package gives you both today without waiting for a runtime upgrade — and on any version, the native backend below outruns stdlib outright.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation. A
native extension built with [PyO3](https://pyo3.rs) — the Rust core linked directly into the
CPython extension module, no `dlopen`, no C-ABI hop, no `ctypes` marshalling, no runtime
bridge. A second backend runs the same core as a `wasm32-wasip1` module inside CPython through
`wasmtime-py`, opt-in via `pip install hyperuuid[wasm]` and `HYPERUUID_WASM=1` — see
[WebAssembly (wasmtime)](#webassembly-wasmtime).

## Install

```sh
pip install hyperuuid
```

Real platform-specific wheels, so it lands at native speed with nothing to compile, the same
way numpy or cryptography does — no compiler needed and no dependencies at all; the PyO3
extension *is* the package. The wheels are `abi3`, so one per platform covers every CPython
from the 3.11 floor up, eight in all:

| | x64 | arm64 |
| --- | :---: | :---: |
| Linux, glibc 2.28 or newer (`manylinux_2_28`) | ✓ | ✓ |
| Linux, musl 1.2 or newer (`musllinux_1_2` — Alpine) | ✓ | ✓ |
| macOS | ✓ | ✓ |
| Windows | ✓ | ✓ |

**Only wheels are published — there is no sdist.** An interpreter none of the eight matches
(a glibc older than 2.28, PyPy, free-threaded CPython) gets `No matching distribution found`
from pip, not a source build.

**Not yet covered: free-threaded (no-GIL) CPython (`3.13t`/`3.14t`).** An `abi3` wheel is
ignored by a free-threaded interpreter (it's a genuinely separate ABI, not a compatibility
flag), so closing this gap means building and shipping additional version-specific
`cp313t`/`cp314t` wheels alongside the existing eight, not just a build-flag change. PyO3
itself has supported free-threading (opt-in, `gil_used = false`) since 0.23; the cleaner
long-term fix — [PEP 803](https://peps.python.org/pep-0803/)'s `abi3t` stable ABI, one build
covering both GIL and no-GIL — needs Python 3.15+, not yet released. Revisiting once that
lands or free-threaded adoption justifies the extra wheel legs. The stance in the code
today: the extension module does not declare `gil_used = false`, so a build from a checkout
for a free-threaded interpreter imports with the GIL switched back on (CPython says so in a
`RuntimeWarning`) — correct, just not parallel. And no call releases the GIL: each is
nanoseconds to microseconds of pure Rust, less than the handoff would cost.

The package is typed: it ships `py.typed` and a stub for the extension module, so mypy and
pyright see `uuid.UUID` and `datetime.datetime` rather than `Any`.

## Usage

```python
import uuid
import hyperuuid

hyperuuid.new_v4()
hyperuuid.new_v5(uuid.NAMESPACE_DNS, "example.com")
hyperuuid.new_v6()
id4 = hyperuuid.new_v7()

hyperuuid.v7_timestamp(id4) # recover the embedded UTC datetime.datetime
hyperuuid.v7_unix_millis(id4) # the same instant as Unix-epoch milliseconds, an int
hyperuuid.get_timestamp(id4) # None instead of assuming id4 is v6/v7
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
epoch, tops out around the year 5236. `hyperuuid.v6_unix_millis(id)` and
`v7_unix_millis(id)` return the same instant as an `int` of Unix-epoch milliseconds, with
no `datetime` built around it — the cheaper call, and for version 7 the one that carries
the whole 48-bit field. `new_v6`/`new_v7` also accept a
`datetime.datetime` directly in place of a raw millisecond count — converted in exact
integer arithmetic and truncated to the millisecond, never rounded into the next one; a
naive `datetime` is read as local time, the way `datetime.timestamp()` reads it.
`get_timestamp(id)`
is the version-agnostic counterpart to `v6_timestamp`/`v7_timestamp` — it checks
`id.version` itself and returns `None` for anything but a genuine v6/v7 UUID, instead
of assuming the caller already knows. `hyperuuid.new_v6_batch(count)`/
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

A caller's bug is an exception, and the same exception on both backends: an argument of the
wrong type is a `TypeError`; a timestamp no field can hold (negative, or past v7's 48 bits
or v6's 60) and a batch `count` outside 0 to 4294967295 are a `ValueError`; a batch too
large to allocate is a `MemoryError`, never a batch of some other size.

`hyperuuid.native_version()` names the core actually loaded, `"major.minor.patch"`, decoded
from the same packed `hyperuuid_version` export every other binding probes, and
`hyperuuid.BACKEND` says which backend loaded it — `"native"` or `"wasm"`.

## Bulk generation into a buffer

`fill_v7(buffer)` and `fill_v6(buffer)` write raw RFC 9562-ordered bytes — 16 per UUID — straight into a `bytearray` you already own, in one native call, without constructing a single `uuid.UUID`:

```python
import hyperuuid

buf = bytearray(1000 * 16)
hyperuuid.fill_v7(buf)                      # 1000 v7 UUIDs, one timestamp capture
first = bytes(buf[0:16])                    # ready for a BYTEA / uniqueidentifier parameter
```

**This is roughly 15x faster than `new_v7_batch`**, and the reason is worth understanding, because it decides whether you should use it at all:

| path | µs / 1000 UUIDs |
| --- | ---: |
| `new_v7_batch(1000)` → `list[UUID]` | 142 |
| `fill_v7(bytearray)` | **9.7** |
| `fill_v7`, then build `uuid.UUID` objects in Python | 940 |

`new_v7_batch` does not spend its time in the native call — it spends it building a thousand `uuid.UUID` instances. Skip that and Python lands at 9.7 µs, which is the same native ceiling the Go (9.8 µs) and C# (9.6 µs) bindings hit for identical work.

The third row is the catch, and it inverts the advice: **if you need `uuid.UUID` objects, keep using `new_v7_batch`.** Filling bytes and constructing UUIDs from them in Python is more than six times as *slow*, because the extension builds them through a much faster path internally than you can from Python. Reach for `fill_v7` only when bytes are the destination — a database parameter, a wire format, a bulk `COPY` — not a step on the way to objects.

`len(buffer)` must be a multiple of 16, and a zero-length buffer writes nothing. Both functions take an optional `datetime` or Unix-epoch millisecond timestamp, same as the rest of the API.

One deliberate limitation: these take a `bytearray`, not any writable buffer. Supporting `memoryview`, `mmap` or NumPy arrays needs `Py_buffer`, which entered CPython's stable ABI in 3.11. That is this extension's floor as of this release (`abi3-py311`), so the wider buffer protocol is no longer ruled out; it is simply not built yet.

## Why not stdlib `uuid`?

This is the one binding where the honest answer genuinely depends on which Python you're running — this package supports 3.11+, and stdlib's own v6/v7 story changed dramatically partway through that range:

- **Python 3.11-3.13:** stdlib has `uuid1`/`uuid3`/`uuid4`/`uuid5` — no v6, no v7, at all. This package is the only way to get either without a third-party dependency, and the native backend outruns stdlib's v4/v5 on top of that.
- **Python 3.14+:** stdlib added `uuid.uuid6()`/`uuid7()`/`uuid8()`, and `uuid7()` genuinely implements RFC 9562 §6.2's monotonic counter (42 bits of it) — this isn't a naive random-bits implementation, it's a real, well-built addition, and it's what the benchmarks below measure against. This package still wins across the board there — see Benchmarks — plus:
  1. **Cross-language consistency.** The same Rust core mints v5 namespace UUIDs for Go, C#, Ruby, and every other binding in this repo — verified in CI to match stdlib's own `uuid.uuid5` byte-for-byte. If your stack isn't Python-only, or you need every service minting IDs from the literal same engine rather than N independent (if individually correct) implementations, that's not something stdlib can offer regardless of version.
  2. **Batch generation.** `new_v7_batch(count)` shares one timestamp capture, one random-bytes fetch, and one counter reservation across the whole batch — stdlib's `uuid7()` has no bulk-generation entry point, so a loop of individual calls is the only option there.
  3. **One behavior across your whole supported range.** If your package needs to run on 3.11 *and* 3.14, this avoids `sys.version_info`-gated code paths for v6/v7 support.

## Benchmarks

Measured with [`pyperf`](https://github.com/psf/pyperf) (linux-x64 on an Intel Core i9-11900H, CPython 3.14.7 as Fedora builds it, `python bench_uuid.py --fast`; see `bench_uuid.py`). Linking the core directly into the extension module — no `ctypes` boundary to cross — puts every generator ahead of stdlib's own C-accelerated implementations:

| Call | hyperuuid | vs. closest stdlib equivalent |
|---|---|---|
| `hyperuuid.new_v4()` | 243 ns | `uuid.uuid4()`: 718 ns — **3.0x faster** |
| `hyperuuid.new_v5(...)` | 374 ns | `uuid.uuid5(...)`: 1.58 µs — **4.2x faster** |
| `hyperuuid.new_v6(...)` | 351 ns | `uuid.uuid6()` (3.14+): 1.26 µs — **3.6x faster** |
| `hyperuuid.new_v7(...)` | 354 ns | `uuid.uuid7()` (3.14+): 1.28 µs — **3.6x faster** |

Most of what a call costs is the `uuid.UUID` it hands back, so the extension builds that through the C API's own entry points — an instance allocated and its two slots set, no Python-level call — all of it inside the stable ABI, so one wheel still covers every CPython from 3.11.

Batch generation amortizes the rest of the per-call cost:

| | Mean | vs. individual calls |
|---|---|---|
| `new_v6_batch(1000)` | 139 µs ± 5 µs | vs. 1000x `new_v6()`: 333 µs ± 8 µs — **2.4x** |
| `new_v7_batch(1000)` | 135 µs ± 6 µs | vs. 1000x `new_v7()`: 339 µs ± 11 µs — **2.5x** |

### Timestamp extraction vs. stdlib's `.time` property

CPython 3.14's `uuid.UUID.time` has real version-aware extraction logic of its own (branches on version, computes the right thing for v6/v7, not just a v1-only stub), so this is a genuine head-to-head — each call measured against a UUID generated once outside the timed loop, so only the extraction itself is timed:

| Call | hyperuuid | vs. stdlib `.time` |
|---|---|---|
| `hyperuuid.v6_unix_millis(...)` → `int` | 163 ns | `UUID.time` (v6): 479 ns — **2.9x faster** |
| `hyperuuid.v7_unix_millis(...)` → `int` | 161 ns | `UUID.time` (v7): 401 ns — **2.5x faster** |
| `hyperuuid.v6_timestamp(...)` → `datetime` | 250 ns | `UUID.time` (v6): 479 ns — **1.9x faster** |
| `hyperuuid.v7_timestamp(...)` → `datetime` | 252 ns | `UUID.time` (v7): 401 ns — **1.6x faster** |

`.time` returns an `int`, so the `*_unix_millis` rows are the like-for-like ones; the `*_timestamp` rows hand back a timezone-aware `datetime` and are still ahead. Reach for the integer form when the value is going to be stored, compared or forwarded as a number.

Worth noting: stdlib's `.time` for v6 returns raw Gregorian-epoch 100ns ticks, not Unix
milliseconds like `hyperuuid.v6_timestamp` — different units if you actually need the value,
but a fair timing comparison of "the cost of pulling the embedded time out" either way.

Reproduce: `maturin develop --release` (from `python/`, inside a virtualenv — `pyproject.toml`
already points maturin at `../rust/Cargo.toml` and the `python` feature) to build the release
extension — `pip install -e ".[bench]"` alone builds debug by default and will understate
every number above — then `pip install pyperf` and
`python bench_uuid.py --fast -o results.json`.

## WebAssembly (wasmtime)

The same Rust core, compiled to `wasm32-wasip1`, run *inside* CPython by
[`wasmtime-py`](https://github.com/bytecodealliance/wasmtime-py) — the inverse of the Pyodide
experiment this package once carried (CPython itself in the browser, loading the core as an
Emscripten side module). Nothing is reimplemented: `hyperuuid._wasm` calls the identical C-ABI
exports the PyO3 extension does — the twelve `uuid_*` functions and `hyperuuid_version` —
across a guest/host memory boundary instead of a direct call. The whole test suite runs
against it.

```sh
pip install hyperuuid[wasm]        # adds wasmtime; the .wasm module ships inside every wheel
HYPERUUID_WASM=1 python app.py     # force it; hyperuuid.BACKEND reports "wasm" or "native"
```

Without the variable, `_native` is used whenever it imports, and `_wasm` is the fallback when it
does not and `wasmtime` is installed — an install whose extension cannot load keeps working
instead of failing at import. One honest limit on that story today: it does not widen where
`pip install hyperuuid` works. Only wheels are published and every one of them carries the
PyO3 extension, so an interpreter no wheel matches has nothing to install, with or without
`[wasm]`. A pure-Python wheel carrying only the wasm backend is what would make
`pip install hyperuuid[wasm]` land anywhere `wasmtime` itself does; it is not built yet.

One platform note, measured on a bare `python:3.14-alpine`: the musl wheel needs nothing
beyond the base image — it carries its own copy of `libgcc_s` — and the whole suite passes
there. The wasm backend on Alpine additionally needs `apk add libgcc`, because `wasmtime`'s
own musl wheel links `libgcc_s` without bundling it.

Three things about the crossing decide the numbers below:

- **Buffers come from the guest.** A wasm module only sees its own linear memory, so this backend
  asks the module's exported `malloc` for every buffer it fills — 16-byte scratch, a v5 name, a
  batch destination — rather than picking an offset itself. That is load-bearing, not tidiness:
  a host-chosen offset past the data segments was tried first, and the guest's own allocator
  (dlmalloc, which claims the tail of the initial memory on first use) corrupted a batch.
- **Calls are serialized.** A wasmtime `Store` is not thread-safe and the v7 counter lives inside
  the one instance, so one process-wide lock guards every call. Uncontended under the GIL; on a
  free-threaded build it is what keeps two threads out of one store.
- **The call path sidesteps wasmtime-py's per-call type lookup.** `Func.__call__` re-fetches the
  function's type from the engine and builds and frees a `FuncType` plus one `ValType` wrapper per
  parameter and result on *every* call — measured at 38 µs per call, almost none of it in the
  guest. This backend builds the argument and result arrays once and hands them to the same
  `wasmtime_func_call` C entry point the library reaches after that bookkeeping, 3.1 µs for the
  bare call. That touches `wasmtime._ffi`, which is not public API, so it is bound inside a `try`
  at load time and degrades to the public call — slow, never broken — if a wasmtime release
  moves it.

Measured end to end on CPython 3.14.7 (Fedora's build), linux-x64, `timeit` best of five, same
session as the native column:

| Call | wasm backend | native (`_native`) |
| --- | ---: | ---: |
| `new_v4()` | 4.7 µs | 0.25 µs |
| `new_v7(ms)` | 5.4 µs | 0.35 µs |
| `new_v5(...)` | 7.1 µs | 0.38 µs |
| `v7_timestamp(...)` | 4.8 µs | 0.24 µs |
| `fill_v7(bytearray)`, 1000 UUIDs | 24 µs | 9.7 µs |
| `new_v7_batch(1000)` → `list[UUID]` | 447 µs | 142 µs |

Read it the way the rest of this README reads: single calls pay the crossing (4-6 µs of
Python-side lock, argument packing, guest memory copy and the call itself), the byte
fill amortizes it to 2.5x native, and the object-building batch is three times native
because this backend builds its `uuid.UUID`s from Python rather than through the C API.If you are on this backend and minting in bulk, reach
for `fill_v7`/`fill_v6` exactly as the section above already advises.

## Verifying provenance

Every wheel PyPI serves carries a GitHub build-provenance attestation, signed directly by
this repo's own `release.yml` (the `pypi-build-wheels` job attests each platform wheel
right where it's built, no reusable workflow in between), so plain `--repo` verifies it:

```sh
pip download hyperuuid==X.Y.Z --no-deps -d .
gh attestation verify hyperuuid-X.Y.Z-*.whl --repo SkunkWerkx/HyperUuid
```

This is a separate thing from the [PEP 740](https://peps.python.org/pep-0740/) attestations
`gh-action-pypi-publish` already sends to PyPI itself, which PyPI-side tooling checks on its
own — this is the GitHub/Sigstore transparency-log route, checked with `gh attestation
verify`, the same route every other artifact in this project uses. See
[csharp/README.md's provenance section](../csharp/README.md#native-binary-provenance) for why
some artifacts here need `--signer-repo` and this one doesn't.

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
