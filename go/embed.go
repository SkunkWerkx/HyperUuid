package hyperuuid

import "embed"

// nativeFS bundles every platform's native build, since a Go module has no package-manager-
// level platform selection the way NuGet's RID folders or Python wheel tags do (same reason
// the Java binding bundles all six into one jar). currentTarget picks the right one at
// runtime.
//
// go:embed fails at compile time if this directory has no matching files, so native/README.md
// is committed as a placeholder — the per-RID native/{rid}/{lib} files themselves are
// gitignored and regenerated via `cargo build --release` (see ../.gitignore).
//
//go:embed native
var nativeFS embed.FS
