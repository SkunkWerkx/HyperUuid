#!/usr/bin/env bash
# Which archive each Go build links, held to a table.
#
# go/backend_static.go picks the core's static library with #cgo lines keyed on GOOS, GOARCH
# and build tags, and two of Go's rules make that easy to get wrong without any build
# failing on the platforms CI runs: GOOS=ios also satisfies `darwin`, and GOOS=android also
# satisfies `linux`, so an unguarded line hands iOS the macOS archive or Android the Linux
# one. iOS, its simulator and Mac Catalyst all build as ios/arm64 besides, told apart by tag
# alone. `go list` evaluates the constraints and the #cgo lines without a C compiler or the
# target's SDK, so every row is checked from one Linux job.
#
# usage: .github/scripts/check_go_archives.sh    (from anywhere in the repository)
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../../go"

# GOOS GOARCH tags expected; tags `-` for none, expected `unsupported` for a build that must
# land on unsupported.go's compile error.
rows="
linux   amd64 -                  staticlib/linux_amd64
linux   arm64 -                  staticlib/linux_arm64
darwin  amd64 -                  staticlib/darwin_amd64
darwin  arm64 -                  staticlib/darwin_arm64
windows amd64 -                  staticlib/windows_amd64
windows arm64 -                  staticlib/windows_arm64
ios     arm64 -                  staticlib/ios_arm64
ios     arm64 iossimulator       staticlib/iossimulator_arm64
ios     arm64 macos,maccatalyst  staticlib/maccatalyst_arm64
ios     amd64 macos,maccatalyst  staticlib/maccatalyst_amd64
ios     amd64 -                  unsupported
android arm64 -                  staticlib/android_arm64
android amd64 -                  staticlib/android_amd64
linux   arm64 hyperuuid_local    ../rust/target/local-core/go/staticlib/linux_arm64
android amd64 hyperuuid_local    ../rust/target/local-core/go/staticlib/android_amd64
darwin  arm64 hyperuuid_local    ../rust/target/local-core/go/staticlib/darwin_arm64
"

failed=0
while read -r goos goarch tags expected; do
  [ -n "$goos" ] || continue
  args=()
  [ "$tags" = - ] || args=(-tags "$tags")
  listed=$(GOOS=$goos GOARCH=$goarch CGO_ENABLED=1 \
    go list "${args[@]}" -f '{{join .CgoFiles " "}}|{{join .GoFiles " "}}|{{join .CgoLDFLAGS " "}}' .)
  IFS='|' read -r cgo_files go_files ldflags <<<"$listed"
  stub=no
  [[ " $go_files " == *" unsupported.go "* ]] && stub=yes
  got="$cgo_files|$stub|$ldflags"
  if [ "$expected" = unsupported ]; then
    want="|yes|"
  else
    want="backend_static.go|no|$PWD/$expected/libhyperuuid.a"
  fi
  if [ "$got" = "$want" ]; then
    printf 'ok   %-8s %-6s %-18s %s\n' "$goos" "$goarch" "$tags" "$expected"
  else
    printf 'FAIL %-8s %-6s %-18s wanted %s, got: %s\n' "$goos" "$goarch" "$tags" "$expected" "$got"
    failed=1
  fi
done <<<"$rows"
exit "$failed"
