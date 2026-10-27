const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { existsSync, readFileSync } = require('node:fs') as typeof import('node:fs')
const path: typeof import('node:path') = require('node:path')
const { test }: typeof import('node:test') = require('node:test')
const api = require('../index.js') as typeof import('../index.js')
const {
  Layout,
  Variant,
  Namespaces,
  newV5,
  v7ToSqlOrder,
  v7FromSqlOrder,
  version,
  variant,
  isRfc,
  getTimestamp,
  v7UnixMillis,
  v7Timestamp,
} = api

type LayoutCode = (typeof Layout)[keyof typeof Layout]
type VariantCode = (typeof Variant)[keyof typeof Variant]
// One corpus record. Each file uses a subset of these fields; JSON.parse does not check them.
interface Vector {
  namespace: string
  name_hex: string
  name: string
  expect: string
  rfc: string
  sql: string
  direction: string
  uuid: string
  layout: string
  version: number
  unix_millis: number | null
  is_rfc: boolean
  variant: string
}

function corpusDirectory(): string {
  const start = __dirname
  let dir = start
  for (;;) {
    const candidate = path.join(dir, 'corpus')
    if (existsSync(candidate)) return candidate
    const parent = path.dirname(dir)
    if (parent === dir) throw new Error(`corpus directory not found above ${start}`)
    dir = parent
  }
}
const CORPUS = corpusDirectory()
const load = (name: string): Vector[] => JSON.parse(readFileSync(path.join(CORPUS, name), 'utf8'))
const hex = (id: string): string => id.replace(/-/g, '')
const bytes = (digits: string): Uint8Array => Uint8Array.from(Buffer.from(digits, 'hex'))
const layouts: Record<string, LayoutCode> = {
  rfc9562: Layout.Rfc9562,
  sql_server: Layout.SqlServer,
}
const variants: Record<string, VariantCode> = {
  ncs: Variant.Ncs,
  rfc9562: Variant.Rfc9562,
  microsoft: Variant.Microsoft,
  future: Variant.Future,
}
const files = [
  'v5.json',
  'sql_order.json',
  'sql_order_refused.json',
  'timestamp.json',
  'inspect.json',
]
const message = 'the value is not a UUID of the version this conversion takes'

test('every corpus file exists', () => {
  for (const file of files) assert.ok(existsSync(path.join(CORPUS, file)), file)
})

test('v5.json', () => {
  for (const v of load('v5.json')) {
    const namespace = (Namespaces as Record<string, string>)[v.namespace]
    assert.equal(hex(newV5(namespace, bytes(v.name_hex))), v.expect, JSON.stringify(v))
    if ('name' in v) assert.equal(hex(newV5(namespace, v.name)), v.expect, JSON.stringify(v))
  }
})

test('sql_order.json', () => {
  for (const v of load('sql_order.json')) {
    const ctx = JSON.stringify(v)
    assert.equal(hex(v7ToSqlOrder(v.rfc)), v.sql, ctx)
    assert.equal(hex(v7FromSqlOrder(v.sql)), v.rfc, ctx)
    assert.equal(hex(v7ToSqlOrder(bytes(v.rfc))), v.sql, ctx)
    assert.equal(hex(v7FromSqlOrder(bytes(v.sql))), v.rfc, ctx)
  }
})

test('sql_order_refused.json', () => {
  const expected = { code: 'ERR_HYPERUUID_WRONG_VERSION', message }
  for (const v of load('sql_order_refused.json')) {
    const convert = v.direction === 'to_sql' ? v7ToSqlOrder : v7FromSqlOrder
    assert.throws(() => convert(v.uuid), expected, JSON.stringify(v))
  }
})

test('timestamp.json', () => {
  for (const v of load('timestamp.json')) {
    const ctx = JSON.stringify(v)
    const L = layouts[v.layout]
    const rfc = v.layout === 'rfc9562'
    assert.equal(version(v.uuid, L), v.version, ctx)
    if (v.unix_millis === null) {
      assert.equal(getTimestamp(v.uuid, L), null, ctx)
      if (rfc) assert.equal(getTimestamp(v.uuid), null, ctx)
      continue
    }
    assert.equal(v.version, 7, ctx)
    assert.equal(v7UnixMillis(v.uuid, L), v.unix_millis, ctx)
    assert.equal(getTimestamp(v.uuid, L)?.getTime(), v.unix_millis, ctx)
    assert.equal(v7Timestamp(v.uuid, L).getTime(), v.unix_millis, ctx)
    if (rfc) {
      assert.equal(v7UnixMillis(v.uuid), v.unix_millis, ctx)
      assert.equal(getTimestamp(v.uuid)?.getTime(), v.unix_millis, ctx)
      assert.equal(v7Timestamp(v.uuid).getTime(), v.unix_millis, ctx)
    }
  }
})

test('inspect.json', () => {
  for (const v of load('inspect.json')) {
    const ctx = JSON.stringify(v)
    const L = layouts[v.layout]
    assert.equal(version(v.uuid, L), v.version, ctx)
    assert.equal(isRfc(v.uuid, v.version, L), v.is_rfc, ctx)
    for (let other = 0; other < 16; other++) {
      if (other !== v.version) assert.equal(isRfc(v.uuid, other, L), false, `${ctx} other=${other}`)
    }
    if (!('variant' in v)) continue
    assert.equal(variant(v.uuid), variants[v.variant], ctx)
    assert.equal(version(v.uuid), v.version, ctx)
    assert.equal(isRfc(v.uuid, v.version), v.is_rfc, ctx)
  }
})
