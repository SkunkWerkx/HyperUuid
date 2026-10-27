const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { readFileSync } = require('node:fs')
const path = require('node:path')
const { test } = require('node:test')
const { isAvailable, nativeVersion } = require('../index.js') as typeof import('../index.js')

test('the addon loads', () => {
  assert.equal(isAvailable(), true)
})

test('nativeVersion matches the core crate', () => {
  const manifest = readFileSync(path.resolve(__dirname, '../../rust/Cargo.toml'), 'utf8')
  const expected = /^version = "([^"]+)"/m.exec(manifest)?.[1]
  assert.equal(nativeVersion(), expected)
})

test('package.json carries the same version', () => {
  const pkg = JSON.parse(readFileSync(path.resolve(__dirname, '../package.json'), 'utf8'))
  assert.equal(pkg.version, nativeVersion())
})
