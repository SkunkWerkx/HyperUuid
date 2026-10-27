const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { test }: typeof import('node:test') = require('node:test')
const api = require('../index.js') as typeof import('../index.js')
const { v7ToSqlOrder, v7FromSqlOrder, newV7, NIL } = api

const RFC = '00000000-0000-7000-8000-000000000000'
const SQL_HEX = '00000000000000808000000000000000'
const message = 'the value is not a UUID of the version this conversion takes'

test('v7ToSqlOrder matches the corpus row', () => {
  assert.equal(v7ToSqlOrder(RFC).replace(/-/g, ''), SQL_HEX)
})

test('v7FromSqlOrder is the inverse', () => {
  assert.equal(v7FromSqlOrder(SQL_HEX), RFC)
})

test('a fresh v7 round-trips', () => {
  const id = newV7()
  assert.equal(v7FromSqlOrder(v7ToSqlOrder(id)), id)
})

test('a non-v7 id is ERR_HYPERUUID_WRONG_VERSION', () => {
  const expected = { code: 'ERR_HYPERUUID_WRONG_VERSION', message }
  assert.throws(() => v7ToSqlOrder(NIL), expected)
  assert.throws(() => v7FromSqlOrder(NIL), expected)
})
