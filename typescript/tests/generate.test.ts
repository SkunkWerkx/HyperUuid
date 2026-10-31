const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { test }: typeof import('node:test') = require('node:test')
const api = require('../index.js') as typeof import('../index.js')
const {
  newV4,
  newV4Bytes,
  newV7,
  newV7Bytes,
  newV7Batch,
  newV7BatchBytes,
  fillV7,
  MAX_V7_BATCH,
  v7UnixMillis,
  getTimestamp,
  isRfc,
} = api

const MS = 1645557742000
const PREFIX = '017f22e279b0'
const hex12 = (id: string): string => id.replace('-', '').slice(0, 12)

test('newV4 is a v4 string and unique', () => {
  assert.match(newV4(), /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/)
  assert.equal(new Set(Array.from({ length: 1000 }, () => newV4())).size, 1000)
})

test('newV4Bytes is 16 bytes with v4 version and variant', () => {
  const b = newV4Bytes()
  assert.ok(b instanceof Uint8Array)
  assert.equal(b.length, 16)
  assert.equal(b[6] >> 4, 4)
  assert.equal(b[8] & 0xc0, 0x80)
})

test('newV7(number) carries the version and the millisecond', () => {
  const id = newV7(MS)
  assert.equal(id[14], '7')
  assert.equal(v7UnixMillis(id), MS)
})

test('newV7 accepts a Date and floors fractions', () => {
  assert.equal(hex12(newV7(new Date(MS))), PREFIX)
  assert.equal(hex12(newV7(MS + 0.7)), PREFIX)
})

test('newV7() embeds the current time', () => {
  assert.ok(Math.abs(parseInt(hex12(newV7()), 16) - Date.now()) < 1000)
})

test('newV7Batch is unique and ascending', () => {
  const ids = newV7Batch(1000)
  assert.equal(ids.length, 1000)
  assert.equal(new Set(ids).size, 1000)
  assert.deepEqual([...ids].sort(), ids)
})

test('newV7Batch(0) is empty', () => {
  assert.deepEqual(newV7Batch(0), [])
})

test('newV7BatchBytes is count * 16 bytes of v7', () => {
  const b = newV7BatchBytes(3)
  assert.equal(b.length, 48)
  for (let i = 0; i < 3; i++) assert.equal(b[i * 16 + 6] >> 4, 7)
})

test('newV7Bytes is 16 bytes of v7', () => {
  const b = newV7Bytes(MS)
  assert.equal(b.length, 16)
  assert.equal(b[6] >> 4, 7)
})

test('fillV7 writes whole ids into a Buffer', () => {
  const buf = Buffer.alloc(32)
  fillV7(buf, MS)
  assert.equal(buf.subarray(0, 16).toString('hex').slice(0, 12), PREFIX)
  assert.equal(buf.subarray(16).toString('hex').slice(0, 12), PREFIX)
})

test('fillV7 writes a Uint8Array in place', () => {
  const arr = new Uint8Array(32)
  fillV7(arr)
  assert.ok(arr.some((x) => x !== 0))
})

test('timestamp errors carry ERR_HYPERUUID_TIMESTAMP_RANGE', () => {
  const code = 'ERR_HYPERUUID_TIMESTAMP_RANGE'
  assert.throws(() => newV7(-1), { code })
  assert.throws(() => newV7(NaN), { code })
  assert.throws(() => newV7(253_402_300_800_000), { code })
  assert.throws(() => newV7(new Date(NaN)), { code })
})

