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

Real platform-specific wheels, so on every platform in the table below it lands at native speed with nothing to compile, the same
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

Platforms with a wheel (the table above) install with no compiler, nothing to compile and
no dependencies. Elsewhere pip falls back to building the same extension from the source
distribution, `hyperuuid-<version>.tar.gz`, which needs Rust 1.89 or newer and a C linker; CI
proves that build on x86-64 glibc 2.17 and on musl, with CPython 3.11 at exactly Rust 1.89.
pip's build isolation fetches `maturin`. s390x (big-endian): CI runs the core's test suite,
byte-exact corpus included, on s390x under QEMU; the extension itself has not run there.
FreeBSD and ppc64le compiled (`cargo check --features python`) on 2026-10-28 and have not run;
as of that date PyPI's `maturin` 1.15.0 has no FreeBSD wheel, so there pip builds `maturin`
too, or install your platform's `py-maturin` and use `--no-build-isolation`.

Free-threaded CPython older than 3.15 has no stable ABI,
so its wheel would serve one interpreter version, too narrow an ABI to ship; free-threading
starts at 3.15, where `abi3t` covers it. Free-threaded 3.14 builds the sdist (a report-only CI
job builds and tests it there); free-threaded 3.13 cannot, since PyO3 0.30 refuses free-threaded
CPython below 3.14.

On Alpine the musl wheel needs nothing beyond the base image — it carries its own copy of
`libgcc_s` — and the whole suite passes on a bare `python:3.14-alpine`.

