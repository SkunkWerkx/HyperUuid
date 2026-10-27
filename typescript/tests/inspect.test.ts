const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { test }: typeof import('node:test') = require('node:test')
const api = require('../index.js') as typeof import('../index.js')
const {
  version,
  variant,
  isRfc,
  getTimestamp,
  v7UnixMillis,
  v7Timestamp,
  Layout,
  Variant,
  NIL,
  MAX,
} = api

const A1 = 'c232ab00-9414-11ec-b3c8-9f6bdeced846'
const V7 = '017f22e2-79b0-7cc3-98c4-dc0c0c07398f'
const MS = 1645557742000
const V7_HEX = V7.replace(/-/g, '')

test('version reads the nibble, honouring the layout', () => {
  assert.equal(version(V7), 7)
  assert.equal(version(A1), 1)
  assert.equal(version(NIL), 0)
  assert.equal(version(V7, Layout.SqlServer), 0)
})

test('ids parse leniently from strings and bytes', () => {
  assert.equal(version(V7.toUpperCase()), 7)
  assert.equal(version(V7_HEX), 7)
  assert.equal(version(Buffer.from(V7_HEX, 'hex')), 7)
  assert.equal(version(Uint8Array.from(Buffer.from(V7_HEX, 'hex'))), 7)
})

test('variant classifies the top bits', () => {
  assert.equal(variant(V7), Variant.Rfc9562)
  assert.equal(variant(NIL), Variant.Ncs)
  assert.equal(variant(MAX), Variant.Future)
})

test('isRfc checks version and layout', () => {
  assert.equal(isRfc(V7, 7), true)
  assert.equal(isRfc(V7, 4), false)
  assert.equal(isRfc(V7, 7, Layout.SqlServer), false)
})

test('timestamps', () => {
  assert.equal(getTimestamp(V7)?.getTime(), MS)
  assert.equal(getTimestamp(A1), null)
  assert.equal(v7UnixMillis(V7), MS)
  assert.equal(v7Timestamp(V7).getTime(), MS)
})

test('NIL and MAX', () => {
  assert.equal(NIL, '00000000-0000-0000-0000-000000000000')
  assert.equal(MAX, 'ffffffff-ffff-ffff-ffff-ffffffffffff')
})

test('bad ids carry ERR_HYPERUUID_INVALID_ID', () => {
  const code = 'ERR_HYPERUUID_INVALID_ID'
  assert.throws(() => version('nope'), { code })
  assert.throws(() => version(`${V7}0`), { code })
  assert.throws(() => version(new Uint8Array(15)), { code })
  assert.throws(() => version(`g${V7.slice(1)}`), { code })
})

test('an unknown layout is napi InvalidArg, not a HyperUuid code', () => {
  assert.throws(() => version(V7, 3 as never), { code: 'InvalidArg' })
})

test('a layout must be exactly 1 or 2', () => {
  const code = 'InvalidArg'
  assert.throws(() => version(V7, 2.5 as never), { code })
  assert.throws(() => isRfc(V7, 7, 1.9 as never), { code })
  assert.throws(() => v7UnixMillis(V7, (2 ** 32 + 2) as never), { code })
  assert.throws(() => getTimestamp(V7, 0 as never), { code })
  assert.throws(() => v7Timestamp(V7, NaN as never), { code })
})

test('a hyphen at the wrong offset is an invalid id', () => {
  assert.throws(() => version('017f22e279-b0-7cc3-98c4-dc0c0c07398f'), {
    code: 'ERR_HYPERUUID_INVALID_ID',
  })
})

test('isRfc answers false for versions that name no UUID version', () => {
  assert.equal(isRfc(V7, 263), false)
  assert.equal(isRfc(V7, -249), false)
  assert.equal(isRfc(V7, 7.5), false)
  assert.equal(isRfc(V7, NaN), false)
})

test('the parser rejects malformed ids', () => {
  const code = 'ERR_HYPERUUID_INVALID_ID'
  assert.throws(() => version(`${V7_HEX}0000`), { code })
  assert.throws(
    () =>
      version(
        V7.replace(/-/g, '')
          .replace(/^(.{7})/, '$1-')
          .slice(0, 36),
      ),
    { code },
  )
  assert.throws(() => version(`é${V7_HEX.slice(2)}`), { code })
  assert.throws(() => version(`é${V7_HEX.slice(1)}`), { code })
  assert.throws(() => version(new Uint8Array(17)), { code })
  assert.throws(() => version(`${V7_HEX.slice(0, 10)}-${V7_HEX.slice(11)}`), { code })
})

test('every inspection door rejects a bad id', () => {
  const code = 'ERR_HYPERUUID_INVALID_ID'
  assert.throws(() => variant('nope'), { code })
  assert.throws(() => isRfc('nope', 7), { code })
  assert.throws(() => getTimestamp('nope'), { code })
  assert.throws(() => v7UnixMillis('nope'), { code })
  assert.throws(() => v7Timestamp('nope'), { code })
})

test('the SqlServer layout path', () => {
  assert.equal(getTimestamp(V7, Layout.SqlServer), null)
  assert.equal(typeof v7UnixMillis(V7, Layout.SqlServer), 'number')
})
