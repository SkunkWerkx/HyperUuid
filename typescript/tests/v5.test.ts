const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { test }: typeof import('node:test') = require('node:test')
const api = require('../index.js') as typeof import('../index.js')
const { newV5, Namespaces, version } = api

const RFC_A4 = '2ed6657d-e927-568b-95e1-2665a8aea6a2'

test('newV5 matches RFC 9562 A.4', () => {
  assert.equal(newV5(Namespaces.dns, 'www.example.com'), RFC_A4)
})

test('a byte name is used as given', () => {
  assert.equal(newV5(Namespaces.dns, new TextEncoder().encode('www.example.com')), RFC_A4)
})

test('the namespace may be 16 bytes', () => {
  const ns = Uint8Array.from(Buffer.from(Namespaces.dns.replace(/-/g, ''), 'hex'))
  assert.equal(newV5(ns, 'www.example.com'), RFC_A4)
})

test('Namespaces is frozen and holds the RFC 9562 constants', () => {
  assert.equal(Object.isFrozen(Namespaces), true)
  assert.deepEqual(
    { ...Namespaces },
    {
      dns: '6ba7b810-9dad-11d1-80b4-00c04fd430c8',
      url: '6ba7b811-9dad-11d1-80b4-00c04fd430c8',
      oid: '6ba7b812-9dad-11d1-80b4-00c04fd430c8',
      x500: '6ba7b814-9dad-11d1-80b4-00c04fd430c8',
    },
  )
})

test('a bad namespace carries ERR_HYPERUUID_INVALID_ID', () => {
  assert.throws(() => newV5('nope', 'x'), { code: 'ERR_HYPERUUID_INVALID_ID' })
})

test('a non-UTF-8 name as bytes still yields a v5', () => {
  assert.equal(version(newV5(Namespaces.dns, Uint8Array.of(0xff, 0xfe))), 5)
})
