const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { test }: typeof import('node:test') = require('node:test')
const { spawnSync }: typeof import('node:child_process') = require('node:child_process')
const path: typeof import('node:path') = require('node:path')
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

// Driving the real stream to its ceiling poisons the process-wide v7 state, so the script runs
// in a process of its own: the same runtime as this one (Node or Bun) loading the same package.
const EXHAUSTION_SCRIPT = `
const assert = require('node:assert/strict')
const api = require(process.env.HYPERUUID_INDEX)
const M = 17592186044414 // 2^44 - 2, the last millisecond the stream holds
const MESSAGE =
  'version 7 counter exhausted: the stream cannot advance without running more than ' +
  '1000 ms ahead of the supplied timestamp or past its last millisecond'

const exhausted = (call) => {
  try {
    call()
  } catch (error) {
    assert.equal(api.isHyperUuidError(error), true, String(error))
    assert.equal(error.code, 'ERR_HYPERUUID_COUNTER_EXHAUSTED')
    assert.equal(error.message, MESSAGE)
    return true
  }
  return false
}

// 1. One single v7 at M succeeds.
api.newV7(M)

// 2. Batches of 65,536 at M until the first error: the batch site.
let batchSite = false
for (let i = 0; i < 1025 && !batchSite; i++) batchSite = exhausted(() => api.newV7Batch(65536, M))
assert.ok(batchSite, 'no batch exhausted the stream')

// 3. Singles at M until the first error: the single site.
let singleSite = false
for (let i = 0; i < 1024 && !singleSite; i++) singleSite = exhausted(() => api.newV7(M))
assert.ok(singleSite, 'no single exhausted the stream')

// 4. Every other v7 door now fails at once.
assert.ok(exhausted(() => api.newV7(M)), 'newV7')
assert.ok(exhausted(() => api.newV7Bytes(M)), 'newV7Bytes')
assert.ok(exhausted(() => api.newV7Batch(1, M)), 'newV7Batch')
assert.ok(exhausted(() => api.newV7BatchBytes(1, M)), 'newV7BatchBytes')
assert.ok(exhausted(() => api.fillV7(new Uint8Array(16), M)), 'fillV7')
`

test('v7 counter exhaustion surfaces as ERR_HYPERUUID_COUNTER_EXHAUSTED at every v7 door', () => {
  const result = spawnSync(process.execPath, ['-e', EXHAUSTION_SCRIPT], {
    env: { ...process.env, HYPERUUID_INDEX: path.resolve(__dirname, '..', 'index.js') },
    encoding: 'utf8',
  })
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`)
})
