# native/

Populated per-RID with the platform's native `libhyperuuid` build (`native/{rid}/{lib}`) by CI
and by `cargo build --release` for local dev — see `../../.gitignore`. This file exists so
`//go:embed native` (in `embed.go`) has at least one tracked file to match on a fresh checkout;
`go:embed` fails to compile against an otherwise-empty directory.
