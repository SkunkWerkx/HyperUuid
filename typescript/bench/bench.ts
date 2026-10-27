const { bench, do_not_optimize, group, run, summary }: typeof import('mitata') = require('mitata')
const { v7 }: typeof import('uuid') = require('uuid')
const api = require('../index.js') as typeof import('../index.js')
const { newV4, newV7, newV7Batch, newV7BatchBytes, v7UnixMillis } = api

const id = newV7()

group('v4', () => {
  summary(() => {
    bench('hyperuuid newV4()', () => do_not_optimize(newV4()))
    bench('crypto.randomUUID()', () => do_not_optimize(crypto.randomUUID()))
  })
})

group('v7', () => {
  summary(() => {
    bench('hyperuuid newV7()', () => do_not_optimize(newV7()))
    bench("uuid's v7()", () => do_not_optimize(v7()))
  })
})

group('1000 v7 ids', () => {
  summary(() => {
    bench('newV7Batch(1000)', () => do_not_optimize(newV7Batch(1000)))
    bench('1000x newV7()', () => {
      const out: string[] = []
      for (let i = 0; i < 1000; i++) out.push(newV7())
      do_not_optimize(out)
    })
    bench('newV7BatchBytes(1000)', () => do_not_optimize(newV7BatchBytes(1000)))
  })
})

group('read', () => {
  bench('v7UnixMillis(id)', () => do_not_optimize(v7UnixMillis(id)))
})

run()
