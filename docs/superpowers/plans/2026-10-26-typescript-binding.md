# TypeScript Binding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `typescript/` napi-rs addon that exposes the HyperUuid core to Node ≥ 22 and Bun with the same persisted-bytes contract as every other binding, proven locally on this Mac.

**Architecture:** A separate `publish = false` cdylib crate in `typescript/` depends on `../rust` by path and exposes `#[napi(catch_unwind)]` functions named in the Python spelling. `@napi-rs/cli` builds the `.node` and generates `binding.js`/`binding.d.ts`; a hand-written `index.js`/`index.d.ts` wraps the loader so a missing addon is reported, not fatal. Tests run on `node --test` and `bun test` from one tree.

**Tech Stack:** Rust 1.99.1 (repo toolchain), `napi` 3.15 / `napi-derive` 3.6 / `napi-build` 2.6, `@napi-rs/cli` 3.10.9, Node 26 (24 and 22 as floors), Bun 1.4, Biome 2.6.3, mitata 1.0.34, TypeScript 5.x.

**Spec:** `docs/superpowers/specs/2026-10-26-r-and-typescript-bindings-design.md` (section "TypeScript binding"). The spec's "This repo's wiring" and "Forge changes" sections are the release-plumbing plan, not this one.

## Global Constraints

- Owner drives git (`CLAUDE.md`): no task commits, pushes or tags. Each task ends with the work verified in the working tree; the owner commits. Nothing in `rust/` changes.
- Node floor 22 (`engines.node >= 22`); Node-API level 5; targets are exactly the repo's eight RIDs under napi's names `linux-x64-gnu`, `linux-arm64-gnu`, `linux-x64-musl`, `linux-arm64-musl`, `darwin-x64`, `darwin-arm64`, `win32-x64-msvc`, `win32-arm64-msvc`.
- Package names: root `hyperuuid`, platform packages `@skunkwerkx/hyperuuid-<napi name>`. `optionalDependencies` are NOT committed: the release pack step injects them at the exact version (release-plumbing plan), so the committed `package.json` has one `"version"` line and `package-lock.json` is regenerated from it.
- Every `#[napi]` function carries `catch_unwind`; `[profile.release]` has `panic = "unwind"` (explicit, with the reason in a comment), `lto = true`, `codegen-units = 1`, `strip = "symbols"`.
- Error codes (the `code` property): `ERR_HYPERUUID_RANDOM_SOURCE`, `ERR_HYPERUUID_TIMESTAMP_RANGE`, `ERR_HYPERUUID_BATCH_TOO_LARGE`, `ERR_HYPERUUID_WRONG_VERSION`, `ERR_HYPERUUID_INVALID_ID`, `ERR_HYPERUUID_INVALID_BUFFER`, `ERR_HYPERUUID_INVALID_COUNT`, `ERR_HYPERUUID_UNAVAILABLE` (index.js only). Type errors (wrong JS type, unknown enum member) are napi's own `InvalidArg`; a caught panic is `GenericFailure`. Neither is a HyperUuid code.
- Messages are copied from `rust/src/python_ext.rs` where one exists: `"uuid_new_v4: random source failure"`, `"uuid_new_v7: random source failure"`, `"uuid_new_v7_batch: random source failure"`, `"unix_millis must be non-negative and fit within 48 bits"`, `"batch count exceeds the 26-bit counter space (67108864 per call)"`, `"the value is not a UUID of the version this conversion takes"`, `"buffer length must be a multiple of 16 (one whole UUID per 16 bytes)"`. New: `"id must be 32 hex digits, 36 with hyphens, or exactly 16 bytes"`, `"count must be a non-negative whole number"`.
- Counts cross as `f64`, never `u32`: napi's `u32` is JavaScript's ToUint32 (`1.5` → 1, `2**32` → 0, `NaN` → 0), so the binding validates finite, non-negative, integral and `<= MAX_V7_BATCH` itself before casting.
- Ids in: `string | Uint8Array` (32 hex digits or 8-4-4-4-12, any case; or exactly 16 bytes). Ids out: lowercase hyphenated. Time in: `number` (Unix milliseconds) or `Date`; absent means now. `MAX_V7_BATCH` is 67108864.
- No `enum`, parameter properties or decorators in test code (type stripping). Test files are `tests/*.test.ts` plus one `tests/esm.test.mts`; run with `node --test tests/` and `bun test tests/`.
- Pinned dev versions in `package.json` are exact (`"3.10.9"`, not `^3`); Dependabot moves them.

## Review Focus

Inputs the spec implies but no task's tests exercise, most likely to bite first:

