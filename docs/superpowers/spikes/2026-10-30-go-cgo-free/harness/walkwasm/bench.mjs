// The raw wasm32-wasip1 module's batch, timed in V8: no translation, no Go. Instantiates the
// module with random_get served by crypto.getRandomValues (the core chunks its draws at
// 64 KiB, under getRandomValues' limit), grows one page of scratch past the core's memory,
// mints 1,000 warm-up batches of 1,000 v7s and then times 10,000 more, and prints the
// nanoseconds per batch. The timestamp is re-read per batch so the stream never has to roll
// more than 1000 ms ahead of it. walk-ab.sh runs this on the base and the tree modules,
// alternating, under WALK_WASM=1.
//
//   node bench.mjs <module.wasm> [batches, default 10000]
import { readFileSync } from "node:fs";
import { webcrypto } from "node:crypto";

const [file, countArg] = process.argv.slice(2);
if (!file) {
  console.error("usage: node bench.mjs <module.wasm> [batches]");
  process.exit(2);
}
const batches = Number(countArg ?? 10000);
const warmups = 1000;
const perBatch = 1000;

let memory;
const { instance } = await WebAssembly.instantiate(readFileSync(file), {
  wasi_snapshot_preview1: {
    random_get(ptr, n) {
      webcrypto.getRandomValues(new Uint8Array(memory.buffer, ptr, n));
      return 0;
    },
  },
});
memory = instance.exports.memory;
const batch = instance.exports.uuid_new_v7_batch;
const scratch = memory.grow(1) * 65536; // one page: 16 * 1000 bytes fit

function mint() {
  const rc = batch(BigInt(Date.now()), perBatch, scratch);
  if (rc !== 0) throw new Error(`uuid_new_v7_batch returned ${rc}`);
}
for (let i = 0; i < warmups; i++) mint();
const start = process.hrtime.bigint();
for (let i = 0; i < batches; i++) mint();
const elapsed = process.hrtime.bigint() - start;
console.log(`${(Number(elapsed) / batches).toFixed(0)} ns/batch (${batches} batches of ${perBatch}, node ${process.versions.node}, v8 ${process.versions.v8})`);
