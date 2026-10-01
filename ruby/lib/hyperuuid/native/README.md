# native/

Populated per-RID with the platform's native `libhyperuuid` build (`native/{rid}/{lib}`) by CI
and by `release.yml` when it packs the gems — see `../../../.gitignore` — plus
`wasm32-wasip1/hyperuuid.wasm`, the same core as a WebAssembly module for the `wasmtime`
backend (`../wasm_runtime.rb`), staged the same way. The RIDs are the ones
`../native_platform.rb` maps `RUBY_PLATFORM` to: `linux-x64`, `linux-arm64`,
`linux-musl-x64`, `linux-musl-arm64`, `osx-x64`, `osx-arm64`, `win-x64`, `win-arm64`.

Nothing has to be staged here for local development: when a file is absent, the Fiddle
backend falls back to `rust/target/release/` and the wasm backend to
`rust/target/wasm32-wasip1/release/` (see `../runtime.rb` and `../wasm_runtime.rb`).

This file exists so the directory has at least one tracked file on a fresh checkout, matching
the Go/Swift bindings' `native/`/`NativeLibs/` placeholder convention. It is not part of the
gem: the gemspec packages `native/*/*`, the staged binaries only.