1. A `Buffer` (Node's `Uint8Array` subclass) as an id or as the `fillV7` target: must behave as a `Uint8Array`. Pinned in Task 3 (`Buffer.from(hex, 'hex')` as id) and Task 2 (`Buffer.alloc(32)` for `fillV7`).
2. `newV7(new Date(NaN))`: an invalid Date must be `ERR_HYPERUUID_TIMESTAMP_RANGE`, not a silent 0. Pinned in Task 2.
3. A fractional millisecond (`newV7(1645557742000.7)`) must floor, matching Python's integer contract. Pinned in Task 2.
4. `newV7Batch` with a negative, fractional, `NaN` or `2**32` count: all must be `ERR_HYPERUUID_INVALID_COUNT`, never a silent 1 or 0. Pinned in Task 2.
5. The ESM named-export surface must equal the CJS one, or `import { x }` silently yields `undefined` for a name the lexer missed. Pinned in Task 6 (the ESM test asserts the full name list).

---

### Task 1: Crate, package and loader skeleton with `nativeVersion`

**Files:**
- Create: `typescript/Cargo.toml`, `typescript/build.rs`, `typescript/.cargo/config.toml`, `typescript/src/lib.rs`, `typescript/package.json`, `typescript/tsconfig.json`, `typescript/index.js`, `typescript/index.d.ts`, `typescript/tests/version.test.ts`, `typescript/.gitignore` (`*.node`, `npm/`, `target/`, `node_modules/`)
- Generated and committed after the first build: `typescript/binding.js`, `typescript/binding.d.ts`, `typescript/Cargo.lock`, `typescript/package-lock.json`

**Interfaces:**
- Produces: `nativeVersion(): string` (`"1.0.0"` today, decoded from `hyperuuid::hyperuuid_version()` as `major.minor.patch`); `isAvailable(): boolean`; the `ErrorCode` enum in `src/lib.rs` (`AsRef<str>` yielding the `ERR_HYPERUUID_*` strings) and `type Result<T> = napi::Result<T, ErrorCode>` that every later task uses; the `index.js` wrapping shape every later task adds names to.

- [ ] **Step 1: Install the toolchain**

Run: `fnm install 26 && fnm install 22 && fnm default 26 && curl -fsSL https://bun.sh/install | bash` (Node 24.22.1 is already installed through fnm). Expected: `node --version` prints v26.x, `bun --version` prints 1.4.x.

- [ ] **Step 2: Write the failing test**

`typescript/tests/version.test.ts`:
```ts
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { isAvailable, nativeVersion } from '../index.js'

test('the addon loads', () => { assert.equal(isAvailable(), true) })

test('nativeVersion matches the core crate', () => {
  const manifest = readFileSync(new URL('../../rust/Cargo.toml', import.meta.url), 'utf8')
  const expected = /^version = "([^"]+)"/m.exec(manifest)?.[1]
  assert.equal(nativeVersion(), expected)
})

test('package.json carries the same version', () => {
  const pkg = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'))
  assert.equal(pkg.version, nativeVersion())
})
```
(`import.meta.url` is unavailable in a CJS `.ts`; if Node reports that, set `"type": "commonjs"` explicitly and use `__dirname` with `path.resolve` instead, in every test file.)

- [ ] **Step 3: Run it to verify it fails**

Run: `cd typescript && node --test tests/`. Expected: FAIL, cannot find `../index.js`.

- [ ] **Step 4: Write the crate**

`typescript/Cargo.toml`: package `hyperuuid-napi`, version `0.0.0`, edition 2024, `publish = false`, `[lib] crate-type = ["cdylib"]`; dependencies `hyperuuid = { path = "../rust" }`, `napi = { version = "3", default-features = false, features = ["napi5"] }`, `napi-derive = "3"`; build-dependency `napi-build = "2"`; the release profile from Global Constraints with a comment on `panic = "unwind"` (catch_unwind needs it; the core is proven no-panic so nothing reaches the unwinder in practice). `typescript/build.rs` calls `napi_build::setup()`. `typescript/.cargo/config.toml` holds only the two musl `target-feature=-crt-static` sections copied from `rust/.cargo/config.toml` lines 28-32, with a one-line comment saying why only those.

`typescript/src/lib.rs`:
```rust
pub enum ErrorCode { RandomSource, TimestampRange, BatchTooLarge, WrongVersion, InvalidId, InvalidBuffer, InvalidCount }
impl AsRef<str> for ErrorCode { /* the ERR_HYPERUUID_* strings */ }
pub type Result<T> = napi::Result<T, ErrorCode>;

#[napi(catch_unwind)]
pub fn native_version() -> String
```

- [ ] **Step 5: Write the package files**

`typescript/package.json`: `name` `hyperuuid`, `version` equal to `rust/Cargo.toml`'s, `main` `index.js`, `types` `index.d.ts`, `files` `["index.js", "index.d.ts", "binding.js", "binding.d.ts", "README.md", "LICENSE"]`, `engines.node >= 22`, `license` MIT, `repository.url` `git+https://github.com/SkunkWerkx/HyperUuid.git` (must match the OIDC repo exactly), `napi` block `{ "binaryName": "hyperuuid", "packageName": "@skunkwerkx/hyperuuid", "constEnum": false, "targets": [the eight triples] }`, scripts `build` (`napi build --release --platform --js binding.js --dts binding.d.ts`), `build:debug` (same without `--release`), `test` (`node --test tests/`), `test:bun` (`bun test tests/`), `typecheck` (`tsc --noEmit`), `lint` (`biome check .`), `format` (`biome format --write .`), `bench` (`node bench/bench.ts`), `bench:bun` (`bun bench/bench.ts`); devDependencies exact: `@napi-rs/cli` 3.10.9, `@biomejs/biome` 2.6.3, `typescript` (current 5.x), `@types/node` (current 26.x or 24.x), `mitata` 1.0.34, `uuid` 14.0.3.

`typescript/tsconfig.json`: `strict`, `module` `nodenext`, `moduleResolution` `nodenext`, `target` `es2022`, `allowImportingTsExtensions` false, `noEmit`, `types ["node"]`, `include ["index.d.ts", "binding.d.ts", "tests", "bench"]`.

`typescript/index.js` (the shape every later task extends):
```js
let native = null
let loadError = null
try { native = require('./binding.js') } catch (error) { loadError = error }
const unavailable = (name) => () => {
  const error = new Error(`hyperuuid: the native addon did not load, so ${name}() is unavailable`, { cause: loadError })
  error.code = 'ERR_HYPERUUID_UNAVAILABLE'
  throw error
}
const door = (name) => (native ? native[name] : unavailable(name))
const nativeVersion = door('nativeVersion')
const isAvailable = () => native !== null
module.exports = { nativeVersion, isAvailable }
```
`typescript/index.d.ts`: `export { nativeVersion } from './binding'` plus `export function isAvailable(): boolean`.

- [ ] **Step 6: Build and run the test**

Run: `cd typescript && npm install && npm run build && node --test tests/`. Expected: PASS ×3; `binding.js`, `binding.d.ts`, `hyperuuid.darwin-arm64.node`, `Cargo.lock`, `package-lock.json` exist. Confirm `binding.d.ts` contains `export declare function nativeVersion(): string`.

- [ ] **Step 7: Negative control for the unavailable path**

Run: `cd typescript && mv hyperuuid.darwin-arm64.node /tmp/hu.node && node -e "const h=require('./index.js'); console.log(h.isAvailable()); try { h.nativeVersion() } catch (e) { console.log(e.code, e.cause?.code) }"; mv /tmp/hu.node hyperuuid.darwin-arm64.node`. Expected: `false` then `ERR_HYPERUUID_UNAVAILABLE MODULE_NOT_FOUND` (or napi's load error code). Record the exact output for the README.

---

### Task 2: Generation: v4, v7, batches, bytes doors, `fillV7`

**Files:**
- Modify: `typescript/src/lib.rs`, `typescript/index.js`, `typescript/index.d.ts`
- Test: `typescript/tests/generate.test.ts`

**Interfaces:**
- Consumes: `ErrorCode`, `Result<T>` from Task 1.
- Produces (binding names): `newV4(): string`, `newV4Bytes(): Uint8Array`, `newV7(time?: number | Date): string`, `newV7Bytes(time?): Uint8Array`, `newV7Batch(count: number, time?): string[]`, `newV7BatchBytes(count: number, time?): Uint8Array` (length `count * 16`), `fillV7(buffer: Uint8Array, time?): void` (writes `buffer.length / 16` ids in place), `MAX_V7_BATCH: number`. Internal Rust helpers every later task reuses: `fn millis(time: Option<Either<f64, JsDate>>) -> Result<u64>` (None → `SystemTime::now()`; NaN, ±∞, negative or `> hyperuuid::v7::MAX_UNIX_MILLIS` → `TimestampRange`; otherwise floor) and `fn format36(uuid: &Uuid) -> String`.

- [ ] **Step 1: Write the failing tests**

`typescript/tests/generate.test.ts` (one `test()` per line):
- `newV4()` matches `/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/`; 1000 calls are unique.
- `newV4Bytes()` is a `Uint8Array` of length 16 with `(b[6] >> 4) === 4` and `(b[8] & 0xc0) === 0x80`.
- `newV7(1645557742000)` has version nibble `7` at index 14 and `v7UnixMillis(id) === 1645557742000` (until Task 3 exports `v7UnixMillis`, assert instead that the first 12 hex digits equal `017f22e279b0`; switch the line over in Task 3).
- `newV7(new Date(1645557742000))` gives the same first 12 hex digits; `newV7(1645557742000.7)` too (floors).
- `newV7()` embeds a timestamp within 1000 ms of `Date.now()` (read the first 12 hex digits as `parseInt(..., 16)`).
- `newV7Batch(1000)` returns 1000 unique strings already in ascending order (`[...ids].sort()` deep-equals `ids`); `newV7Batch(0)` returns `[]`.
- `newV7BatchBytes(3).length === 48` and the three 16-byte slices each have the 7 nibble.
- `fillV7(Buffer.alloc(32), 1645557742000)` leaves both slices starting `017f22e279b0`; `fillV7(new Uint8Array(32))` writes in place (the array changes).
- Errors, each `assert.throws(fn, { code })`: `newV7(-1)`, `newV7(NaN)`, `newV7(2 ** 48)`, `newV7(new Date(NaN))` → `ERR_HYPERUUID_TIMESTAMP_RANGE`; `newV7Batch(MAX_V7_BATCH + 1)` and `fillV7(new Uint8Array(16 * (MAX_V7_BATCH + 1)))` (skip the second if allocation fails) → `ERR_HYPERUUID_BATCH_TOO_LARGE`; `fillV7(new Uint8Array(17))` → `ERR_HYPERUUID_INVALID_BUFFER`; `newV7Batch(1.5)`, `newV7Batch(-1)`, `newV7Batch(NaN)`, `newV7Batch(2 ** 32)`, `newV7BatchBytes(1.5)` → `ERR_HYPERUUID_INVALID_COUNT`.
- `MAX_V7_BATCH === 67108864`.

- [ ] **Step 2: Run them to verify they fail**

Run: `cd typescript && node --test tests/generate.test.ts`. Expected: FAIL, names not exported.

- [ ] **Step 3: Implement in `src/lib.rs`**

Signatures (all `#[napi(catch_unwind)]`, Python spelling so napi camelCases them):
```rust
pub fn new_v4() -> Result<String>
pub fn new_v4_bytes() -> Result<Uint8Array>
pub fn new_v7(time: Option<Either<f64, JsDate>>) -> Result<String>
pub fn new_v7_bytes(time: Option<Either<f64, JsDate>>) -> Result<Uint8Array>
pub fn new_v7_batch(count: f64, time: Option<Either<f64, JsDate>>) -> Result<Vec<String>>
pub fn new_v7_batch_bytes(count: f64, time: Option<Either<f64, JsDate>>) -> Result<Uint8Array>
pub fn fill_v7(mut buffer: Uint8Array, time: Option<Either<f64, JsDate>>) -> Result<()>
#[napi] pub const MAX_V7_BATCH: u32 = hyperuuid::v7::MAX_BATCH;
```
Core calls: `hyperuuid::v4::new_v4()`, `v7::new_v7(millis)`, `v7::new_v7_batch(millis, count, &mut out)`. Batch: `fn count(value: f64) -> Result<u32>` rejects non-finite, negative or fractional values with `InvalidCount` and `> MAX_BATCH` with `BatchTooLarge`, before anything is allocated; map `NewV7Error::{Random → RandomSource, TimestampOutOfRange → TimestampRange, BatchTooLarge → BatchTooLarge, BufferTooSmall → InvalidBuffer}`. `fill_v7`: length not a multiple of 16 → `InvalidBuffer`; `len / 16 > MAX_BATCH` → `BatchTooLarge`; then one `new_v7_batch` into the slice. Strings: format into a stack `[u8; 36]` and return `String` for now (Task 8 measures `create_string_latin1`). If napi 3 rejects `JsDate` in a parameter position, take `Option<Either<f64, Unknown>>` and coerce with `Unknown::coerce_to_number()`; if `Uint8Array` by value does not write through, take `Uint8ArraySlice<'_>`. If `#[napi] pub const` is rejected, add `#[napi(catch_unwind)] pub fn max_v7_batch() -> u32` and define `MAX_V7_BATCH = native.maxV7Batch()` in `index.js`, so the value still comes from the core.

Then add the eight names to `index.js` (`door(...)`) and `index.d.ts`.

- [ ] **Step 4: Build and run**

Run: `cd typescript && npm run build && node --test tests/generate.test.ts`. Expected: PASS for every line.

---

### Task 3: Id parsing and inspection doors

**Files:**
- Modify: `typescript/src/lib.rs`, `typescript/index.js`, `typescript/index.d.ts`
- Test: `typescript/tests/inspect.test.ts`

**Interfaces:**
- Produces: enums `Layout { Rfc9562 = 1, SqlServer = 2 }`, `Variant { Ncs = 1, Rfc9562 = 2, Microsoft = 3, Future = 4 }`; `version(id, layout?: Layout): number`, `variant(id): Variant`, `isRfc(id, version: number, layout?): boolean`, `getTimestamp(id, layout?): Date | null`, `v7UnixMillis(id, layout?): number`, `v7Timestamp(id, layout?): Date` (the unchecked bit read as a Date); consts `NIL`, `MAX`. Internal `fn parse16(id: Either<String, Uint8Array>) -> Result<Uuid>` (hex table, allocation-free, any case, 32 or 36 chars, or 16 bytes; else `InvalidId`) used by every later door.

- [ ] **Step 1: Write the failing tests**

`typescript/tests/inspect.test.ts`, using `const A1 = 'c232ab00-9414-11ec-b3c8-9f6bdeced846'` (RFC A.1, v1), `const V7 = '017f22e2-79b0-7cc3-98c4-dc0c0c07398f'` (RFC A.6):
- `version(V7) === 7`, `version(A1) === 1`, `version(NIL) === 0`, `version(V7, Layout.SqlServer) === 0`.
- Leniency: `version(V7.toUpperCase())`, `version(V7.replace(/-/g, ''))`, `version(Buffer.from(V7.replace(/-/g, ''), 'hex'))`, `version(Uint8Array.from(Buffer.from(..., 'hex')))` all `=== 7`.
- `variant(V7) === Variant.Rfc9562`, `variant(NIL) === Variant.Ncs`, `variant(MAX) === Variant.Future`.
- `isRfc(V7, 7) === true`, `isRfc(V7, 4) === false`, `isRfc(V7, 7, Layout.SqlServer) === false`.
- `getTimestamp(V7)?.getTime() === 1645557742000`; `getTimestamp(A1) === null`; `v7UnixMillis(V7) === 1645557742000`; `v7Timestamp(V7).getTime() === 1645557742000`.
- `NIL === '00000000-0000-0000-0000-000000000000'`, `MAX === 'ffffffff-ffff-ffff-ffff-ffffffffffff'`.
- Errors → `ERR_HYPERUUID_INVALID_ID`: `version('nope')`, `version(V7 + '0')`, `version(new Uint8Array(15))`, `version('g' + V7.slice(1))`. `version(V7, 3 as never)` throws (napi `InvalidArg`; assert only that it throws and that `code !== 'ERR_HYPERUUID_INVALID_ID'`).

- [ ] **Step 2: Run to verify failure**

Run: `cd typescript && node --test tests/inspect.test.ts`. Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
#[napi] pub enum Layout { Rfc9562 = 1, SqlServer = 2 }
#[napi] pub enum Variant { Ncs = 1, Rfc9562 = 2, Microsoft = 3, Future = 4 }
#[napi] pub const NIL: &str = "00000000-0000-0000-0000-000000000000";
#[napi] pub const MAX: &str = "ffffffff-ffff-ffff-ffff-ffffffffffff";
pub fn version(id: Either<String, Uint8Array>, layout: Option<Layout>) -> Result<u8>
pub fn variant(id: ...) -> Result<Variant>
pub fn is_rfc(id: ..., version: u8, layout: Option<Layout>) -> Result<bool>
pub fn get_timestamp(env: Env, id: ..., layout: Option<Layout>) -> Result<Option<JsDate>>
pub fn v7_unix_millis(id: ..., layout: Option<Layout>) -> Result<f64>
pub fn v7_timestamp(env: Env, id: ..., layout: Option<Layout>) -> Result<JsDate>
```
Core calls: `Uuid::version_in`, `Uuid::variant`, `Uuid::is_rfc_in`, `hyperuuid::get_timestamp_in(&uuid, layout).map(|t| t.to_unix_millis())`, `v7::unix_millis_in`. Dates via `env.create_date(ms as f64)`. `layout.unwrap_or(Layout::Rfc9562)` maps to `hyperuuid::Layout`. If `&str` consts are rejected by napi, define `NIL`/`MAX` in `index.js`/`index.d.ts` instead. Add the names to `index.js` and `index.d.ts` (`export { Layout, Variant, ... } from './binding'`).

- [ ] **Step 4: Build and run**

Run: `cd typescript && npm run build && node --test tests/`. Expected: PASS, including the Task 2 `v7UnixMillis` line now un-deferred.

---

### Task 4: v5 and namespaces

**Files:**
- Modify: `typescript/src/lib.rs`, `typescript/index.js`, `typescript/index.d.ts`
- Test: `typescript/tests/v5.test.ts`

**Interfaces:**
- Produces: `newV5(namespace: string | Uint8Array, name: string | Uint8Array): string`; `Namespaces: { readonly dns: string; readonly url: string; readonly oid: string; readonly x500: string }` (frozen object in `index.js`, values the RFC 9562 §6.6 constants `6ba7b810-9dad-11d1-80b4-00c04fd430c8`, `...11-...`, `...12-...`, `...14-...`).

- [ ] **Step 1: Write the failing tests**

`typescript/tests/v5.test.ts`: `newV5(Namespaces.dns, 'www.example.com') === '2ed6657d-e927-568b-95e1-2665a8aea6a2'` (RFC A.4); the same with `new TextEncoder().encode('www.example.com')`; the same with the namespace as 16 bytes; `Object.isFrozen(Namespaces)`; `newV5('nope', 'x')` → `ERR_HYPERUUID_INVALID_ID`; a non-UTF-8 name as bytes (`Uint8Array.of(0xff, 0xfe)`) returns a v5 (version nibble 5).

- [ ] **Step 2: Run to verify failure.** Expected: FAIL.

- [ ] **Step 3: Implement** `pub fn new_v5(namespace: Either<String, Uint8Array>, name: Either<String, Uint8Array>) -> Result<String>` over `hyperuuid::v5::new_v5(ns, name_bytes)`; a string name is its UTF-8 bytes. `Namespaces` lives in `index.js` (`Object.freeze({...})`) and `index.d.ts`.

- [ ] **Step 4: Build and run** `node --test tests/`. Expected: PASS.

---

### Task 5: SQL Server order

**Files:**
- Modify: `typescript/src/lib.rs`, `typescript/index.js`, `typescript/index.d.ts`
- Test: `typescript/tests/sql_order.test.ts`

**Interfaces:**
- Produces: `v7ToSqlOrder(id): string`, `v7FromSqlOrder(id): string`.

- [ ] **Step 1: Write the failing tests**

Using the first `corpus/sql_order.json` row: `v7ToSqlOrder('00000000-0000-7000-8000-000000000000').replace(/-/g, '') === '00000000000000808000000000000000'`, the inverse, a round trip of `newV7()`, and `v7ToSqlOrder(NIL)` → `ERR_HYPERUUID_WRONG_VERSION` with message `the value is not a UUID of the version this conversion takes`; `v7FromSqlOrder(NIL)` the same.

- [ ] **Step 2: Run to verify failure.** Expected: FAIL.

- [ ] **Step 3: Implement** both over `hyperuuid::v7::{to_sql_order, to_rfc_order}`; `SqlOrderError` → `WrongVersion` with the core's Display text.

- [ ] **Step 4: Build and run.** Expected: PASS.

---

### Task 6: Corpus replay, ESM surface, error helpers, typecheck

**Files:**
- Modify: `typescript/index.js`, `typescript/index.d.ts`
- Test: `typescript/tests/corpus.test.ts`, `typescript/tests/esm.test.mts`, `typescript/tests/errors.test.ts`

**Interfaces:**
- Produces: `type HyperUuidErrorCode = 'ERR_HYPERUUID_RANDOM_SOURCE' | ... | 'ERR_HYPERUUID_INVALID_COUNT' | 'ERR_HYPERUUID_UNAVAILABLE'`, `interface HyperUuidError extends Error { code: HyperUuidErrorCode }`, `isHyperUuidError(value: unknown): value is HyperUuidError` (an `Error` whose `code` is a string starting `ERR_HYPERUUID_`).

- [ ] **Step 1: Write the corpus test**

`typescript/tests/corpus.test.ts`: locate the corpus by walking up from the test file's directory until a `corpus/` directory exists; throw `new Error('corpus directory not found above ' + start)` otherwise. Read each file with `JSON.parse`. Helpers: `hex = (id) => id.replace(/-/g, '')`, `bytes = (hex) => Uint8Array.from(Buffer.from(hex, 'hex'))`. One `test()` per corpus file, asserting per vector exactly what `python/tests/test_corpus.py` asserts:
- `v5.json`: `hex(newV5(Namespaces[namespace], bytes(name_hex))) === expect`, and when `name` is present, the same with the string.
- `sql_order.json`: `hex(v7ToSqlOrder(rfc)) === sql`, `hex(v7FromSqlOrder(sql)) === rfc`, and both again through the bytes door.
- `sql_order_refused.json`: the converter named by `direction` throws `{ code: 'ERR_HYPERUUID_WRONG_VERSION', message: 'the value is not a UUID of the version this conversion takes' }`.
- `timestamp.json`: `version(uuid, L) === version`; when `unix_millis` is `null`, `getTimestamp(uuid, L) === null`; else `version === 7`, `v7UnixMillis(uuid, L) === unix_millis`, `getTimestamp(uuid, L).getTime() === unix_millis`, `v7Timestamp(uuid, L).getTime() === unix_millis`; for `rfc9562` rows the no-layout forms too. (No overflow branch: a Date holds 2^48 ms.)
- `inspect.json`: `version(uuid, L) === version`; `isRfc(uuid, version, L) === is_rfc`; `isRfc(uuid, other, L) === false` for every other `other` in 0..15; when `variant` is present, `variant(uuid) === Variant[Name]` with `{ncs: Variant.Ncs, rfc9562: Variant.Rfc9562, microsoft: Variant.Microsoft, future: Variant.Future}`, `version(uuid) === version`, `isRfc(uuid, version) === is_rfc`.
`L` maps `rfc9562 → Layout.Rfc9562`, `sql_server → Layout.SqlServer`. The test enumerates the five file names and fails on a missing one.

- [ ] **Step 2: Write the ESM and error tests**

`typescript/tests/esm.test.mts`: `import { newV7, newV4, Layout, Namespaces, isAvailable } from '../index.js'` and `import * as all from '../index.js'`; assert `typeof newV7 === 'function'`, and `Object.keys(all).filter((k) => k !== 'default' && k !== 'module.exports').sort()` deep-equals the full export list (Node's CJS interop adds `default` and, from 22, `module.exports` to the namespace) (the twenty-five names: the seventeen doors, `isAvailable`, `isHyperUuidError`, `Layout`, `Variant`, `Namespaces`, `NIL`, `MAX`, `MAX_V7_BATCH`). `typescript/tests/errors.test.ts`: `isHyperUuidError(caught from newV7(-1)) === true`, `isHyperUuidError(new Error('x')) === false`, `isHyperUuidError(null) === false`.

- [ ] **Step 3: Run both** (`node --test tests/`). Expected: the ESM test FAILS on the key list until `index.js` exports via one shorthand object literal; the corpus test passes if Tasks 2-5 are right.

- [ ] **Step 4: Implement** `isHyperUuidError` in `index.js`; make the final `module.exports = { ... }` one object literal of shorthand keys (cjs-module-lexer needs that); write `index.d.ts` fully: re-exports from `./binding`, `isAvailable`, `Namespaces`, `HyperUuidErrorCode`, `HyperUuidError`, `isHyperUuidError`.

- [ ] **Step 5: Run everything on Node and Bun, and the typecheck**

Run: `cd typescript && node --test tests/ && bun test tests/ && npm run typecheck`. Expected: all PASS on both runtimes. If Bun's `node:test` shim fails a file, add the ten-line `tests/harness.ts` the spec allows (export `test`/`assert` from `bun:test` when `process.versions.bun`, else from `node:test`) and import it from every test.

- [ ] **Step 6: Corpus negative control**

Run: `cd typescript && python3 - <<'EOF'
import json,pathlib; p=pathlib.Path('../corpus/sql_order.json'); d=json.loads(p.read_text()); d[0]['sql']='ff'+d[0]['sql'][2:]; p.write_text(json.dumps(d, indent=2)+'\n')
EOF
node --test tests/corpus.test.ts; git -C .. checkout corpus/sql_order.json`. Expected: the sql_order test FAILS, then the file is restored (`git status` clean for `corpus/`).

---

### Task 7: Export-check sites and the `catch_unwind` rule

**Files:**
- Modify: `.github/scripts/check_exports.py` (`SITES`, a new `ts_spelling`, a `napi_catch_unwind()` check)
- Test: `.github/scripts/tests/test_check_exports.py` (new)

**Interfaces:**
- Produces: `ts_spelling(export) -> str | None` (VERSION → `native_version`; `uuid_v7_unix_millis_in` → `None`, merged; else `python_spelling`), `TS_EXTRA = ("new_v4_bytes", "new_v7_bytes", "new_v7_batch_bytes", "fill_v7", "v7_timestamp")`, `camel(name) -> str`, two `SITES` entries: `"TypeScript (napi)"` on `typescript/src/lib.rs` with regex `r'(?m)^#\[napi\((?:[^)\n]*)\)\]\s*\npub fn (\w+)\('`, and `"TypeScript (binding.d.ts)"` on `typescript/binding.d.ts` with `r'(?m)^export declare function (\w+)\('` wanting `lambda e: camel(ts_spelling(e))`; `napi_catch_unwind(path)` exits 1 naming every `#[napi(...)]` line directly above a `pub fn` that lacks `catch_unwind`.

- [ ] **Step 1: Write the failing test**

`.github/scripts/tests/test_check_exports.py` (unittest, loading the module with `importlib.util.spec_from_file_location` as `test_abi_probe.py` does): `test_napi_site_requires_catch_unwind` writes a `tmp` file with `#[napi]\npub fn new_v4()` and asserts `napi_catch_unwind(path)` raises `SystemExit`; a second fixture with `#[napi(catch_unwind)]` passes; `test_ts_spelling_merges_the_layout_pair` asserts `ts_spelling('uuid_v7_unix_millis_in') is None` and `ts_spelling('hyperuuid_version') == 'native_version'`; `test_camel` asserts `camel('v7_to_sql_order') == 'v7ToSqlOrder'`.

- [ ] **Step 2: Run** `python3 -m unittest discover -s .github/scripts/tests -v`. Expected: the new tests FAIL (attributes missing).

- [ ] **Step 3: Implement** in `check_exports.py`; call `napi_catch_unwind("typescript/src/lib.rs")` from `main()` before the site comparison.

- [ ] **Step 4: Run** the unit tests and `python3 .github/scripts/check_exports.py`. Expected: PASS and the script reports every site including the two new ones in sync.

- [ ] **Step 5: Negative controls**

Remove `catch_unwind` from one function in `src/lib.rs`, run the script: expected exit 1 naming it; restore. Comment out `export declare function newV5` in `binding.d.ts`, run: expected exit 1 on the `.d.ts` site; restore (`git checkout` is not available for an untracked file, so re-run `npm run build`).

---

### Task 8: Panic probe, latin1 strings, Biome, clippy, bench, README

**Files:**
- Create: `typescript/biome.json`, `typescript/bench/bench.ts`, `typescript/README.md`, `typescript/LICENSE` (copy of the root `LICENSE`)
- Modify: `typescript/src/lib.rs` (string return path), `typescript/package.json` (if a script changes)
- Scratch (not committed): `/private/tmp/claude-501/.../scratchpad/panic-probe/` (a copy of the crate skeleton with two functions)

- [ ] **Step 1: Panic probe pair**

In the scratchpad, a crate with `#[napi(catch_unwind)] pub fn caught() { panic!("probe") }` and `#[napi] pub fn uncaught() { panic!("probe") }`, built with `napi build --platform`. Run: `node -e "const p=require('./index.js'); try { p.caught() } catch (e) { console.log('caught', e.code, e.message) }"` → expected `caught GenericFailure ...probe...`. Then `node -e "require('./index.js').uncaught()"; echo "exit $?"` → expected a non-zero exit with the process terminated (the negative control). Record both outputs verbatim in the PR report.

- [ ] **Step 2: Bench**

`typescript/bench/bench.ts` with mitata: `newV4` vs `crypto.randomUUID()`; `newV7` vs `uuid`'s `v7()`; `newV7Batch(1000)` vs a 1000-iteration `newV7` loop; `newV7BatchBytes(1000)`; `v7UnixMillis`. Run `npm run bench` and `npm run bench:bun`; keep the output.

- [ ] **Step 3: latin1 strings, measured**

Change the string doors to build the 36 bytes on the stack and return through `env.create_string_latin1` (signature in napi 3: `Env::create_string_latin1(&self, &[u8]) -> Result<JsString<'_>>`; the door then takes `env: &Env` and returns `Result<JsString<'_>>`). Rebuild, run `node --test tests/` (PASS), re-run the bench. Keep whichever of `String`/latin1 is faster; the README says which and by how much. If napi rejects the lifetime, keep `String` and say so in the README.

- [ ] **Step 4: Lint and format**

`typescript/biome.json`: Biome 2 schema, `files.includes` `["index.js", "tests/**", "bench/**", "*.json"]`, formatter `indentStyle: space`, `lineWidth: 100`, recommended lints, `javascript.formatter.quoteStyle: single`, `semicolons: asNeeded`. Run `npm run lint`, `cargo fmt --check` and `cargo clippy --release -- -D warnings` in `typescript/`. Expected: clean.

- [ ] **Step 5: README**

`typescript/README.md`: install (`npm install hyperuuid`, with `<version>` never concrete), the API table (every door, its signature, its error codes), the leniency rule for ids, time input, the `isAvailable()` contract and the Task 1 Step 7 output, Bun support, the benchmark table dated and labelled with this Mac's model, losses printed beside wins, and the local loop (`npm ci`, `npm run build`, `npm test`, `npm run test:bun`). Verify every snippet by running it.

- [ ] **Step 6: Floors and the cross-build**

Run the suite under `fnm exec --using 22 node --test tests/` and `fnm exec --using 24 ...`. Expected: PASS on 22 and 24. Run `npx napi build --release --platform --target x86_64-apple-darwin` and `lipo -archs hyperuuid.darwin-x64.node` → `x86_64`; delete that file afterwards (`.gitignore` covers `*.node`).

---

## Verification (whole plan)

- `cd typescript && npm ci && npm run build && npm test && npm run test:bun && npm run typecheck && npm run lint` on Node 26; `npm test` again on 22 and 24.
- `python3 .github/scripts/check_exports.py` passes; the two Task 7 negative controls fail.
- `python3 -m unittest discover -s .github/scripts/tests -v` passes.
- `git status` shows only `typescript/`, `.github/scripts/check_exports.py`, `.github/scripts/tests/test_check_exports.py` and the two spec-driven doc edits; no `*.node`, `npm/` or `target/` is tracked.
- Left for CI (named in the PR report): the musl `.node` NEEDED gate, the osx-x64 Rosetta suite, the win-arm64 Bun question, the Linux glibc floor of a std-linking `.node`.