test('v7 timestamps stop at the last millisecond of 9999 in both directions', () => {
  // The cap is the last instant .NET's DateTime and Python's datetime can name. One
  // millisecond more is ERR_HYPERUUID_TIMESTAMP_RANGE; a v7 another generator dated past the
  // cap has no timestamp at getTimestamp, as a v6 has none, while v7UnixMillis trusts the
  // caller and reads the 48-bit field as it is.
  const last = new Date('9999-12-31T23:59:59.999Z')
  assert.equal(last.getTime(), 253_402_300_799_999)
  assert.equal(getTimestamp(newV7(last))?.getTime(), 253_402_300_799_999)
  assert.equal(getTimestamp(newV7(253_402_300_799_999))?.getTime(), 253_402_300_799_999)
  for (const past of [253_402_300_800_000, new Date('+010000-01-01T00:00:00Z')]) {
    assert.throws(() => newV7(past), {
      code: 'ERR_HYPERUUID_TIMESTAMP_RANGE',
      message:
        'unix_millis must be non-negative and at most 253402300799999 (9999-12-31T23:59:59.999Z)',
    })
  }

  const past = 'ffffffff-ffff-7000-8000-010203040506'
  assert.equal(isRfc(past, 7), true)
  assert.equal(getTimestamp(past), null)
  assert.equal(v7UnixMillis(past), 281_474_976_710_655)
})

test('oversize batches carry ERR_HYPERUUID_BATCH_TOO_LARGE', () => {
  const code = 'ERR_HYPERUUID_BATCH_TOO_LARGE'
  assert.throws(() => newV7Batch(MAX_V7_BATCH + 1), { code })
  assert.throws(() => newV7Batch(2 ** 32), { code })
  assert.throws(() => newV7Batch(1e300), { code })
  let big: Uint8Array | undefined
  try {
    big = new Uint8Array(16 * (MAX_V7_BATCH + 1))
  } catch {
    /* allocation refused */
  }
  if (big) assert.throws(() => fillV7(big), { code })
})

test('a ragged buffer carries ERR_HYPERUUID_INVALID_BUFFER', () => {
  assert.throws(() => fillV7(new Uint8Array(17)), { code: 'ERR_HYPERUUID_INVALID_BUFFER' })
})

test('bad counts carry ERR_HYPERUUID_INVALID_COUNT', () => {
  const code = 'ERR_HYPERUUID_INVALID_COUNT'
  assert.throws(() => newV7Batch(1.5), { code })
  assert.throws(() => newV7Batch(-1), { code })
  assert.throws(() => newV7Batch(NaN), { code })
  assert.throws(() => newV7BatchBytes(1.5), { code })
})

test('MAX_V7_BATCH is the 26-bit counter space', () => {
  assert.equal(MAX_V7_BATCH, 67108864)
})

test('fillV7 writes only inside an offset view', () => {
  const buf = new Uint8Array(64).fill(0xaa)
  fillV7(new Uint8Array(buf.buffer, 16, 32))
  assert.ok(buf.subarray(0, 16).every((b) => b === 0xaa))
  assert.ok(buf.subarray(48, 64).every((b) => b === 0xaa))
  for (const at of [16, 32]) {
    assert.equal(buf[at + 6] >> 4, 7)
  }
})

test('fillV7 on an empty view does nothing', () => {
  assert.doesNotThrow(() => fillV7(new Uint8Array(0)))
})

// Under WASI the loader starts the guest with napi.wasm.initialMemory pages (992, 65,011,712
// bytes), so an 80 MB result can only exist after memory.grow; the stream must survive it.
test('an 80 MB batch is whole and the stream survives it', () => {
  const bytes = newV7BatchBytes(5_000_000)
  assert.equal(bytes.length, 80_000_000)
  for (const at of [0, 79_999_984]) {
    assert.equal(bytes[at + 6] >> 4, 7)
    assert.equal(bytes[at + 8] & 0xc0, 0x80)
  }
  const last = Buffer.from(bytes.subarray(79_999_984)).toString('hex')
  assert.ok(Buffer.from(newV7Bytes()).toString('hex') > last)
})

// 160,000 random bytes cross the core's 64 KiB getrandom chunk on WASI (rust/src/entropy.rs).
test('a fill past the WASI getrandom chunk is whole', () => {
  const buf = new Uint8Array(16 * 10_000)
  fillV7(buf)
  let previous = ''
  for (let i = 0; i < buf.length; i += 16) {
    const id = Buffer.from(buf.subarray(i, i + 16)).toString('hex')
    assert.equal(id[12], '7')
    assert.equal(buf[i + 8] & 0xc0, 0x80)
    assert.ok(id > previous)
    previous = id
  }
})
