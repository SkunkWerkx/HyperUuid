// The corpus, run against the WASI package in a browser. Each check appends a line to
// `#out`; the last line is `PASS`, or the first failure is `FAIL: <reason>` and nothing
// follows it. `check-dom.mjs` reads that verdict from the DOM Chrome dumps.
//
// `tests/corpus.test.ts` is the source of truth for the five corpus checks; they are ported
// here one for one, assertion for assertion, since the browser has neither `node:test` nor
// `node:assert`.
import inspectVectors from '../../corpus/inspect.json'
import sqlOrderVectors from '../../corpus/sql_order.json'
import sqlOrderRefusedVectors from '../../corpus/sql_order_refused.json'
import timestampVectors from '../../corpus/timestamp.json'
import v5Vectors from '../../corpus/v5.json'

const out = document.getElementById('out')
const log = (line) => {
  out.textContent += `${line}\n`
}

function equal(actual, expected, context) {
  if (actual !== expected) {
    throw new Error(
      `${context}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`,
    )
  }
}

function throws(fn, code, message, context) {
  let thrown
  try {
    fn()
  } catch (error) {
    thrown = error
  }
  if (thrown === undefined) throw new Error(`${context}: did not throw`)
  equal(thrown.code, code, `${context}: code`)
  if (message !== undefined) equal(thrown.message, message, `${context}: message`)
}

const hex = (id) => id.replace(/-/g, '')
function bytes(digits) {
  const buf = new Uint8Array(digits.length / 2)
  for (let i = 0; i < buf.length; i++) buf[i] = Number.parseInt(digits.slice(2 * i, 2 * i + 2), 16)
  return buf
}
const WRONG_VERSION = 'the value is not a UUID of the version this conversion takes'
// RFC 9562 section 6.6. The root package's `index.js` defines `Namespaces`; the WASI package
// is the binding alone and has no such export, so a browser consumer brings its own.
const Namespaces = Object.freeze({
  dns: '6ba7b810-9dad-11d1-80b4-00c04fd430c8',
  url: '6ba7b811-9dad-11d1-80b4-00c04fd430c8',
  oid: '6ba7b812-9dad-11d1-80b4-00c04fd430c8',
  x500: '6ba7b814-9dad-11d1-80b4-00c04fd430c8',
})

