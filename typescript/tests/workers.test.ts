const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const path: typeof import('node:path') = require('node:path')
const { test }: typeof import('node:test') = require('node:test')
const { Worker }: typeof import('node:worker_threads') = require('node:worker_threads')
const { newV7, isRfc } = require('../index.js') as typeof import('../index.js')

// Each worker loads the binding again (under WASI, its own module instance and so its own v7
// stream), so a worker must mint ids on its own while the main thread keeps minting too.
test('a worker thread mints v7 ids of its own', async () => {
  const entry = path.resolve(__dirname, '../index.js')
  const script = `
    const { parentPort } = require('node:worker_threads')
    const { newV7 } = require(${JSON.stringify(entry)})
    parentPort.postMessage(newV7())
  `
  const fromWorker = await new Promise<string>((resolve, reject) => {
    const worker = new Worker(script, { eval: true })
    worker.once('message', resolve)
    worker.once('error', reject)
  })
  assert.equal(typeof fromWorker, 'string')
  assert.ok(isRfc(fromWorker, 7))
  assert.ok(isRfc(newV7(), 7))
})
