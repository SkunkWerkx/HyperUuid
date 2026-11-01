// The public-API half of smoke.cjs, for the bundler scenario: a self-contained bundle is
// judged on what it does, not on which file it loaded, so binding.js is never inspected. The
// scenario runs it under NAPI_RS_FORCE_WASI=error, under which the loader never consults the
// native addon, so a pass here is a pass on the WASI module.
'use strict'
const fs = require('node:fs')
const path = require('node:path')
const api = require('@skunkwerkx/hyperuuid')

function fail(message) {
  process.stdout.write(`FAIL: ${message}\n`)
  process.exit(1)
}

if (!api.isAvailable()) fail('isAvailable() is false')
process.stdout.write(`available=true version=${api.nativeVersion()}\n`)
const v4 = api.newV4()
if (!api.isRfc(v4, 4)) fail(`newV4() gave ${v4}`)
const v7 = api.newV7()
if (!api.isRfc(v7, 7)) fail(`newV7() gave ${v7}`)
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
process.stdout.write('checks: v4 v7 v5-vector sql-order-vector OK\n')
