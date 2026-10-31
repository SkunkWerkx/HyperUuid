// What a consumer sees: require('hyperuuid'), ask whether anything loaded, and if so use it.
// Prints one line, `available=<bool> target=<native|wasm32-wasip1|none> [version=<v>]`, which
// run.sh and container/consumer.sh assert on. When nothing loaded, the first door must throw
// ERR_HYPERUUID_UNAVAILABLE and its cause chain is printed one per line, so the negative
// controls can see that both the native attempts and the WASI one were made. binding.js is
// consulted for the target only after isAvailable() says true: index.js is what tolerates a
// failing binding.js, and this script never requires it when nothing loaded.
'use strict'
const fs = require('node:fs')
const path = require('node:path')
const api = require('hyperuuid')

function fail(message) {
  process.stdout.write(`FAIL: ${message}\n`)
  process.exit(1)
}

if (!api.isAvailable()) {
  process.stdout.write('available=false target=none\n')
  let thrown
  try {
    api.newV4()
  } catch (error) {
    thrown = error
  }
  if (!thrown) fail('newV4() did not throw with nothing loaded')
  if (thrown.code !== 'ERR_HYPERUUID_UNAVAILABLE')
    fail(`newV4() threw ${thrown.code}, not ERR_HYPERUUID_UNAVAILABLE`)
  for (let cause = thrown.cause; cause; cause = cause.cause) {
    process.stdout.write(`cause: ${cause.constructor.name}: ${cause.message.split('\n')[0]}\n`)
  }
  process.exit(0)
}

const binding = require(path.join(path.dirname(require.resolve('hyperuuid')), 'binding.js'))
process.stdout.write(
  `available=true target=${binding.__napiBindingTarget} version=${api.nativeVersion()}\n`,
)

const v4 = api.newV4()
if (!api.isRfc(v4, 4)) fail(`newV4() gave ${v4}`)
const v7 = api.newV7()
if (!api.isRfc(v7, 7)) fail(`newV7() gave ${v7}`)
if (Math.abs(api.v7UnixMillis(v7) - Date.now()) > 1000) fail(`v7UnixMillis(${v7}) is far from now`)

const corpus = process.env.HYPERUUID_CORPUS
if (!corpus) fail('HYPERUUID_CORPUS is not set')
const load = (name) => JSON.parse(fs.readFileSync(path.join(corpus, name), 'utf8'))
const hex = (id) => id.replace(/-/g, '')
const v5 = load('v5.json')[0]
const got = hex(api.newV5(api.Namespaces[v5.namespace], Buffer.from(v5.name_hex, 'hex')))
if (got !== v5.expect) fail(`v5 vector: expected ${v5.expect}, got ${got}`)
const order = load('sql_order.json')[0]
if (hex(api.v7ToSqlOrder(order.rfc)) !== order.sql) fail(`sql_order vector: to_sql of ${order.rfc}`)
if (hex(api.v7FromSqlOrder(order.sql)) !== order.rfc)
  fail(`sql_order vector: from_sql of ${order.sql}`)
process.stdout.write('checks: v4 v7 clock v5-vector sql-order-vector OK\n')