**Free-threaded CPython runs it without the GIL.** The extension declares it needs no GIL, so a
free-threaded interpreter keeps the GIL off after `import hyperuuid` (no `RuntimeWarning`), and
every function is safe to call from any number of threads at once: the v7 stream is one atomic
the threads share, so IDs from different threads stay unique and each thread's stay in order.
The bindings are written once, for free-threading; a GIL build runs the same code. `fill_v7`
holds the `bytearray`'s own lock and a buffer export on it while it writes, so two threads
filling the same `bytearray` take turns, and another thread resizing it meanwhile waits or
gets `BufferError` instead of freeing the memory underneath. A thread writing it at the same
time through a `memoryview`, which takes no lock, races on its contents and nothing more: the
bytes are whichever write landed last. No call releases the GIL on a GIL build: each is nanoseconds to microseconds
of pure Rust, less than the handoff would cost. CI runs the whole suite on free-threaded 3.15
on linux-x64, linux-arm64, osx-arm64, win-x64 and win-arm64 on the release board (a pull
request runs it on linux-x64), and installs each wheel on its own interpreters before it is
published.

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
UTC `datetime.datetime` from a version 7 UUID. A version 7 is minted with a timestamp up to
9999-12-31T23:59:59.999Z (253,402,300,799,999 ms), the last instant `datetime` can name, and
`get_timestamp` answers `None` for one another generator dated past that, as it does for a
version 6; the 48-bit field itself reaches 10889, so `v7_timestamp` raises `OverflowError` for
such a value. `hyperuuid.v7_unix_millis(id)` returns the same instant as
an `int` of Unix-epoch milliseconds, with no `datetime` built around it — the cheaper call,
and the one that reads the whole 48-bit field as it is. `new_v7` also accepts a
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
wrong type is a `TypeError`; a timestamp outside 1970-01-01T00:00:00Z to
9999-12-31T23:59:59.999Z, a batch `count` outside 0 to `MAX_V7_BATCH` and a
`layout` that is not a `Layout` are a `ValueError`, as is a refused SQL Server order
conversion (below); a batch too large to allocate is a
`MemoryError`, never a batch of some other size. The v7 stream running out
(see [Version 7 batches](#version-7-batches)) is a `RuntimeError`, as is the random source
failing: the state is at fault, not the arguments.

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

A v7 batch (`new_v7_batch`, `fill_v7`) is always in strictly increasing order, and every v7
call, single or batch, reserves from one process-wide stream that is re-seeded at random
whenever a timestamp moves it forward. A single call consumes one of a millisecond's 2^20
steps and a batch of n consumes ⌈n/64⌉. For timestamps up to 2^44 − 2 (the year 2527):

- **Ordering.** A call whose timestamp is within 10 s of the newest stream timestamp either
  raises `RuntimeError` (exhaustion, below) or reserves from the stream, and its UUIDs then
  sort strictly after every UUID from a reservation that completed before it began, on any
  thread. Overlapping reservations are ordered by reservation, which need not be completion
  order.
- **Embedded timestamp.** At most 11 s ahead of the one supplied: a timestamp up to 11 s
  behind the stream is clamped onto the stream's millisecond, and a millisecond whose steps
  run out splits into the next, so a split's tail carries the millisecond after it (at most
  1000 ms ahead of the supplied timestamp). An out-of-stream call carries its own timestamp
  exactly.
- **Backfill and future-dated calls.** A timestamp more than 11 s behind the stream, or past
  2^44 − 2, is minted at itself, outside the stream: it succeeds, leaves the stream alone and
  is ordered against nothing. One call far in the future therefore advances the stream, and
  until the clock catches up, real-time calls are minted out of stream, unordered among
  themselves.
- **Exhaustion.** A split that would take the stream more than 1000 ms ahead of the supplied
  timestamp, or past 2^44 − 2, raises `RuntimeError` with the message `version 7 counter
  exhausted: the stream cannot advance without running more than 1000 ms ahead of the supplied
  timestamp or past its last millisecond`, before anything is reserved. It is a state error,
  not an argument error (the random source failing is a `RuntimeError` too), and a call whose
  timestamp is past the stream's millisecond succeeds. After a `RuntimeError` from `fill_v7`,
  treat the buffer as clobbered.
- **Batch size.** One batch holds at most `hyperuuid.MAX_V7_BATCH` (67,108,864) UUIDs: a
  larger `count`, or a `fill_v7` buffer of more than that many, is a `ValueError` naming the
  limit, raised before anything is allocated or written.

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

**This is about 36x faster than `new_v7_batch`**, and the reason is worth understanding, because it decides whether you should use it at all: (Measured before the per-millisecond re-seed, core change, 2026-10-27.)

| path | µs / 1000 UUIDs |
| --- | ---: |
| `new_v7_batch(1000)` → `list[UUID]` † | 132 |
| `fill_v7(bytearray)` † | **3.7** |
| `fill_v7`, then build `uuid.UUID` objects in Python † | 888 |

† Measured before the per-millisecond re-seed (core change, 2026-10-27).

`new_v7_batch` does not spend its time in the native call — it spends it building a thousand `uuid.UUID` instances. Skip that and Python lands at 3.7 µs, which is the same native ceiling the C# binding (3.8 µs) hits for identical work on this machine. (Measured before the per-millisecond re-seed, core change, 2026-10-27.)

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
| `hyperuuid.new_v7(...)` † | 342 ns | `uuid.uuid7()` (3.14+): 1.29 µs — **3.8x faster** |

† Measured before the per-millisecond re-seed (core change, 2026-10-27).

Most of what a call costs is the `uuid.UUID` it hands back, so the extension builds that through the C API's own entry points — an instance allocated and its two slots set, no Python-level call — all of it inside the stable ABI, so one wheel still covers every CPython from 3.11.

Batch generation amortizes the rest of the per-call cost:

| | Mean | vs. individual calls |
|---|---|---|
| `new_v7_batch(1000)` † | 132 µs ± 3 µs | vs. 1000x `new_v7()`: 323 µs ± 9 µs — **2.4x** |

† Measured before the per-millisecond re-seed (core change, 2026-10-27).

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

Every wheel PyPI serves, the Pyodide one included, and the sdist verify from the release tag alone: the
release attestation, signed by this repository's `release.yml` at the tag, names every one of
them, with the release inventory as its predicate.

```sh
pip download hyperuuid==<version> --no-deps -d .
curl -sLO "$(curl -s https://pypi.org/pypi/hyperuuid/<version>/json |
  jq -r '.urls[] | select(.packagetype=="sdist") | .url')"
for f in hyperuuid-<version>*; do
  gh attestation verify "$f" --repo SkunkWerkx/HyperUuid \
    --source-ref refs/tags/v<version> --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
done
```

Each wheel and the sdist also carry build provenance where each was built, installed and attested:
`hyper-build-wheels.yml` in `SkunkWerkx/.github`, at the preparation commit on master the
release was prepared from (`release.yml` verifies each wheel and the sdist against it before publishing it
unchanged). That check names the signer, or uses `--owner` for both:

```sh
for f in hyperuuid-<version>*; do
  gh attestation verify "$f" \
    --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
  # or: gh attestation verify "$f" --owner SkunkWerkx
done
```

Both are the GitHub/Sigstore transparency-log route, checked with `gh attestation verify`, the
same route every other artifact in this project uses. Separately, PyPI itself holds a
[PEP 740](https://peps.python.org/pep-0740/) publish attestation for every wheel and the sdist, filed by
`gh-action-pypi-publish` at publication and shown on each file's PyPI page, which PyPI-side
tooling checks on its own. Those name the uploading workflow, and PyPI marks a release whose
workflow differs from the previous one's: 1.0.0's name `pypi-recovery.yml`, the one-off
workflow that uploaded that release's wheels after `release.yml`'s own upload failed before
sending anything (the same packed and attested bytes, after the same checks), and 1.1.0's name
`release.yml` again, so PyPI marks both. Every file of either still verifies from its tag with
the commands above. The root README's [Provenance](../README.md#provenance) section has
the whole picture.

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
