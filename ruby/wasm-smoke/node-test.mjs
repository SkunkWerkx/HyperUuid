// node node-test.mjs <ruby.wasm>: instantiates the interpreter under Node's WASI and evaluates
// test.rb in it. A failed check raises in Ruby, which @ruby/wasm-wasi rethrows here, so the
// process exits non-zero.
import fs from "node:fs/promises";
import { WASI } from "node:wasi";
import { RubyVM } from "@ruby/wasm-wasi";

const module = await WebAssembly.compile(await fs.readFile(process.argv[2]));
const wasi = new WASI({ version: "preview1", args: ["ruby.wasm"], env: {}, preopens: {} });
const { vm } = await RubyVM.instantiateModule({ module, wasip1: wasi });
const src = await fs.readFile(new URL("./test.rb", import.meta.url), "utf8");
console.log(vm.eval(src).toString());
console.log("DONE");
