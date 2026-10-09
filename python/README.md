# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/hyperuuid.svg)](https://pypi.org/project/hyperuuid/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

**Python 3.14 finally added `uuid.uuid7()` to stdlib, with a real monotonic counter — genuinely well done. If you're stuck on 3.11-3.13 like most production code still is, stdlib has no v6/v7 at all, and this package gives you both today without waiting for a runtime upgrade — and on any version, the native extension below outruns stdlib outright.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation. A
native extension built with [PyO3](https://pyo3.rs) — the Rust core linked directly into the
CPython extension module, no `dlopen`, no C-ABI hop, no `ctypes` marshalling, no runtime
bridge.

## Install

```sh
pip install hyperuuid
```

Real platform-specific wheels, so it lands at native speed with nothing to compile, the same
way numpy or cryptography does — no compiler needed and no dependencies at all; the PyO3
extension *is* the package. The wheels are `abi3`, so one per platform covers every CPython
from the 3.11 floor up — eight native ones, plus a ninth for Pyodide in the browser
([below](#in-the-browser-pyodide)):

| | x64 | arm64 |
| --- | :---: | :---: |
| Linux, glibc 2.28 or newer (`manylinux_2_28`) | ✓ | ✓ |
| Linux, musl 1.2 or newer (`musllinux_1_2` — Alpine) | ✓ | ✓ |
| macOS | ✓ | ✓ |
| Windows | ✓ | ✓ |

**Only wheels are published — there is no sdist.** An interpreter none of the wheels matches
(a glibc older than 2.28, PyPy, free-threaded CPython) gets `No matching distribution found`
from pip, not a source build.

On Alpine the musl wheel needs nothing beyond the base image — it carries its own copy of
`libgcc_s` — and the whole suite passes on a bare `python:3.14-alpine`.

**Not yet covered: free-threaded (no-GIL) CPython (`3.13t`/`3.14t`).** An `abi3` wheel is
ignored by a free-threaded interpreter (it's a genuinely separate ABI, not a compatibility
flag), so closing this gap means building and shipping additional version-specific
`cp313t`/`cp314t` wheels alongside the existing ones, not just a build-flag change. PyO3
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
hyperuuid.is_rfc(id4, 7) # True: the RFC 9562 variant and version 7, in one call
stored = hyperuuid.v7_to_sql_order(id4) # byte order SQL Server's uniqueidentifier needs to sort by creation order
hyperuuid.v7_unix_millis(stored, hyperuuid.Layout.SQL_SERVER) # read in place, no conversion back

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
is the version-agnostic counterpart to `v6_timestamp`/`v7_timestamp` — it checks the
value itself, in one native call, and returns `None` for anything that isn't an RFC 9562
version 6 or 7 UUID, instead of assuming the caller already knows. The variant is part of
that check: a 6 or 7 in the version nibble under another variant carries no timestamp. `hyperuuid.new_v6_batch(count)`/
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

A caller's bug is an exception: an argument of the
wrong type is a `TypeError`; a timestamp no field can hold (negative, or past v7's 48 bits
or v6's 60), a batch `count` outside 0 to 4294967295 (0 to `MAX_V7_BATCH` for v7) and a
`layout` that is not a `Layout` are a `ValueError`; a batch too large to allocate is a
`MemoryError`, never a batch of some other size.

### Version 7 batches

A v7 batch (`new_v7_batch`, `fill_v7`) is always in strictly increasing order. The 26-bit
counter that orders UUIDs within a millisecond is one process-wide sequence that wraps every
2^26 values, so a batch can straddle the wrap; the UUIDs from the wrap on carry
`unix_millis + 1` rather than sorting before the ones ahead of them, so an embedded
timestamp can be one millisecond past the one supplied, never more. For the same reason one
batch holds at most `hyperuuid.MAX_V7_BATCH` (67,108,864) UUIDs: a larger `count`, or a
`fill_v7` buffer of more than that many, is a `ValueError` naming the limit, raised before
anything is allocated or written. Version 6 has no counter and no such limit.

## Inspecting a UUID

```python
import hyperuuid
from hyperuuid import Layout, Variant

hyperuuid.version(id4)                     # 7: the version nibble, 0-15
hyperuuid.variant(id4)                     # Variant.RFC9562
hyperuuid.is_rfc(id4, 7)                   # True: the guard before trusting v7 fields

stored = hyperuuid.v7_to_sql_order(id4)
hyperuuid.version(stored, Layout.SQL_SERVER)       # 7
hyperuuid.is_rfc(stored, 7, Layout.SQL_SERVER)     # True
hyperuuid.get_timestamp(stored, Layout.SQL_SERVER) # the embedded datetime, read in place
```

`version`, `variant` and `is_rfc` are answered by the native core, the same functions every
other binding calls, so no bit-reading is duplicated here. `version(id)` is the version
nibble whatever the variant (stdlib's `id.version` is `None` for a non-RFC variant; this is
not), Nil reads as 0 and Max as 15, and `is_rfc(id, n)` is the check that means "an RFC 9562
UUID of version n" — `False`, never an error, for an `n` outside 0 to 15. `variant(id)` is a
`Variant` — `NCS` (Nil included), `RFC9562`, `MICROSOFT` or `FUTURE` (Max included) — read in
RFC 9562 order.

`Layout` says which byte order a `uuid.UUID` is held in: `Layout.RFC9562`, what every other
function here takes and returns and the default everywhere, or `Layout.SQL_SERVER`, the order
`v6_to_sql_order`/`v7_to_sql_order` return (and a `uniqueidentifier` column holds).
`version`, `is_rfc`, `v6_unix_millis`/`v7_unix_millis`, `v6_timestamp`/`v7_timestamp` and
`get_timestamp` all take one as their last argument, and read a SQL-ordered value's permuted
bytes in place, with no conversion back first. In `Layout.SQL_SERVER` only versions 6 and 7
have an order: `version` is 6 or 7 for bytes that form a SQL-ordered v6 or v7, and 0 for bytes
that don't, and the core checks the variant where each version puts it, so a SQL-ordered v6
never reads as a v7 or the other way round. The bytes alone cannot say which layout a value
is in — an RFC-ordered random v4 forms a valid SQL-ordered v7 one time in 16 — so the caller
must keep track of the layout.

`Layout` and `Variant` are `IntEnum`s holding the native core's codes (`Layout.RFC9562 == 1`,
`Layout.SQL_SERVER == 2`; `Variant.NCS == 1` through `Variant.FUTURE == 4`), and a plain `int`
equal to a layout code is accepted where a `Layout` is. Neither has an "unspecified" member:
the layout defaults to `Layout.RFC9562`, and anything else — an unknown code is a
`ValueError`, a non-integer a `TypeError` — is refused rather than guessed at. There is no
raw-bytes form; wrap 16 bytes in `uuid.UUID(bytes=...)` first, exactly as they are held.

`hyperuuid.native_version()` names the core actually loaded, `"major.minor.patch"`, decoded
from the same packed `hyperuuid_version` export every other binding probes.
`hyperuuid.BACKEND` is always `"native"`, the PyO3 extension.

## In the browser (Pyodide)

The same PyO3 extension, compiled for Pyodide's Emscripten target, is published to PyPI as
`hyperuuid-X.Y.Z-cp311-abi3-pyemscripten_2026_0_wasm32.whl` (about 74 KB), so micropip finds
it the way pip finds the native wheels:

```python
import micropip
await micropip.install("hyperuuid")

import uuid, hyperuuid
hyperuuid.new_v7()
hyperuuid.new_v5(uuid.NAMESPACE_DNS, "example.com")
```

It is the native backend (`hyperuuid.BACKEND == "native"`), with the same API and no
JavaScript bridge: randomness comes from Emscripten's `getentropy` (the browser's
`crypto.getRandomValues`), and the clock is `Date.now()`, so v6/v7 timestamps have
millisecond resolution there, as everywhere. CI installs the wheel into Pyodide and runs this
package's whole pytest suite in it twice, under Node and in headless Chrome.

The wheel is for **Pyodide 314.x** (Python 3.14, platform `pyemscripten_2026_0`). It is
`abi3`, but a Pyodide ABI is one Python minor built with one exact Emscripten, so each
Pyodide ABI year needs its own wheel: earlier Pyodide lines (0.29.x and older) find no wheel,
and the next line is covered by the release that adds its build.

## Bulk generation into a buffer

`fill_v7(buffer)` and `fill_v6(buffer)` write raw RFC 9562-ordered bytes — 16 per UUID — straight into a `bytearray` you already own, in one native call, without constructing a single `uuid.UUID`:

```python
import hyperuuid

buf = bytearray(1000 * 16)
hyperuuid.fill_v7(buf)                      # 1000 v7 UUIDs, one timestamp capture
first = bytes(buf[0:16])                    # ready for a BYTEA / uniqueidentifier parameter
```

**This is roughly 35x faster than `new_v7_batch`**, and the reason is worth understanding, because it decides whether you should use it at all:

| path | µs / 1000 UUIDs |
| --- | ---: |
| `new_v7_batch(1000)` → `list[UUID]` | 132 |
| `fill_v7(bytearray)` | **3.7** |
| `fill_v7`, then build `uuid.UUID` objects in Python | 888 |

`new_v7_batch` does not spend its time in the native call — it spends it building a thousand `uuid.UUID` instances. Skip that and Python lands at 3.7 µs, which is the same native ceiling the Go (3.6 µs) and C# (3.7 µs) bindings hit for identical work.

The third row is the catch, and it inverts the advice: **if you need `uuid.UUID` objects, keep using `new_v7_batch`.** Filling bytes and constructing UUIDs from them in Python is more than six times as *slow*, because the extension builds them through a much faster path internally than you can from Python. Reach for `fill_v7` only when bytes are the destination — a database parameter, a wire format, a bulk `COPY` — not a step on the way to objects.

`len(buffer)` must be a multiple of 16, a zero-length buffer writes nothing, and `fill_v7` takes at most `MAX_V7_BATCH` UUIDs' worth ([above](#version-7-batches)). Both functions take an optional `datetime` or Unix-epoch millisecond timestamp, same as the rest of the API.

One deliberate limitation: these take a `bytearray`, not any writable buffer. Supporting `memoryview`, `mmap` or NumPy arrays needs `Py_buffer`, which entered CPython's stable ABI in 3.11. That is this extension's floor as of this release (`abi3-py311`), so the wider buffer protocol is no longer ruled out; it is simply not built yet.

## Why not stdlib `uuid`?

This is the one binding where the honest answer genuinely depends on which Python you're running — this package supports 3.11+, and stdlib's own v6/v7 story changed dramatically partway through that range:

- **Python 3.11-3.13:** stdlib has `uuid1`/`uuid3`/`uuid4`/`uuid5` — no v6, no v7, at all. This package is the only way to get either without a third-party dependency, and the native extension outruns stdlib's v4/v5 on top of that.
- **Python 3.14+:** stdlib added `uuid.uuid6()`/`uuid7()`/`uuid8()`, and `uuid7()` genuinely implements RFC 9562 §6.2's monotonic counter (42 bits of it) — this isn't a naive random-bits implementation, it's a real, well-built addition, and it's what the benchmarks below measure against. This package still wins across the board there — see Benchmarks — plus:
  1. **Cross-language consistency.** The same Rust core mints v5 namespace UUIDs for Go, C#, Ruby, and every other binding in this repo — verified in CI to match stdlib's own `uuid.uuid5` byte-for-byte. If your stack isn't Python-only, or you need every service minting IDs from the literal same engine rather than N independent (if individually correct) implementations, that's not something stdlib can offer regardless of version.
  2. **Batch generation.** `new_v7_batch(count)` shares one timestamp capture, one random-bytes fetch, and one counter reservation across the whole batch — stdlib's `uuid7()` has no bulk-generation entry point, so a loop of individual calls is the only option there.
  3. **One behavior across your whole supported range.** If your package needs to run on 3.11 *and* 3.14, this avoids `sys.version_info`-gated code paths for v6/v7 support.

## Benchmarks

Measured with [`pyperf`](https://github.com/psf/pyperf) (linux-x64 on an Intel Core i9-11900H, CPython 3.14.7 as Fedora builds it, `python bench_uuid.py --fast`; see `bench_uuid.py`). Linking the core directly into the extension module — no `ctypes` boundary to cross — puts every generator ahead of stdlib's own C-accelerated implementations:

| Call | hyperuuid | vs. closest stdlib equivalent |
|---|---|---|
| `hyperuuid.new_v4()` | 215 ns | `uuid.uuid4()`: 729 ns — **3.4x faster** |
| `hyperuuid.new_v5(...)` | 372 ns | `uuid.uuid5(...)`: 1.58 µs — **4.2x faster** |
| `hyperuuid.new_v6(...)` | 335 ns | `uuid.uuid6()` (3.14+): 1.30 µs — **3.9x faster** |
| `hyperuuid.new_v7(...)` | 342 ns | `uuid.uuid7()` (3.14+): 1.29 µs — **3.8x faster** |

Most of what a call costs is the `uuid.UUID` it hands back, so the extension builds that through the C API's own entry points — an instance allocated and its two slots set, no Python-level call — all of it inside the stable ABI, so one wheel still covers every CPython from 3.11.

Batch generation amortizes the rest of the per-call cost:

| | Mean | vs. individual calls |
|---|---|---|
| `new_v6_batch(1000)` | 131 µs ± 3 µs | vs. 1000x `new_v6()`: 316 µs ± 8 µs — **2.4x** |
| `new_v7_batch(1000)` | 132 µs ± 3 µs | vs. 1000x `new_v7()`: 323 µs ± 9 µs — **2.4x** |

### Timestamp extraction vs. stdlib's `.time` property

CPython 3.14's `uuid.UUID.time` has real version-aware extraction logic of its own (branches on version, computes the right thing for v6/v7, not just a v1-only stub), so this is a genuine head-to-head — each call measured against a UUID generated once outside the timed loop, so only the extraction itself is timed:

| Call | hyperuuid | vs. stdlib `.time` |
|---|---|---|
| `hyperuuid.v6_unix_millis(...)` → `int` | 164 ns | `UUID.time` (v6): 484 ns — **3.0x faster** |
| `hyperuuid.v7_unix_millis(...)` → `int` | 163 ns | `UUID.time` (v7): 396 ns — **2.4x faster** |
| `hyperuuid.v6_timestamp(...)` → `datetime` | 250 ns | `UUID.time` (v6): 484 ns — **1.9x faster** |
| `hyperuuid.v7_timestamp(...)` → `datetime` | 242 ns | `UUID.time` (v7): 396 ns — **1.6x faster** |

`.time` returns an `int`, so the `*_unix_millis` rows are the like-for-like ones; the `*_timestamp` rows hand back a timezone-aware `datetime` and are still ahead. Reach for the integer form when the value is going to be stored, compared or forwarded as a number.

Worth noting: stdlib's `.time` for v6 returns raw Gregorian-epoch 100ns ticks, not Unix
milliseconds like `hyperuuid.v6_timestamp` — different units if you actually need the value,
but a fair timing comparison of "the cost of pulling the embedded time out" either way.

Reproduce: `maturin develop --release` (from `python/`, inside a virtualenv — `pyproject.toml`
already points maturin at `../rust/Cargo.toml` and the `python` feature) to build the release
extension — `pip install -e ".[bench]"` alone builds debug by default and will understate
every number above — then `pip install pyperf` and
`python bench_uuid.py --fast -o results.json`.

## Verifying provenance

Every wheel PyPI serves, the Pyodide one included, carries a GitHub build-provenance attestation. The wheels are built,
installed and attested in CI by the shared `hyper-build-wheels.yml` workflow in
`SkunkWerkx/.github`, and `release.yml` verifies each one before publishing it unchanged, so
the verify command names that signer:

```sh
pip download hyperuuid==X.Y.Z --no-deps -d .
gh attestation verify hyperuuid-X.Y.Z-*.whl \
  --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
# or: gh attestation verify hyperuuid-X.Y.Z-*.whl --owner SkunkWerkx
```

This is a separate thing from the [PEP 740](https://peps.python.org/pep-0740/) attestations
`gh-action-pypi-publish` already sends to PyPI itself, which PyPI-side tooling checks on its
own — this is the GitHub/Sigstore transparency-log route, checked with `gh attestation
verify`, the same route every other artifact in this project uses. See
[csharp/README.md's provenance section](../csharp/README.md#native-binary-provenance) for why
an artifact signed by the shared workflow needs `--signer-repo`.

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
