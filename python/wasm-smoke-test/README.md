# Python WASM smoke test

Proves the Rust core runs correctly inside a real [Pyodide](https://pyodide.org/) (CPython
compiled to WebAssembly) session, loaded dynamically at runtime via plain `ctypes.CDLL` — the
exact mechanism `hyperuuid`'s own `_runtime.py` already uses natively, completely unchanged.
Not a hopeful description: this is a real `node` process running a real Pyodide interpreter,
not a mock.

This is a third, genuinely different Rust build artifact shape from the other two proven this
project — not the `cdylib` every native binding dlopens, and not the `staticlib` the C# WASM
build links statically. Pyodide's own packaging convention (`meta.yaml`'s `shared_library`
build type) calls for a real Emscripten **side module** — a dynamically loadable unit, built
with Emscripten's own `-sSIDE_MODULE=2` linker flag:

```bash
cd ../../rust
source ~/emsdk/emsdk_env.sh   # emcc on PATH — see the repo root's toolchain docs
RUSTFLAGS="-C link-args=-sSIDE_MODULE=2" cargo build --release --target wasm32-unknown-emscripten
```

Then run the smoke test:

```bash
npm install
node test.mjs
```

Real, measured result on this machine: `v4` (randomness), the RFC 9562 Appendix A.4 `v5`
vector, and a real-clock `v7` (embedded timestamp exactly matched the wall clock, zero drift)
all pass.

**What this doesn't prove yet:** a turnkey `pip install`-and-go experience — this is
proof-of-concept only, matching where the Go/Swift/Ruby/PHP/Java WASM investigations landed
(see the repo root README's WebAssembly section for the full picture across every binding).
Shipping this for real would mean building the Emscripten side module in CI, packaging it as a
Pyodide-compatible artifact, and wiring `_runtime.py` to load it when running under Pyodide
instead of its current native-platform detection — real, additional engineering, deliberately
not done yet.
