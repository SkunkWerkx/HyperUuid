// The acceptance check for the browser page: the serialized DOM Chrome dumps must carry
// `PASS` as the last non-empty line of `<pre id="out">` and no `FAIL` line anywhere in it.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'
import { check } from './check-dom.mjs'

const page = (body) =>
  `<!DOCTYPE html><html><head></head><body><pre id="out">${body}</pre><script type="module" src="./main.js"></script></body></html>`

test('a passing DOM is accepted', () => {
  const result = check(page('v5.json: 12 vectors\nsql_order.json: 8 vectors\nPASS\n'))
  assert.equal(result.ok, true)
  assert.deepEqual(result.lines, ['v5.json: 12 vectors', 'sql_order.json: 8 vectors', 'PASS'])
})

test('a FAIL line is rejected with its reason', () => {
  const result = check(
    page('v5.json: 12 vectors\nFAIL: sql_order.json: expected &quot;a&quot; &lt; &quot;b&quot;\n'),
  )
  assert.equal(result.ok, false)
  assert.equal(result.lines[1], 'FAIL: sql_order.json: expected "a" < "b"')
  assert.match(result.reason, /FAIL/)
})

test('PASS followed by further output is rejected', () => {
  const result = check(page('PASS\nsomething after\n'))
  assert.equal(result.ok, false)
  assert.match(result.reason, /last/)
})

test('a DOM without #out is rejected', () => {
  const result = check('<!DOCTYPE html><html><body><p>nothing</p></body></html>')
  assert.equal(result.ok, false)
  assert.match(result.reason, /#out/)
})

test('the command exits 0 on PASS and 1 otherwise', () => {
  const dir = mkdtempSync(join(tmpdir(), 'check-dom-'))
  const script = fileURLToPath(new URL('./check-dom.mjs', import.meta.url))
  const run = (name, html) => {
    const file = join(dir, name)
    writeFileSync(file, html)
    return spawnSync(process.execPath, [script, file], { encoding: 'utf8' })
  }
  assert.equal(run('pass.html', page('PASS\n')).status, 0)
  const failed = run('fail.html', page('FAIL: boom\n'))
  assert.equal(failed.status, 1)
  assert.match(failed.stdout + failed.stderr, /FAIL: boom/)
  assert.equal(run('trailing.html', page('PASS\nmore\n')).status, 1)
  assert.equal(run('missing.html', '<html></html>').status, 1)
  assert.equal(spawnSync(process.execPath, [script], { encoding: 'utf8' }).status, 2)
})
