# native/

Populated per-RID with the platform's native `libhyperuuid` build (`native/{rid}/{lib}`) by CI
and by `cargo build --release` for local dev — see `../../../.gitignore` — plus
`wasm32-wasip1/hyperuuid.wasm`, the same core as a WebAssembly module for the `wasmtime`
backend (`../wasm_runtime.rb`), staged the same way from
`cargo build --release --target wasm32-wasip1` run inside `rust/`. This file exists so
the directory has at least one tracked file on a fresh checkout, matching the Go/Swift
bindings' `native/`/`NativeLibs/` placeholder convention.
