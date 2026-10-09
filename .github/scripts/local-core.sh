#!/usr/bin/env bash
# Builds the core from this checkout for the three bindings whose suites otherwise run
# against committed binaries (php/src/native, go/staticlib, the Swift artifact bundle), and
# touches none of those: everything lands under rust/target/, which git ignores.
#
#   PHP    rust/target/release/libhyperuuid.so     HYPERUUID_NATIVE_LIBRARY=<that path> vendor/bin/phpunit
#   Go     rust/target/local-core/go/staticlib/    go test -tags hyperuuid_local ./...
#   Swift  rust/target/local-core/swift/           HYPERUUID_LOCAL_CORE=1 swift test
#
# The archives come from the forge's build-static-libs.sh, because a plain `cargo staticlib`
# archive does not link into Go (its trim is what drops the rust_eh_personality reference).
# The script writes into a repository root's go/ and swift/ trees, so it is handed a
# stand-in root, rust/target/local-core/, whose rust/ is this checkout's own and whose go/
# and swift/ are the output directories above. Only the host's targets are built: the musl
# archive Go links on Linux, and the archive Swift picks for the host triple.
#
# With ANDROID=1 it also builds the two Android archives (aarch64 and x86_64), which Go
# (GOOS=android) and Swift (the Swift SDK for Android) link, for a suite cross-compiled and
# run on an emulator or device (.github/scripts/android_build_suite.sh).
#
# usage: .github/scripts/local-core.sh    (FORGE=<path> if the SkunkWerkx/.github checkout
#                                          is not ../.github beside this repository)
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
forge="${FORGE:-$repo/../.github}/.github/actions/static-libs/build-static-libs.sh"
[ -f "$forge" ] || { echo "error: $forge not found; set FORGE to the SkunkWerkx/.github checkout" >&2; exit 1; }

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) targets=(x86_64-unknown-linux-musl x86_64-unknown-linux-gnu) lib=libhyperuuid.so ;;
  Linux-aarch64) targets=(aarch64-unknown-linux-musl aarch64-unknown-linux-gnu) lib=libhyperuuid.so ;;
  Darwin-x86_64) targets=(x86_64-apple-darwin) lib=libhyperuuid.dylib ;;
  Darwin-arm64) targets=(aarch64-apple-darwin) lib=libhyperuuid.dylib ;;
  *) echo "error: no local core for $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac

[ "${ANDROID:-}" = 1 ] && targets+=(aarch64-linux-android x86_64-linux-android)

(cd "$repo/rust" && cargo cdylib)

stand_in="$repo/rust/target/local-core"
bundle="$stand_in/swift/HyperUuidCore.artifactbundle"
mkdir -p "$stand_in/go" "$bundle"
ln -sfn ../.. "$stand_in/rust"
ln -sfn "$repo/swift/HyperUuidCore.artifactbundle/include" "$bundle/include"
bash "$forge" hyperuuid HyperUuid "$stand_in" "${targets[@]}"

cat <<EOF

local core built from $(git -C "$repo" rev-parse --short HEAD)$(git -C "$repo" diff --quiet HEAD -- rust || echo ' plus uncommitted changes'):
  php/    HYPERUUID_NATIVE_LIBRARY=$repo/rust/target/release/$lib vendor/bin/phpunit
  go/     go test -tags hyperuuid_local ./...
  swift/  HYPERUUID_LOCAL_CORE=1 swift test
EOF
