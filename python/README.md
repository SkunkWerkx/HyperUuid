# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/hyperuuid.svg)](https://pypi.org/project/hyperuuid/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

**Python 3.14 finally added `uuid.uuid7()` to stdlib, with a real monotonic counter — genuinely well done. If you're stuck on 3.11-3.13 like most production code still is, stdlib has no v7 at all, and this package gives you one today without waiting for a runtime upgrade — and on any version, the native extension below outruns stdlib outright.**

RFC 9562 UUID v4 (random), v5 (deterministic) and v7 (time-sortable) generation, with a
version 8 SQL Server order for v7. A native extension built with [PyO3](https://pyo3.rs) — the Rust core linked directly into the
CPython extension module, no `dlopen`, no C-ABI hop, no `ctypes` marshalling, no runtime
bridge.

## Install

```sh
pip install hyperuuid
```

Real platform-specific wheels, so it lands at native speed with nothing to compile, the same
way numpy or cryptography does — no compiler needed and no dependencies at all; the PyO3
extension *is* the package. Each platform gets two wheels, both stable-ABI, which between
them cover every CPython from the 3.11 floor up, with the GIL or without it:

| Wheel | Interpreters |
| --- | --- |
| `cp311-abi3` | CPython 3.11 and newer, with the GIL |
| `cp315-abi3.abi3t` | CPython 3.15 and newer, free-threaded or not ([PEP 803](https://peps.python.org/pep-0803/)) |

on each of eight platforms, plus a seventeenth wheel for Pyodide in the browser
([below](#in-the-browser-pyodide)):

| | x64 | arm64 |
| --- | :---: | :---: |
| Linux, glibc 2.28 or newer (`manylinux_2_28`) | ✓ | ✓ |
| Linux, musl 1.2 or newer (`musllinux_1_2` — Alpine) | ✓ | ✓ |
| macOS | ✓ | ✓ |
| Windows | ✓ | ✓ |

**Only wheels are published — there is no sdist.** An interpreter none of the wheels matches
(a glibc older than 2.28, PyPy, free-threaded 3.13 or 3.14) gets `No matching distribution
found` from pip, not a source build. Free-threaded CPython older than 3.15 has no stable ABI,
so its wheel would serve one interpreter version, too narrow an ABI to ship; free-threading
starts at 3.15, where `abi3t` covers it.

On Alpine the musl wheel needs nothing beyond the base image — it carries its own copy of
`libgcc_s` — and the whole suite passes on a bare `python:3.14-alpine`.

**Free-threaded CPython runs it without the GIL.** The extension declares it needs no GIL, so a
free-threaded interpreter keeps the GIL off after `import hyperuuid` (no `RuntimeWarning`), and
every function is safe to call from any number of threads at once: the v7 counter is one atomic
the threads share, so IDs from different threads stay unique and each thread's stay in order.
The bindings are written once, for free-threading; a GIL build runs the same code. `fill_v7`
holds the `bytearray`'s own lock and a buffer export on it while it writes, so two threads
filling the same `bytearray` take turns, and another thread resizing it meanwhile waits or
gets `BufferError` instead of freeing the memory underneath. A thread writing it at the same
time through a `memoryview`, which takes no lock, races on its contents and nothing more: the
bytes are whichever write landed last. No call releases the GIL on a GIL build: each is nanoseconds to microseconds
of pure Rust, less than the handoff would cost. CI runs the whole suite on free-threaded 3.15
on linux-x64, linux-arm64, osx-arm64, win-x64 and win-arm64, and installs each wheel on its own interpreters before it is published.

The package is typed: it ships `py.typed` and a stub for the extension module, so mypy and
pyright see `uuid.UUID` and `datetime.datetime` rather than `Any`.

## Usage

```python
import uuid

import hyperuuid

id1 = hyperuuid.new_v4()
id2 = hyperuuid.new_v5(uuid.NAMESPACE_DNS, "example.com")
id4 = hyperuuid.new_v7()
print("v4:", id1)
print("v5:", id2)
print("v7:", id4)

# recover the embedded UTC datetime.datetime
print("timestamp:", hyperuuid.v7_timestamp(id4))
# the same instant as Unix-epoch milliseconds, an int
print("unix millis:", hyperuuid.v7_unix_millis(id4))
# the same datetime, or None for any value that isn't an RFC 9562 v7
print("checked timestamp:", hyperuuid.get_timestamp(id4))
# True: the RFC 9562 variant and version 7, in one call
print("is v7:", hyperuuid.is_rfc(id4, 7))

# byte order SQL Server's uniqueidentifier needs to sort by creation order
stored = hyperuuid.v7_to_sql_order(id4)
print("sql order:", stored)
# read in place, no conversion back
print("sql is v7:", hyperuuid.is_rfc(stored, 7, hyperuuid.Layout.SQL_SERVER))
print("sql timestamp:", hyperuuid.get_timestamp(stored, hyperuuid.Layout.SQL_SERVER))
print("sql unix millis:", hyperuuid.v7_unix_millis(stored, hyperuuid.Layout.SQL_SERVER))

# One native call, one random-bytes fetch, one counter reservation for the whole batch:
batch = hyperuuid.new_v7_batch(1000)
print("batch:", len(batch))
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
`datetime.datetime` cannot). `hyperuuid.v7_unix_millis(id)` returns the same instant as
an `int` of Unix-epoch milliseconds, with no `datetime` built around it — the cheaper call,
and the one that carries the whole 48-bit field. `new_v7` also accepts a
`datetime.datetime` directly in place of a raw millisecond count — converted in exact
integer arithmetic and truncated to the millisecond, never rounded into the next one; a
naive `datetime` is read as local time, the way `datetime.timestamp()` reads it.
`get_timestamp(id)`
is the checked counterpart to `v7_timestamp` — it checks the value itself, in one native
call, and returns `None` for anything that isn't an RFC 9562 version 7 UUID, instead of
assuming the caller already knows. Only version 7 carries a timestamp here: a version 6 or 1
from another generator is `None` too. The variant is part of that check: a 7 in the version
nibble under another variant carries no timestamp. `hyperuuid.new_v7_batch(count)`
generates `count` UUIDs sharing one timestamp capture and one native call, instead of
`count` of each.

A caller's bug is an exception: an argument of the
wrong type is a `TypeError`; a timestamp no field can hold (negative, or past v7's 48
bits), a batch `count` outside 0 to `MAX_V7_BATCH` and a
`layout` that is not a `Layout` are a `ValueError`, as is a refused SQL Server order
conversion (below); a batch too large to allocate is a
`MemoryError`, never a batch of some other size.

### SQL Server order

`hyperuuid.v7_to_sql_order(id)`/`v7_from_sql_order(id)` convert a version 7 UUID to and
from the byte order SQL Server's `uniqueidentifier` needs on the wire to sort by creation
order — computed once in the native Rust core rather than reimplemented in Python, and verified
there (and independently against the real `System.Data.SqlTypes.SqlGuid` comparator in
the C# binding's test suite). The converted value is an RFC 9562 version 8, the version for
custom layouts: the 8 and the RFC variant sit at octets 7 and 8, where SQL Server's text
form and a .NET `Guid` show them (`uuid.UUID(bytes_le=stored.bytes).version` is 8), and
the other 122 bits are filled in the comparator's order: the timestamp and counter first,
then the 48 random bits. Those are all 122 of a v7's free bits, so every v7 converts,
`version(stored, Layout.SQL_SERVER)` still answers 7, and the conversion back is exact.

Each conversion takes only a version 7, in its own order, and raises `ValueError`
otherwise, the same exception as a timestamp `new_v7` can't hold, with the message
`the value is not a UUID of the version this conversion takes`: `v7_to_sql_order` given
anything but an RFC 9562 v7, and `v7_from_sql_order` anything but a SQL-ordered value
converted from one (an RFC-ordered value included).

### Version 7 batches

A v7 batch (`new_v7_batch`, `fill_v7`) is always in strictly increasing order. The 26-bit
counter that orders UUIDs within a millisecond is one process-wide sequence that wraps every
2^26 values, so a batch can straddle the wrap; the UUIDs from the wrap on carry
`unix_millis + 1` rather than sorting before the ones ahead of them, so an embedded
timestamp can be one millisecond past the one supplied, never more. For the same reason one
batch holds at most `hyperuuid.MAX_V7_BATCH` (67,108,864) UUIDs: a larger `count`, or a
`fill_v7` buffer of more than that many, is a `ValueError` naming the limit, raised before
anything is allocated or written.

The roll-forward orders one batch, not the stream. The next batch or `new_v7` call in the
same real millisecond starts its counter just past the wrap and carries the supplied
timestamp, so it sorts before the previous batch's tail, which was stamped a millisecond
later; two single `new_v7` calls either side of the wrap in one millisecond sort in reverse
the same way. It happens at most once per `MAX_V7_BATCH` UUIDs the process mints.

## Inspecting a UUID

```python
import hyperuuid
from hyperuuid import Layout

hyperuuid.version(id4)  # 7: the version nibble, 0-15
hyperuuid.variant(id4)  # Variant.RFC9562
hyperuuid.is_rfc(id4, 7)  # True: the guard before trusting v7 fields

stored = hyperuuid.v7_to_sql_order(id4)
hyperuuid.version(stored, Layout.SQL_SERVER)  # 7
hyperuuid.is_rfc(stored, 7, Layout.SQL_SERVER)  # True
hyperuuid.get_timestamp(stored, Layout.SQL_SERVER)  # the embedded datetime, read in place
```

`version`, `variant` and `is_rfc` are answered by the native core, the same functions every
other binding calls, so no bit-reading is duplicated here. `version(id)` is the version
nibble whatever the variant (stdlib's `id.version` is `None` for a non-RFC variant; this is
not), Nil reads as 0 and Max as 15, and `is_rfc(id, n)` is the check that means "an RFC 9562
UUID of version n" — `False`, never an error, for an `n` outside 0 to 15. `variant(id)` is a
`Variant` — `NCS` (Nil included), `RFC9562`, `MICROSOFT` or `FUTURE` (Max included) — read in
RFC 9562 order only, with no layout form: `is_rfc(id, n, Layout.SQL_SERVER)` is the check
for a SQL-ordered value or a `uniqueidentifier` read-back, and it tests the 8 and the variant
where that layout puts them.

`Layout` says which byte order a `uuid.UUID` is held in: `Layout.RFC9562`, what every other
function here takes and returns and the default everywhere, or `Layout.SQL_SERVER`, the order
`v7_to_sql_order` returns (and a `uniqueidentifier` column holds). `version`, `is_rfc`,
`v7_unix_millis`, `v7_timestamp` and `get_timestamp` all take one as their last argument, and read a SQL-ordered value's bytes
in place, with no conversion back first. In `Layout.SQL_SERVER` `version` is the version a
value was converted from, 7, once the core has found its 8 and variant; bytes without the 8
and variant read as 0, with no timestamp. The bytes alone cannot say which
layout a value is in — an RFC-ordered random v4 carries an 8 and the variant where the
layout puts them one time in 16 — so the caller must keep track of the layout, carry it with
the value, and pass it at every call site that can see a SQL-ordered value. With the default layout, `version` and `is_rfc` read RFC order,
so `is_rfc(sql_ordered, 7)` reads the wrong bytes; there is deliberately no layout-agnostic
check.

`Layout` and `Variant` are `IntEnum`s holding the native core's codes (`Layout.RFC9562 == 1`,
`Layout.SQL_SERVER == 2`; `Variant.NCS == 1` through `Variant.FUTURE == 4`), and a plain `int`
equal to a layout code is accepted where a `Layout` is. Neither has an "unspecified" member:
the layout defaults to `Layout.RFC9562`, and anything else — an unknown code is a
`ValueError`, a non-integer a `TypeError` — is refused rather than guessed at. There is no
raw-bytes form; wrap 16 bytes in `uuid.UUID(bytes=...)` first, exactly as they are held.

`hyperuuid.native_version()` names the core actually loaded, `"major.minor.patch"`, decoded
from the same packed `hyperuuid_version` export every other binding probes.

## Binding to SQL Server

Bind `v7_to_sql_order`'s result as its `.bytes`, not as the `uuid.UUID`. pyodbc 5.3,
mssql-python 1.15 and pymssql 2.4 all send a `uuid.UUID`, and a `str`, in SQL Server's
mixed-endian text order, which stores the first three groups byte-reversed and loses the
creation order ([details](https://github.com/SkunkWerkx/HyperUuid#sql-server-ordering)). `bytes` go in as binary and land byte for byte (`cursor` is an open DB-API cursor on the
database):

```python
stored = hyperuuid.v7_to_sql_order(id4)
cursor.execute("INSERT INTO orders (id) VALUES (?)", stored.bytes)

cursor.execute("SELECT CAST(id AS binary(16)) FROM orders")
stored = uuid.UUID(bytes=cursor.fetchone()[0])  # ready for Layout.SQL_SERVER
```

pymssql takes `%s` placeholders where the other two take `?`. Reading needs the `CAST` for the
same reason: all three drivers return a `uniqueidentifier` with those groups reversed.

## In the browser (Pyodide)

The same PyO3 extension, compiled for Pyodide's Emscripten target, is published to PyPI as
`hyperuuid-<version>-cp311-abi3-pyemscripten_2026_0_wasm32.whl` (about 74 KB), so micropip finds
it the way pip finds the native wheels:

```python
import micropip

await micropip.install("hyperuuid")

import uuid, hyperuuid

hyperuuid.new_v7()
hyperuuid.new_v5(uuid.NAMESPACE_DNS, "example.com")
```

It is the same native extension, with the same API and no
JavaScript bridge: randomness comes from Emscripten's `getentropy` (the browser's
`crypto.getRandomValues`), and the clock is `Date.now()`, so v7 timestamps have
millisecond resolution there, as everywhere. CI installs the wheel into Pyodide and runs this
package's whole pytest suite in it twice, under Node and in headless Chrome.

The wheel is for **Pyodide 314.x** (Python 3.14, platform `pyemscripten_2026_0`). It is
`abi3`, but a Pyodide ABI is one Python minor built with one exact Emscripten, so each
Pyodide ABI year needs its own wheel: earlier Pyodide lines (0.29.x and older) find no wheel,
and the next line is covered by the release that adds its build.

## Bulk generation into a buffer

`fill_v7(buffer)` writes raw RFC 9562-ordered version 7 bytes — 16 per UUID — straight into a `bytearray` you already own, in one native call, without constructing a single `uuid.UUID`:

```python
import hyperuuid

buf = bytearray(1000 * 16)
hyperuuid.fill_v7(buf)  # 1000 v7 UUIDs, one timestamp capture
first = bytes(buf[0:16])  # ready for a BYTEA / uniqueidentifier parameter
```

**This is about 36x faster than `new_v7_batch`**, and the reason is worth understanding, because it decides whether you should use it at all:

| path | µs / 1000 UUIDs |
| --- | ---: |
| `new_v7_batch(1000)` → `list[UUID]` | 132 |
| `fill_v7(bytearray)` | **3.7** |
| `fill_v7`, then build `uuid.UUID` objects in Python | 888 |

`new_v7_batch` does not spend its time in the native call — it spends it building a thousand `uuid.UUID` instances. Skip that and Python lands at 3.7 µs, which is the same native ceiling the Go (3.7 µs) and C# (3.8 µs) bindings hit for identical work.

The third row is the catch, and it inverts the advice: **if you need `uuid.UUID` objects, keep using `new_v7_batch`.** Filling bytes and constructing UUIDs from them in Python is more than six times as *slow*, because the extension builds them through a much faster path internally than you can from Python. Reach for `fill_v7` only when bytes are the destination — a database parameter, a wire format, a bulk `COPY` — not a step on the way to objects.

`len(buffer)` must be a multiple of 16, a zero-length buffer writes nothing, and it takes at most `MAX_V7_BATCH` UUIDs' worth ([above](#version-7-batches)). It takes an optional `datetime` or Unix-epoch millisecond timestamp, same as the rest of the API.

One deliberate limitation: these take a `bytearray`, not any writable buffer. Supporting `memoryview`, `mmap` or NumPy arrays needs `Py_buffer`, which entered CPython's stable ABI in 3.11. That is this extension's floor (`abi3-py311`), so the wider buffer protocol is not ruled out; it is simply not built yet.

## Why not stdlib `uuid`?

This is the one binding where the honest answer genuinely depends on which Python you're running — this package supports 3.11+, and stdlib's own v7 story changed dramatically partway through that range:

- **Python 3.11-3.13:** stdlib has `uuid1`/`uuid3`/`uuid4`/`uuid5` — no v7 at all. This package is the only way to get one without a third-party dependency, and the native extension outruns stdlib's v4/v5 on top of that.
- **Python 3.14+:** stdlib added `uuid.uuid6()`/`uuid7()`/`uuid8()`, and `uuid7()` genuinely implements RFC 9562 §6.2's monotonic counter (42 bits of it) — this isn't a naive random-bits implementation, it's a real, well-built addition, and it's what the benchmarks below measure against. This package still wins across the board there — see Benchmarks — plus:
  1. **Cross-language consistency.** The same Rust core mints v5 namespace UUIDs for Go, C#, Ruby, and every other binding in this repo — verified in CI to match stdlib's own `uuid.uuid5` byte-for-byte. If your stack isn't Python-only, or you need every service minting IDs from the literal same engine rather than N independent (if individually correct) implementations, that's not something stdlib can offer regardless of version.
  2. **Batch generation.** `new_v7_batch(count)` shares one timestamp capture, one random-bytes fetch, and one counter reservation across the whole batch — stdlib's `uuid7()` has no bulk-generation entry point, so a loop of individual calls is the only option there.
  3. **One behavior across your whole supported range.** If your package needs to run on 3.11 *and* 3.14, this avoids `sys.version_info`-gated code paths for v7 support.

## Benchmarks

Measured with [`pyperf`](https://github.com/psf/pyperf) (linux-x64 on an Intel Core i9-11900H, CPython 3.14.7 as Fedora builds it, `python bench_uuid.py --fast`; see `bench_uuid.py`). Linking the core directly into the extension module — no `ctypes` boundary to cross — puts every generator ahead of stdlib's own C-accelerated implementations:

| Call | hyperuuid | vs. closest stdlib equivalent |
|---|---|---|
| `hyperuuid.new_v4()` | 215 ns | `uuid.uuid4()`: 729 ns — **3.4x faster** |
| `hyperuuid.new_v5(...)` | 372 ns | `uuid.uuid5(...)`: 1.58 µs — **4.2x faster** |
| `hyperuuid.new_v7(...)` | 342 ns | `uuid.uuid7()` (3.14+): 1.29 µs — **3.8x faster** |

Most of what a call costs is the `uuid.UUID` it hands back, so the extension builds that through the C API's own entry points — an instance allocated and its two slots set, no Python-level call — all of it inside the stable ABI, so one wheel still covers every CPython from 3.11.

Batch generation amortizes the rest of the per-call cost:

| | Mean | vs. individual calls |
|---|---|---|
| `new_v7_batch(1000)` | 132 µs ± 3 µs | vs. 1000x `new_v7()`: 323 µs ± 9 µs — **2.4x** |

### Timestamp extraction vs. stdlib's `.time` property

CPython 3.14's `uuid.UUID.time` has real version-aware extraction logic of its own (branches on version, computes the right thing for v7, not just a v1-only stub), so this is a genuine head-to-head — each call measured against a UUID generated once outside the timed loop, so only the extraction itself is timed:

| Call | hyperuuid | vs. stdlib `.time` |
|---|---|---|
| `hyperuuid.v7_unix_millis(...)` → `int` | 163 ns | `UUID.time` (v7): 396 ns — **2.4x faster** |
| `hyperuuid.v7_timestamp(...)` → `datetime` | 242 ns | `UUID.time` (v7): 396 ns — **1.6x faster** |

`.time` returns an `int`, so the `v7_unix_millis` row is the like-for-like one; the `v7_timestamp` row hands back a timezone-aware `datetime` and is still ahead. Reach for the integer form when the value is going to be stored, compared or forwarded as a number.

Reproduce: `maturin develop --release` (from `python/`, inside a virtualenv — `pyproject.toml`
already points maturin at `../rust/Cargo.toml` and the `python` feature) to build the release
extension — `pip install -e ".[bench]"` alone builds debug by default and will understate
every number above — then `pip install pyperf` and
`python bench_uuid.py --fast -o results.json`.

## Verifying provenance

Every wheel PyPI serves, the Pyodide one included, verifies from the release tag alone: the
release attestation, signed by this repository's `release.yml` at the tag, names every one of
them, with the release inventory as its predicate.

```sh
pip download hyperuuid==<version> --no-deps -d .
gh attestation verify hyperuuid-<version>-*.whl --repo SkunkWerkx/HyperUuid \
  --source-ref refs/tags/v<version> --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
```

Each wheel also carries build provenance from where it was built, installed and attested:
`hyper-build-wheels.yml` in `SkunkWerkx/.github`, at the preparation commit on master the
release was prepared from (`release.yml` verifies each wheel against it before publishing it
unchanged). That check names the signer, or uses `--owner` for both:

```sh
gh attestation verify hyperuuid-<version>-*.whl \
  --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
# or: gh attestation verify hyperuuid-<version>-*.whl --owner SkunkWerkx
```

Both are the GitHub/Sigstore transparency-log route, checked with `gh attestation verify`, the
same route every other artifact in this project uses. Separately, PyPI itself holds a
[PEP 740](https://peps.python.org/pep-0740/) publish attestation for every wheel, filed by
`gh-action-pypi-publish` at publication and shown on each file's PyPI page, which PyPI-side
tooling checks on its own. The root README's [Provenance](../README.md#provenance) section has
the whole picture.

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
