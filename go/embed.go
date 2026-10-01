package hyperuuid

import "embed"

// nativeFS bundles every platform's native build, since a Go module has no package-manager-
// level platform selection the way NuGet's RID folders or Python wheel tags do (same reason
// the Java binding bundles all of them into one jar). currentTarget picks the right one at
// runtime.
//
// The per-RID native/{rid}/{lib} files are committed to git — a go:embed consumer gets
// whatever's literally in the tree at the resolved module version, with no packing step to
// stage them in (see native/README.md); stage-native-binaries.yml keeps them fresh. The
// README also guarantees go:embed always has a match, since it fails at compile time on an
// empty directory.
//
//go:embed native
var nativeFS embed.FS