try {
  // The package's `browser` condition resolves to napi's `hyperuuid.wasip1-browser.js`,
  // which fetches and instantiates the module under top-level await and then exports the
  // bindings as plain named exports: there is no promise to await on the consumer's side.
  // The import is dynamic so that a failure to load lands in `#out` as a FAIL line instead
  // of leaving the page blank.
  const binding = await import('@skunkwerkx/hyperuuid-wasm32-wasip1')
  const {
    Layout,
    Variant,
    newV4,
    newV5,
    newV7,
    newV7Batch,
    fillV7,
    v7ToSqlOrder,
    v7FromSqlOrder,
    version,
    variant,
    isRfc,
    getTimestamp,
    v7UnixMillis,
    v7Timestamp,
  } = binding
  log(`loaded: target=${binding.__napiBindingTarget}`)

  const layouts = { rfc9562: Layout.Rfc9562, sql_server: Layout.SqlServer }
  const variants = {
    ncs: Variant.Ncs,
    rfc9562: Variant.Rfc9562,
    microsoft: Variant.Microsoft,
    future: Variant.Future,
  }

  for (const v of v5Vectors) {
    const namespace = Namespaces[v.namespace]
    equal(hex(newV5(namespace, bytes(v.name_hex))), v.expect, JSON.stringify(v))
    if ('name' in v) equal(hex(newV5(namespace, v.name)), v.expect, JSON.stringify(v))
  }
  log(`v5.json: ${v5Vectors.length} vectors`)

  for (const v of sqlOrderVectors) {
    const ctx = JSON.stringify(v)
    equal(hex(v7ToSqlOrder(v.rfc)), v.sql, ctx)
    equal(hex(v7FromSqlOrder(v.sql)), v.rfc, ctx)
    equal(hex(v7ToSqlOrder(bytes(v.rfc))), v.sql, ctx)
    equal(hex(v7FromSqlOrder(bytes(v.sql))), v.rfc, ctx)
  }
  log(`sql_order.json: ${sqlOrderVectors.length} vectors`)

  for (const v of sqlOrderRefusedVectors) {
    const convert = v.direction === 'to_sql' ? v7ToSqlOrder : v7FromSqlOrder
    throws(() => convert(v.uuid), 'ERR_HYPERUUID_WRONG_VERSION', WRONG_VERSION, JSON.stringify(v))
  }
  log(`sql_order_refused.json: ${sqlOrderRefusedVectors.length} vectors`)

  for (const v of timestampVectors) {
    const ctx = JSON.stringify(v)
    const L = layouts[v.layout]
    const rfc = v.layout === 'rfc9562'
    equal(version(v.uuid, L), v.version, ctx)
    // getTimestamp answers only for a timestamp the core would mint: a version 7 dated past
    // 253402300799999 ms (9999-12-31T23:59:59.999Z) has none, as a version 6 has none, while
    // v7UnixMillis and v7Timestamp read the 48-bit field as it is.
    const minted = v.unix_millis !== null && v.unix_millis <= 253_402_300_799_999
    equal(getTimestamp(v.uuid, L)?.getTime() ?? null, minted ? v.unix_millis : null, ctx)
    if (rfc) equal(getTimestamp(v.uuid)?.getTime() ?? null, minted ? v.unix_millis : null, ctx)
    if (v.unix_millis === null) continue
    equal(v.version, 7, ctx)
    equal(v7UnixMillis(v.uuid, L), v.unix_millis, ctx)
    equal(v7Timestamp(v.uuid, L).getTime(), v.unix_millis, ctx)
    if (rfc) {
      equal(v7UnixMillis(v.uuid), v.unix_millis, ctx)
      equal(v7Timestamp(v.uuid).getTime(), v.unix_millis, ctx)
    }
  }
  log(`timestamp.json: ${timestampVectors.length} vectors`)

  for (const v of inspectVectors) {
    const ctx = JSON.stringify(v)
    const L = layouts[v.layout]
    equal(version(v.uuid, L), v.version, ctx)
    equal(isRfc(v.uuid, v.version, L), v.is_rfc, ctx)
    for (let other = 0; other < 16; other++) {
      if (other !== v.version) equal(isRfc(v.uuid, other, L), false, `${ctx} other=${other}`)
    }
    if (!('variant' in v)) continue
    equal(variant(v.uuid), variants[v.variant], ctx)
    equal(version(v.uuid), v.version, ctx)
    equal(isRfc(v.uuid, v.version), v.is_rfc, ctx)
  }
  log(`inspect.json: ${inspectVectors.length} vectors`)

  // The generation doors: the random source and the v7 clock in this runtime.
  equal(isRfc(newV4(), 4), true, 'newV4')
  const v7 = newV7()
  equal(isRfc(v7, 7), true, 'newV7')
  const batch = newV7Batch(10)
  equal(batch.length, 10, 'newV7Batch(10).length')
  for (let i = 0; i < batch.length; i++) {
    equal(isRfc(batch[i], 7), true, `newV7Batch[${i}]`)
    if (i > 0 && !(batch[i - 1] < batch[i])) {
      throw new Error(`newV7Batch: ${batch[i - 1]} is not before ${batch[i]}`)
    }
  }
  const filled = new Uint8Array(160)
  fillV7(filled)
  for (let i = 0; i < filled.length; i += 16) {
    equal(filled[i + 6] >> 4, 7, `fillV7 slot ${i / 16} version`)
    equal(filled[i + 8] & 0xc0, 0x80, `fillV7 slot ${i / 16} variant`)
  }
  const skew = Math.abs(v7UnixMillis(v7) - Date.now())
  if (!(skew < 1000)) throw new Error(`v7UnixMillis(newV7()) is ${skew} ms from Date.now()`)
  throws(() => version('x'), 'ERR_HYPERUUID_INVALID_ID', undefined, "version('x')")
  log('generation: newV4, newV7, newV7Batch(10), fillV7(160 bytes), clock, invalid id')

  log('PASS')
} catch (error) {
  log(`FAIL: ${error?.stack ?? error}`)
}
