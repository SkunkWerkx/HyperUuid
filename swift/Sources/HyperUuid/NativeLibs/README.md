# NativeLibs/

Populated per-RID with the platform's native `libhyperuuid` build (`NativeLibs/{rid}/{lib}`)
by CI and by `cargo build --release` for local dev — see `../../../../.gitignore`. This file
exists so the SPM `resources: [.copy("NativeLibs")]` rule has at least one tracked file on a
fresh checkout.
