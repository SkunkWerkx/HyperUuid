// The one acceptance check for the browser smoke page, locally and in CI: given the DOM
// Chrome serialized with `--dump-dom`, read the text of `<pre id="out">` and accept it only
// when the last non-empty line is exactly `PASS` and no line starts with `FAIL`. The page
// writes `PASS` last and stops at its first `FAIL: <reason>`, so a `PASS` with output after
// it is not a pass either: something wrote past the verdict.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const ENTITIES = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: ' ' }

function decode(text) {
  return text.replace(/&(#x[0-9a-fA-F]+|#\d+|[a-zA-Z]+);/g, (whole, name) => {
    if (name[0] === '#') {
      const code =
        name[1] === 'x' || name[1] === 'X' ? parseInt(name.slice(2), 16) : Number(name.slice(1))
      return String.fromCodePoint(code)
    }
    return name in ENTITIES ? ENTITIES[name] : whole
  })
}

/** @returns {{ ok: boolean, lines: string[], reason: string | null }} */
export function check(html) {
  const match = /<pre\s+id="out"[^>]*>([\s\S]*?)<\/pre>/i.exec(html)
  if (!match) return { ok: false, lines: [], reason: 'no #out (<pre id="out">) in the DOM' }
  const lines = decode(match[1])
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line !== '')
  const failed = lines.find((line) => line.startsWith('FAIL'))
  if (failed) return { ok: false, lines, reason: failed }
  if (lines.length === 0)
    return { ok: false, lines, reason: '#out is empty: the page wrote nothing' }
  const last = lines[lines.length - 1]
  if (last !== 'PASS') {
    return { ok: false, lines, reason: `the last line is ${JSON.stringify(last)}, not PASS` }
  }
  return { ok: true, lines, reason: null }
}

function main(argv) {
  if (argv.length !== 1) {
    process.stderr.write('usage: node check-dom.mjs <dom.html>\n')
    return 2
  }
  const result = check(readFileSync(argv[0], 'utf8'))
  if (result.ok) {
    process.stdout.write(`${result.lines.join('\n')}\n`)
    return 0
  }
  process.stdout.write(`${result.lines.join('\n')}\n`)
  process.stderr.write(`check-dom: ${result.reason}\n`)
  return 1
}

if (process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1])) {
  process.exitCode = main(process.argv.slice(2))
}
