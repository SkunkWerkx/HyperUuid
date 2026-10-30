# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![npm](https://img.shields.io/npm/v/hyperuuid.svg)](https://www.npmjs.com/package/hyperuuid)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

RFC 9562 UUID v4 (random), v5 (deterministic) and v7 (time-sortable) generation, with a
version 8 SQL Server order for v7. A Node-API addon built with [napi-rs](https://napi.rs): the
Rust core every other HyperUuid binding calls, linked straight into the `.node` file, with
types for TypeScript and the same API under Node and Bun.

It is not the fastest way to mint a single id in JavaScript: `crypto.randomUUID()` and the
`uuid` package beat it per call on this machine ([Benchmarks](#benchmarks)). What it gives
you is the same engine, bit for bit, as the C#, Go, Java, PHP, Python, Ruby and Swift
bindings, the SQL Server order, and batch and buffer generation that pay for randomness once.

## Install

```sh
npm install hyperuuid
```

The package itself is JavaScript and types; the addon comes from a platform package that npm
picks for you through `optionalDependencies`, so nothing compiles at install. One is
published for each of eight platforms, as `@skunkwerkx/hyperuuid-<platform>`:

| | x64 | arm64 |
| --- | :---: | :---: |
| Linux, glibc (`linux-*-gnu`) | ✓ | ✓ |
| Linux, musl (`linux-*-musl`, Alpine) | ✓ | ✓ |
| macOS (`darwin-*`) | ✓ | ✓ |
| Windows (`win32-*-msvc`) | ✓ | ✓ |

Node 22 or newer (Node-API 5), or Bun. There is no WebAssembly fallback in this release: a
platform outside the eight, or an install run with `--omit=optional`, has no addon, and the
package says so through `isAvailable()` rather than failing at `require` ([below](#when-the-addon-is-missing)).

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
time v7's 48-bit field cannot hold (negative, past 2^48 - 1, `NaN`, an infinity, an invalid
`Date`) is `ERR_HYPERUUID_TIMESTAMP_RANGE`.

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
  // ERR_HYPERUUID_TIMESTAMP_RANGE unix_millis must be non-negative and fit within 48 bits
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
caught. Every door catches a Rust panic and throws it as napi's `GenericFailure` rather than
taking the process down; the core is built to have none, so that is a backstop.

Counts are checked, not coerced: `newV7Batch(1.5)`, `-1`, `NaN` and `Infinity` are
`ERR_HYPERUUID_INVALID_COUNT`, and anything past `MAX_V7_BATCH` is
`ERR_HYPERUUID_BATCH_TOO_LARGE`, before anything is allocated. A version passed to `isRfc` that
names no UUID version (outside 0 to 15, or not a whole number) answers `false`.

### When the addon is missing

`require('hyperuuid')` always succeeds. If the platform package is absent, `isAvailable()` is
`false`, the plain values (`Namespaces`, `NIL`, `MAX`, `MAX_V7_BATCH`) are still there, `Layout`
and `Variant` are still there as frozen literals, and every door throws `ERR_HYPERUUID_UNAVAILABLE` with napi's
loader error as its `cause`:

```js
const hyperuuid = require('hyperuuid')

console.log(hyperuuid.isAvailable())
try {
  hyperuuid.newV7()
} catch (error) {
  console.log(error.code, error.cause.code)
}
```

With the addon removed, that prints:

```text
false
ERR_HYPERUUID_UNAVAILABLE undefined
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

The same package runs under Bun: `bun test tests/` runs this package's whole suite (the same
57 tests `node --test` runs), and the [Benchmarks](#benchmarks) below were taken on both.

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
randomness (`v7UnixMillis`) is 115 ns on Node; the rest is the random source. On Apple
platforms the core asks `CCRandomGenerateBytes` for every id (the user-space ChaCha20 path in
`rust/src/entropy.rs` is Linux and Android only), and Apple's generator costs more from 12
bytes up: a v4 draws 16, a v7 11. `crypto.randomUUID()` draws from a buffer of random bytes it
refills in bulk, so it pays that cost once per many ids. A batch does the same: one fetch for
the whole batch, about 44 ns an id as strings on Node and 3.6 ns as bytes. If you mint ids one
at a time and only need v4, `crypto.randomUUID()` is the faster call.

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

Pass `--js binding.js --dts binding.d.ts` to any other `napi build --platform` (a
`--target` cross build included), as `npm run build` does: without them napi writes its
generated loader to `index.js` and `index.d.ts`, over this package's own.

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
