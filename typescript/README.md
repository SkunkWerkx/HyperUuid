# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![npm](https://img.shields.io/npm/v/hyperuuid.svg)](https://www.npmjs.com/package/hyperuuid)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

RFC 9562 UUID v4 (random), v5 (deterministic) and v7 (time-sortable) generation, with a
version 8 SQL Server order for v7. A Node-API addon built with [napi-rs](https://napi.rs): the
Rust core every other HyperUuid binding calls, linked straight into the `.node` file, with
types for TypeScript and the same API under Node and Bun.

It is not the fastest way to mint a single v4 in JavaScript: `crypto.randomUUID()` beats it
per call on this machine, and the `uuid` package's `v7()` beats its v7 on Bun, where on Node
it is 2.3x faster ([Benchmarks](#benchmarks)). What it gives
you is the same engine, bit for bit, as the C#, Go, Java, PHP, Python, Ruby and Swift
bindings, the SQL Server order, and batch and buffer generation that pay for randomness once.

## Install

```sh
npm install hyperuuid
```

The package itself is JavaScript and types; the core comes from one of ten packages. The
native addon is a platform package that npm picks for you through `optionalDependencies`, so
nothing compiles at install, one for each of eight platforms, as
`@skunkwerkx/hyperuuid-<platform>`. The tenth, `@skunkwerkx/hyperuuid-wasm32-wasip1`, is the
WebAssembly fallback: a plain dependency of the root, so it is always installed, and loaded only
when no native addon fits.

| | x64 | arm64 |
| --- | :---: | :---: |
| Linux, glibc (`linux-*-gnu`) | ✓ | ✓ |
| Linux, musl (`linux-*-musl`, Alpine) | ✓ | ✓ |
| macOS (`darwin-*`) | ✓ | ✓ |
| Windows (`win32-*-msvc`) | ✓ | ✓ |
| Everything else: the WebAssembly fallback (`wasm32-wasip1`) | ✓ | ✓ |

The fallback is independent of the CPU, so the last row covers more than its two columns say:
the whole suite passes on it on Linux x64 (glibc), macOS arm64 and Windows x64, and the
consumer smoke on Alpine; elsewhere little-endian it should load, and is not tested. On a
big-endian host (s390x) it loads and the consumer smoke's checks pass, and the process then
never exits: a printed loss, [below](#when-no-native-addon-fits).
It runs on Node 22.13 or newer
(`^22.13.0 || >=23.5.0`, the floor of the WebAssembly runtime), or Bun. An install run with
`--omit=optional`, or on a platform outside the eight, gets the fallback instead of the addon
([below](#when-no-native-addon-fits)).

## Usage

```js
const hyperuuid = require('hyperuuid')

const id1 = hyperuuid.newV4()
const id2 = hyperuuid.newV5(hyperuuid.Namespaces.dns, 'example.com')
const id4 = hyperuuid.newV7()
console.log('v4:', id1)
console.log('v5:', id2)
console.log('v7:', id4)

// the embedded instant as a Date
console.log('timestamp:', hyperuuid.v7Timestamp(id4))
// the same instant as Unix-epoch milliseconds, a number
console.log('unix millis:', hyperuuid.v7UnixMillis(id4))
// the same Date, or null for any value that isn't an RFC 9562 v7
console.log('checked timestamp:', hyperuuid.getTimestamp(id4))
// true: the RFC 9562 variant and version 7, in one call
console.log('is v7:', hyperuuid.isRfc(id4, 7))

// byte order SQL Server's uniqueidentifier needs to sort by creation order
const stored = hyperuuid.v7ToSqlOrder(id4)
console.log('sql order:', stored)
// read in place, no conversion back
const { SqlServer } = hyperuuid.Layout
console.log('sql is v7:', hyperuuid.isRfc(stored, 7, SqlServer))
console.log('sql timestamp:', hyperuuid.getTimestamp(stored, SqlServer))
console.log('sql unix millis:', hyperuuid.v7UnixMillis(stored, SqlServer))

// One native call, one random-bytes fetch, one counter reservation for the whole batch:
const batch = hyperuuid.newV7Batch(1000)
console.log('batch:', batch.length)
```

ESM works the same way, named imports included:

```js
import { newV7, v7Timestamp } from 'hyperuuid'

const id = newV7(new Date('2022-02-22T19:22:22Z'))
console.log(id.slice(0, 13), v7Timestamp(id).toISOString()) // 017f22e2-79b0 2022-02-22T19:22:22.000Z
```

Ids come back as lowercase hyphenated strings, or as a `Uint8Array` of 16 bytes from the
`Bytes` doors. Ids go in as either: a string of 32 hex digits or the 8-4-4-4-12 hyphenated
form, in any case, or exactly 16 bytes (a `Uint8Array`, so a `Buffer` too). Anything else is
`ERR_HYPERUUID_INVALID_ID`; braces, `urn:uuid:` and other lengths are refused rather than
guessed at. `Namespaces` holds the RFC 9562 section 6.6 namespaces (`dns`, `url`, `oid`,
`x500`) for `newV5`; a string name is hashed as its UTF-8 bytes, a `Uint8Array` name as given.
`NIL` and `MAX` are the section 5.9 and 5.10 special values.

Time goes in as a `number` of Unix-epoch milliseconds or a `Date`, and leaving it out means
now. A fractional number is floored to the millisecond, never rounded into the next one. A
time outside 1970-01-01T00:00:00Z to 9999-12-31T23:59:59.999Z (253,402,300,799,999 ms, the
generation cap), or no time at all (`NaN`, an infinity, an invalid `Date`), is
`ERR_HYPERUUID_TIMESTAMP_RANGE`. The 48-bit field itself reaches 10889: `getTimestamp` reads a
v7 another generator dated past the cap as it reads a v6, `null`, and `v7UnixMillis` reads the
field as it is.

### API

| Door | Signature | Error codes |
| --- | --- | --- |
| `newV4` | `(): string` | `RANDOM_SOURCE` |
| `newV4Bytes` | `(): Uint8Array` | `RANDOM_SOURCE` |
| `newV7` | `(time?: number \| Date): string` | `TIMESTAMP_RANGE`, `COUNTER_EXHAUSTED`, `RANDOM_SOURCE` |
| `newV7Bytes` | `(time?: number \| Date): Uint8Array` | `TIMESTAMP_RANGE`, `COUNTER_EXHAUSTED`, `RANDOM_SOURCE` |
| `newV7Batch` | `(count: number, time?: number \| Date): string[]` | `INVALID_COUNT`, `BATCH_TOO_LARGE`, `TIMESTAMP_RANGE`, `COUNTER_EXHAUSTED`, `RANDOM_SOURCE` |
| `newV7BatchBytes` | `(count: number, time?: number \| Date): Uint8Array` | `INVALID_COUNT`, `BATCH_TOO_LARGE`, `TIMESTAMP_RANGE`, `COUNTER_EXHAUSTED`, `RANDOM_SOURCE` |
| `fillV7` | `(buffer: Uint8Array, time?: number \| Date): void` | `INVALID_BUFFER`, `BATCH_TOO_LARGE`, `TIMESTAMP_RANGE`, `COUNTER_EXHAUSTED`, `RANDOM_SOURCE` |
| `newV5` | `(namespace: string \| Uint8Array, name: string \| Uint8Array): string` | `INVALID_ID` |
| `version` | `(id: string \| Uint8Array, layout?: Layout): number` | `INVALID_ID` |
| `variant` | `(id: string \| Uint8Array): Variant` | `INVALID_ID` |
| `isRfc` | `(id: string \| Uint8Array, version: number, layout?: Layout): boolean` | `INVALID_ID` |
| `getTimestamp` | `(id: string \| Uint8Array, layout?: Layout): Date \| null` | `INVALID_ID` |
| `v7Timestamp` | `(id: string \| Uint8Array, layout?: Layout): Date` | `INVALID_ID` |
| `v7UnixMillis` | `(id: string \| Uint8Array, layout?: Layout): number` | `INVALID_ID` |
| `v7ToSqlOrder` | `(id: string \| Uint8Array): string` | `INVALID_ID`, `WRONG_VERSION` |
| `v7FromSqlOrder` | `(id: string \| Uint8Array): string` | `INVALID_ID`, `WRONG_VERSION` |
| `nativeVersion` | `(): string` | none |

Every code in the table is short for `ERR_HYPERUUID_<code>`, and every door can also throw
`ERR_HYPERUUID_UNAVAILABLE` when the addon did not load. Besides the doors: `isAvailable():
boolean`, `isHyperUuidError(value): value is HyperUuidError`, `MAX_V7_BATCH` (67108864),
`Namespaces`, `NIL`, `MAX`, and the `Layout` and `Variant` enums. `nativeVersion()` is the
loaded core's `major.minor.patch`.

### Errors

A HyperUuid error is an `Error` with a `code` from the table, and the same message the Python
binding raises where one exists:

```js
const { newV7, version, isHyperUuidError } = require('hyperuuid')

try {
  version('not a uuid')
} catch (error) {
  // true ERR_HYPERUUID_INVALID_ID id must be 32 hex digits, 36 with hyphens, or exactly 16 bytes
  console.log(isHyperUuidError(error), error.code, error.message)
}
try {
  newV7(-1)
} catch (error) {
  // ERR_HYPERUUID_TIMESTAMP_RANGE unix_millis must be non-negative and at most 253402300799999 (9999-12-31T23:59:59.999Z)
  console.log(error.code, error.message)
}
try {
  version(42)
} catch (error) {
  console.log(isHyperUuidError(error), error.code) // false InvalidArg
}
```

An argument of the wrong JavaScript type, or a number that is not a member of `Layout`, is
napi's own status code, `InvalidArg` or `NumberExpected` (numbers and enums report
`NumberExpected`; string-or-bytes parameters report `InvalidArg`), not a HyperUuid code: that is a type error the compiler would have
caught. On the native addon, every door catches a Rust panic and throws it as napi's
`GenericFailure` rather than taking the process down; the core is built to have none, so that
is a backstop. On the WebAssembly fallback a panic aborts instead
([below](#when-no-native-addon-fits)).

Counts are checked, not coerced: `newV7Batch(1.5)`, `-1`, `NaN` and `Infinity` are
`ERR_HYPERUUID_INVALID_COUNT`, and anything past `MAX_V7_BATCH` is
`ERR_HYPERUUID_BATCH_TOO_LARGE`, before anything is allocated. A version passed to `isRfc` that
names no UUID version (outside 0 to 15, or not a whole number) answers `false`.

### When no native addon fits

`require('hyperuuid')` tries the platform package for the host first. When none fits (the
install ran with `--omit=optional`, or the platform is not one of the eight) it loads the
WebAssembly fallback instead: the same core compiled to `wasm32-wasip1`, run in-process
through Node's `node:wasi` by [`@napi-rs/wasm-runtime`](https://www.npmjs.com/package/@napi-rs/wasm-runtime),
from `@skunkwerkx/hyperuuid-wasm32-wasip1`, which the root depends on outright. The API is the
same, the suite is the same, and `isAvailable()` is `true`. What differs:

- Node prints, once, at `require`:

  ```text
  (node:NNN) ExperimentalWarning: WASI is an experimental feature and might change at any time
  ```

  That is Node's own warning about `node:wasi`, not this package's;
  `node --disable-warning=ExperimentalWarning` silences it.
- It is slower: about 1.5 µs for a single id against 0.2 µs natively, and the batch doors keep
  their edge ([Benchmarks](#benchmarks)). The install also carries the runtime's three dependencies
  (`@napi-rs/wasm-runtime`, `@emnapi/core`, `@emnapi/runtime`) and what they pull in
  (`@tybys/wasm-util`, `@emnapi/wasi-threads`, `tslib`): six packages, about 3.1 MB on disk
  beside the 174 KB module.
- Each module instance has its own v7 stream. A `worker_threads` worker loads its own instance,
  so its ids are strictly ordered among themselves and against nothing minted on the main
  thread, where the native addon shares one process-wide stream.
- Panics abort. `wasm32-wasip1` has no unwinder, so the fallback is built with `panic = "abort"`
  and `catch_unwind` has nothing to catch: a panic surfaces as `RuntimeError: unreachable`
  thrown from the call, after Rust's panic message on stderr. The core is proven to have no
  panics in its own code, and nothing more is claimed: the instance's state after one is not
  claimed sound.
- Bun 1.4.4 cannot run it: its `node:wasi` has no reactor `initialize`, so Bun with no native
  addon has `isAvailable()` `false`. Bun with the addon passes the whole suite.

`NAPI_RS_FORCE_WASI=error` in the environment loads the fallback even where an addon is
installed and fails rather than falling back further; that is how CI runs the whole suite
against it on Linux x64 (glibc), macOS arm64 and Windows x64, and how to try it locally. The
consumer smoke runs from the published tarballs on Alpine, at the exact floor, Node 22.13.0,
and on the floating Node 22 and 24 images; the Node floor is enforced (`engines`), not
advisory.

**Big-endian hosts** (s390x, tested in `node:24-alpine` for `linux/s390x` under QEMU) are a
loss, printed here: the fallback loads, and every check of the consumer smoke passes (v4, v7,
the clock, a v5 vector and a SQL-order vector), but the process never exits on its own. The
WebAssembly runtime (`@emnapi/runtime` 2.0.0-beta.2 through `@napi-rs/wasm-runtime` 1.2.6)
writes the module's threadsafe-function reference count with a little-endian `DataView` and
reads it back through a host-endian `Uint32Array`, so on a big-endian host the count reads as
16,777,216, is never seen to reach zero, and the `MessagePort` the runtime holds to keep the
event loop alive while work is pending is never released. The consumer tests record that the
process has to be killed rather than claiming the platform, and fail the day it changes. A
long-running process is not affected by the exit; the same byte-order mix is used elsewhere in
the runtime, so nothing more is claimed for big-endian hosts than the smoke's checks.

**Bundlers.** esbuild and webpack cannot fold the loader's platform `require`s into one file:
webpack fails the build on them, and an esbuild bundle cannot load the module from a directory
with no `node_modules`. The form that works, for both, is `hyperuuid` external (esbuild
`--external:hyperuuid`, webpack `externals: { hyperuuid: 'commonjs hyperuuid' }`) with
`hyperuuid` installed beside the bundle.

**Browsers.** The fallback package's `browser` entry exports the same doors over a fetched
module (`import('@skunkwerkx/hyperuuid-wasm32-wasip1')` under a bundler that honours the
`browser` condition). It is tested in headless Chrome on Linux, from a Vite build served without
cross-origin isolation (no `Cross-Origin-Opener-Policy` or `Cross-Origin-Embedder-Policy`
headers). Firefox and Safari are not tested.

When the fallback is missing too (a lockfile that pinned neither package, say, or a
`node_modules` pruned by hand), `require` still succeeds, `isAvailable()` is `false`, the plain
values (`Namespaces`, `NIL`, `MAX`, `MAX_V7_BATCH`) are still there, `Layout` and `Variant` are
still there as frozen literals, and every door throws `ERR_HYPERUUID_UNAVAILABLE` with napi's
loader error as its `cause`, whose own `cause` chain names every attempt, native and WebAssembly:

```js
const hyperuuid = require('hyperuuid')

console.log(hyperuuid.isAvailable())
try {
  hyperuuid.newV7()
} catch (error) {
  console.log(error.code, error.cause.message)
}
```

With both packages removed, that prints:

```text
false
ERR_HYPERUUID_UNAVAILABLE Cannot find native binding. npm has a bug related to optional dependencies (https://github.com/npm/cli/issues/4828). Please try `npm i` again after removing both package-lock.json and node_modules directory.
```

### SQL Server order

`v7ToSqlOrder(id)`/`v7FromSqlOrder(id)` convert a version 7 UUID to and from the byte order
SQL Server's `uniqueidentifier` needs to sort by creation order, computed in the Rust core
rather than reimplemented here. The converted value is an RFC 9562 version 8, the version for
custom layouts, with the timestamp and counter placed where SQL Server's comparator reads
first, and the conversion back is exact:

```js
const { newV5, newV7, v7ToSqlOrder, v7FromSqlOrder, Namespaces } = require('hyperuuid')

const id = newV7(1645557742000)
const stored = v7ToSqlOrder(id)
console.log(v7FromSqlOrder(stored) === id) // true
try {
  v7ToSqlOrder(newV5(Namespaces.dns, 'www.example.com'))
} catch (error) {
  // ERR_HYPERUUID_WRONG_VERSION the value is not a UUID of the version this conversion takes
  console.log(error.code, error.message)
}
```

Each conversion takes only a version 7 in its own order: `v7ToSqlOrder` an RFC 9562 v7,
`v7FromSqlOrder` a SQL-ordered value converted from one. The root README's
[SQL Server ordering](https://github.com/SkunkWerkx/HyperUuid#sql-server-ordering) has the
layout and why drivers need the bytes rather than the text form.

### Version 7 batches and buffers

A v7 batch (`newV7Batch`, `newV7BatchBytes`, `fillV7`) is always in strictly increasing
order, including across a millisecond whose steps run out: the ids from the split on carry the
next millisecond. One batch holds at most `MAX_V7_BATCH` (67,108,864) ids. Every v7 door shares
one process-wide stream that may run up to 1000 ms ahead of the timestamp you supply; a call
that would take it further, or past its last millisecond (2^44 - 2), throws
`ERR_HYPERUUID_COUNTER_EXHAUSTED` and reserves nothing. It is a state error, not an argument
error, and a call whose timestamp is past the stream's millisecond succeeds. A timestamp more
than 11 s behind the stream is minted at itself, outside it, and is ordered against nothing.
The root README's
[v7 ordering, precisely](https://github.com/SkunkWerkx/HyperUuid#v7-ordering-precisely) has
the whole contract.

When bytes are the destination (a database parameter, a wire format), skip the strings:

```js
const { fillV7, newV7BatchBytes } = require('hyperuuid')

const buf = new Uint8Array(1000 * 16)
fillV7(buf) // 1000 v7 ids, one timestamp capture, no allocation
const first = buf.subarray(0, 16) // ready for a uuid / uniqueidentifier parameter
console.log(first.length, first[6] >> 4) // 16 7

const bytes = newV7BatchBytes(1000) // the same, into a fresh Uint8Array
console.log(bytes.length) // 16000
```

`fillV7`'s buffer length must be a multiple of 16 (`ERR_HYPERUUID_INVALID_BUFFER`), and an
empty one writes nothing. It writes the caller's memory in place; a `Uint8Array` over a
`SharedArrayBuffer` that another worker writes at the same time is a race the caller made.

## Inspecting a UUID

```js
const { newV7, version, variant, isRfc, getTimestamp, v7ToSqlOrder, Layout, Variant, NIL, MAX } =
  require('hyperuuid')

const id = newV7()
console.log(version(id)) // 7
console.log(variant(id) === Variant.Rfc9562) // true
console.log(isRfc(id, 7)) // true
console.log(version(NIL), version(MAX)) // 0 15
console.log(isRfc(id, 99)) // false, never an error

const stored = v7ToSqlOrder(id)
console.log(version(stored, Layout.SqlServer)) // 7
console.log(isRfc(stored, 7, Layout.SqlServer)) // true
console.log(isRfc(stored, 7)) // false: RFC order reads the wrong bytes
console.log(getTimestamp(stored, Layout.SqlServer)) // the embedded Date, read in place

// 32 hex digits, hyphenated, any case, or exactly 16 bytes
const hex = id.replaceAll('-', '').toUpperCase()
console.log(version(hex), version(Buffer.from(hex, 'hex'))) // 7 7
```

`version`, `variant` and `isRfc` are answered by the native core, the same functions every
other binding calls. `version(id)` is the version nibble whatever the variant, so Nil reads as
0 and Max as 15. `variant(id)` is a `Variant` (`Ncs`, Nil included; `Rfc9562`; `Microsoft`;
`Future`, Max included), read in RFC 9562 order only.

`Layout` says which byte order an id is held in: `Layout.Rfc9562`, the default everywhere, or
`Layout.SqlServer`, the order `v7ToSqlOrder` returns and a `uniqueidentifier` column holds.
`version`, `isRfc`, `getTimestamp`, `v7Timestamp` and `v7UnixMillis` take one as their last
argument and read a SQL-ordered value in place. The bytes alone cannot say which layout a
value is in, so the caller keeps track of it and passes it wherever a SQL-ordered value can
appear; there is deliberately no layout-agnostic check. `Layout` and `Variant` are plain
numeric objects (`Layout.Rfc9562 === 1`, `Layout.SqlServer === 2`; `Variant.Ncs === 1` through
`Variant.Future === 4`), not TypeScript `const enum`s, so they work under type stripping.

`getTimestamp(id)` checks the value and returns `null` for anything that is not an RFC 9562
version 7; `v7Timestamp` and `v7UnixMillis` read the 48 timestamp bits unchecked, whatever
the version. `v7UnixMillis` is the cheaper one, with no `Date` built.

## Bun

The same package runs under Bun on the native addon: `bun test tests/` runs this package's
whole suite (the same 60 tests `node --test` runs), and the [Benchmarks](#benchmarks) below
were taken on both. The WebAssembly fallback does not load on Bun 1.4.4
([above](#when-no-native-addon-fits)).

## Benchmarks

Apple M6 (Mac18,5), macOS, Node 26.13.0 / Bun 1.4.4, measured 2026-10-30, with
[mitata](https://github.com/evanwashere/mitata) (`npm run bench` and `npm run bench:bun`; see
`bench/bench.ts`). These numbers are not comparable to the [linux-x64 tables](../docs/benchmarks.md), which were
measured on linux-x64 on an Intel Core i9-11900H. A second run the same night landed within 4%
of every figure below except Node's `newV7BatchBytes(1000)` (3.07 µs). Bun's `uuid` `v7()` is
the noisiest baseline here (41 ns to 178 µs per call within one run), so read its ratio with
that spread. Averages per call:

| Call | Node 26 | Bun 1.4 | vs. |
| --- | ---: | ---: | --- |
| `newV4()` | 239 ns | 258 ns | `crypto.randomUUID()`: 58.0 ns / 40.5 ns: **4.1x / 6.4x slower** |
| `newV7()` | 155 ns | 166 ns | `uuid`'s `v7()`: 350 ns / 135 ns: 2.3x faster / **1.2x slower** |
| `newV7Batch(1000)` | 44.2 µs | 27.3 µs | 1000x `newV7()`: 155 µs / 169 µs: 3.5x / 6.2x faster |
| `newV7BatchBytes(1000)` | 3.62 µs | 2.93 µs | `newV7Batch(1000)`: 12x / 9.3x faster |
| `v7UnixMillis(id)` | 115 ns | 156 ns | |

`newV4()` loses on both runtimes and `newV7()` on Bun. A call into the addon that touches no
randomness (`v7UnixMillis`) is 115 ns on Node; most of the rest is the random source. On Apple
platforms the core asks `CCRandomGenerateBytes` for every id (the user-space ChaCha20 pool in
`rust/src/entropy.rs` is Linux only), and Apple's generator costs more from 12
bytes up: a v4 draws 16, a v7 11. `crypto.randomUUID()` draws from a buffer of random bytes it
refills in bulk, so it pays that cost once per many ids. A batch does the same: one fetch for
the whole batch, about 44 ns an id as strings on Node and 3.6 ns as bytes. If you mint ids one
at a time and only need v4, `crypto.randomUUID()` is the faster call.

The WebAssembly fallback, measured 2026-10-31 on the same machine (`NAPI_RS_FORCE_WASI=error
npm run bench`, Node 26.13.0, rustc 1.99.1, `@napi-rs/wasm-runtime` 1.2.6), with the native
addon's figures from the same session beside it, since the night before's differ by a few
per cent:

| Call | Node 26, native | Node 26, WebAssembly | Bun 1.4, WebAssembly |
| --- | ---: | ---: | ---: |
| `newV4()` | 250 ns | 1.50 µs (`crypto.randomUUID()` 67.2 ns: **22x slower**) | unavailable |
| `newV7()` | 171 ns | 1.52 µs (`uuid`'s `v7()` 378 ns: **4.0x slower**) | unavailable |
| `newV7Batch(1000)` | 46.3 µs | 611 µs (1000x `newV7()` 1.53 ms: 2.5x faster) | unavailable |
| `newV7BatchBytes(1000)` | 3.39 µs | 40.4 µs | unavailable |
| `v7UnixMillis(id)` | 113 ns | 890 ns | unavailable |

Every call pays the WASI boundary (about 0.9 µs where the native addon pays 0.1), so the
fallback is 6x to 13x behind the addon and loses to both JavaScript baselines on single ids;
batches still pay it once. Bun 1.4.4 cannot initialise the module (above), so its column is
unavailable rather than a number. The module is 174,278 bytes (the `wasi` profile: release with LTO,
stripped, `panic = "abort"`), and its runtime (`@napi-rs/wasm-runtime`, `@emnapi/core`,
`@emnapi/runtime` and the three packages they pull in) adds about 3.1 MB to `node_modules`.

String ids are built as latin1 JavaScript strings straight from 36 bytes on the stack, not
returned as a Rust `String`, which V8 would decode as UTF-8. The choice was made on a
measurement against the `String` form during development; the harness in the repo measures
only the shipped form, so the table above is the receipt.

## Local loop

From `typescript/`, with a Rust toolchain installed:

```sh
npm ci
npm run build       # the release addon for this machine, plus binding.js and binding.d.ts
npm test            # node --test
npm run test:bun    # bun test
npm run typecheck
npm run lint
npm run bench
```

The WebAssembly fallback needs the `wasm32-wasip1` target (`rustup target add wasm32-wasip1`):

```sh
npm run build:wasi                  # hyperuuid.wasm32-wasip1.wasm and the hyperuuid.wasip1* loaders
NAPI_RS_FORCE_WASI=error npm test   # the same suite on the fallback, failing rather than falling back
NAPI_RS_FORCE_WASI=error npm run bench
```

Pass `--js binding.js --dts binding.d.ts` and `--platform` to any other `napi build` (a
`--target` cross build included), as both build scripts do: without the first two napi writes
its generated loader to `index.js` and `index.d.ts`, over this package's own, and without
`--platform` the WASI build deletes `binding.js` as a stale managed file.

What a consumer gets from the published tarballs is proven by installing them:
[`consumer-tests/`](consumer-tests/README.md) packs the ten packages, serves them from a local
registry and runs the install scenarios (plain, `--omit=optional`, no native for the host,
forced fallback, deleted packages, a Mac-made lockfile on Linux, the exact Node floor, the
big-endian loss on s390x under emulation, esbuild and webpack) against them.

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
