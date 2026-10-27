import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as all from '../index.js'
import { isAvailable, Layout, Namespaces, newV4, newV7 } from '../index.js'

test('named imports work from ESM', () => {
  assert.equal(typeof newV7, 'function')
  assert.equal(typeof newV4, 'function')
  assert.equal(isAvailable(), true)
  assert.equal(typeof Layout, 'object')
  assert.equal(Namespaces.dns, '6ba7b810-9dad-11d1-80b4-00c04fd430c8')
})

test('the namespace exposes exactly the public names', () => {
  const keys = Object.keys(all)
    .filter((k) => k !== 'default' && k !== 'module.exports')
    .sort()
  const expected = [
    'newV4',
    'newV4Bytes',
    'newV7',
    'newV7Bytes',
    'newV7Batch',
    'newV7BatchBytes',
    'fillV7',
    'version',
    'variant',
    'isRfc',
    'getTimestamp',
    'v7UnixMillis',
    'v7Timestamp',
    'newV5',
    'v7ToSqlOrder',
    'v7FromSqlOrder',
    'nativeVersion',
    'isAvailable',
    'isHyperUuidError',
    'Layout',
    'Variant',
    'Namespaces',
    'NIL',
    'MAX',
    'MAX_V7_BATCH',
  ].sort()
  assert.deepEqual(keys, expected)
})
