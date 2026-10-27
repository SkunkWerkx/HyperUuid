const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { test }: typeof import('node:test') = require('node:test')
const api = require('../index.js') as typeof import('../index.js')
const native = require('../binding.js') as typeof import('../binding.js')
const { isHyperUuidError, newV7, NIL, MAX, MAX_V7_BATCH } = api

test('isHyperUuidError recognises a binding error', () => {
  let caught: unknown
  try {
    newV7(-1)
  } catch (error) {
    caught = error
  }
  assert.equal(isHyperUuidError(caught), true)
})

test('isHyperUuidError refuses everything else', () => {
  assert.equal(isHyperUuidError(new Error('x')), false)
  assert.equal(isHyperUuidError(null), false)
  assert.equal(isHyperUuidError({ code: 'ERR_HYPERUUID_INVALID_ID' }), false)
  assert.equal(isHyperUuidError(Object.assign(new Error('x'), { code: 'ERR_OTHER' })), false)
  assert.equal(isHyperUuidError(Object.assign(new Error('x'), { code: 42 })), false)
})

test('the fallback literals equal the native values', () => {
  assert.equal(NIL, native.NIL)
  assert.equal(MAX, native.MAX)
  assert.equal(MAX_V7_BATCH, native.MAX_V7_BATCH)
  assert.deepEqual({ ...api.Layout }, { Rfc9562: 1, SqlServer: 2 })
  assert.deepEqual({ ...api.Variant }, { Ncs: 1, Rfc9562: 2, Microsoft: 3, Future: 4 })
})
