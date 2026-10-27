let native = null
let loadError = null
try {
  native = require('./binding.js')
} catch (error) {
  loadError = error
}
const unavailable = (name) => () => {
  const error = new Error(`hyperuuid: the native addon did not load, so ${name}() is unavailable`, {
    cause: loadError,
  })
  error.code = 'ERR_HYPERUUID_UNAVAILABLE'
  throw error
}
const door = (name) => (native ? native[name] : unavailable(name))
const nativeVersion = door('nativeVersion')
const newV4 = door('newV4')
const newV4Bytes = door('newV4Bytes')
const newV7 = door('newV7')
const newV7Bytes = door('newV7Bytes')
const newV7Batch = door('newV7Batch')
const newV7BatchBytes = door('newV7BatchBytes')
const fillV7 = door('fillV7')
// Plain values with literal fallbacks, so they exist (and keep their types) when the addon did not load.
const MAX_V7_BATCH = native ? native.MAX_V7_BATCH : 67108864
const version = door('version')
const variant = door('variant')
const isRfc = door('isRfc')
const getTimestamp = door('getTimestamp')
const v7UnixMillis = door('v7UnixMillis')
const v7Timestamp = door('v7Timestamp')
const newV5 = door('newV5')
const v7ToSqlOrder = door('v7ToSqlOrder')
const v7FromSqlOrder = door('v7FromSqlOrder')
// RFC 9562 section 6.6. A plain value, so it exists even when the addon did not load.
const Namespaces = Object.freeze({
  dns: '6ba7b810-9dad-11d1-80b4-00c04fd430c8',
  url: '6ba7b811-9dad-11d1-80b4-00c04fd430c8',
  oid: '6ba7b812-9dad-11d1-80b4-00c04fd430c8',
  x500: '6ba7b814-9dad-11d1-80b4-00c04fd430c8',
})
// Frozen literal fallbacks, so the enums exist when the addon did not load.
const Layout = native ? native.Layout : Object.freeze({ Rfc9562: 1, SqlServer: 2 })
const Variant = native
  ? native.Variant
  : Object.freeze({ Ncs: 1, Rfc9562: 2, Microsoft: 3, Future: 4 })
const NIL = native ? native.NIL : '00000000-0000-0000-0000-000000000000'
const MAX = native ? native.MAX : 'ffffffff-ffff-ffff-ffff-ffffffffffff'
const isAvailable = () => native !== null
const isHyperUuidError = (value) =>
  value instanceof Error &&
  typeof value.code === 'string' &&
  value.code.startsWith('ERR_HYPERUUID_')
module.exports = {
  nativeVersion,
  isAvailable,
  isHyperUuidError,
  MAX_V7_BATCH,
  newV4,
  newV4Bytes,
  newV7,
  newV7Bytes,
  newV7Batch,
  newV7BatchBytes,
  fillV7,
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
  newV5,
  Namespaces,
  v7ToSqlOrder,
  v7FromSqlOrder,
}
