# native/

Populated per-RID with the platform's native `libhyperuuid` build (`native/{rid}/{lib}`) by CI
and by `cargo build --release` for local dev — see `../../../.gitignore`. This file exists so
the directory has at least one tracked file on a fresh checkout, matching the Go/Swift
bindings' `native/`/`NativeLibs/` placeholder convention.
