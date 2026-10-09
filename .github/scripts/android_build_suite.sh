#!/usr/bin/env bash
# Cross-compiles one binding's test suite for Android and stages it for
# android_device_test.sh: the test executable, the shared libraries it needs at run time,
# and the conformance corpus, in one directory to push.
#
#   android_build_suite.sh go    <x86_64|aarch64> <stage-dir> [go build tags]
#   android_build_suite.sh swift <x86_64|aarch64> <stage-dir>
#
# Needs ANDROID_NDK_HOME. Go links go/staticlib/android_{amd64,arm64} (or, with the
# hyperuuid_local tag, what local-core.sh built), through the NDK's clang as cgo's CC, into
# one static test binary. Swift needs a toolchain of 6.3 or later on PATH, the first with a
# Swift SDK for Android; the SDK matching that toolchain's version is installed from
# swift.org if it is not already, checked against the checksum swift.org publishes for it,
# and pointed at the NDK. Its runtime is shared libraries, so they are staged beside the
# test bundle along with the NDK's libc++_shared.so. Swift's Android floor is API 28, Go's
# and the archives' 21.
#
# ci.yml's test-android job runs both, and so does a local emulator run:
#
#   ANDROID=1 .github/scripts/local-core.sh
#   .github/scripts/android_build_suite.sh go x86_64 /tmp/go hyperuuid_local
#   .github/scripts/android_device_test.sh /tmp/go ./hyperuuid.test -test.v
#   HYPERUUID_LOCAL_CORE=1 .github/scripts/android_build_suite.sh swift x86_64 /tmp/swift
#   .github/scripts/android_device_test.sh /tmp/swift ./HyperUuidPackageTests.xctest
set -euo pipefail

usage="usage: android_build_suite.sh <go|swift> <x86_64|aarch64> <stage-dir> [go build tags]"
binding="${1:?$usage}"
arch="${2:?$usage}"
stage="${3:?$usage}"
tags="${4:-}"
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ndk="${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to the Android NDK}"
prebuilt="$ndk/toolchains/llvm/prebuilt/linux-x86_64"

case "$arch" in
  x86_64) goarch=amd64 ;;
  aarch64) goarch=arm64 ;;
  *) echo "$usage" >&2; exit 2 ;;
esac

rm -rf "$stage"
mkdir -p "$stage"
# Absolute from here on: the Go build runs with -C go/, which would otherwise resolve a
# relative stage against that directory instead of the caller's.
stage="$(cd "$stage" && pwd)"
cp -r "$repo/corpus" "$stage/corpus"

# Each test executable is linked with 16 KB pages, as NDK r28 and later do by default and an
# older NDK on the machine would not: the emulator CI runs it on uses 16 KB pages, where a
# 4 KB-aligned executable does not load, and that would fail the run for a reason that is
# not the core's.
case "$binding" in
  go)
    CC="$prebuilt/bin/$arch-linux-android21-clang" GOOS=android GOARCH="$goarch" CGO_ENABLED=1 \
      go -C "$repo/go" test -c ${tags:+-tags "$tags"} -ldflags=-extldflags=-Wl,-z,max-page-size=16384 \
      -o "$stage/hyperuuid.test" .
    ;;
  swift)
    version="$(swift --version 2>&1 | sed -n 's/.*Swift version \([0-9][0-9.]*\).*/\1/p' | head -1)"
    [ -n "$version" ] || { echo "error: no Swift toolchain on PATH" >&2; exit 1; }
    # A toolchain says 6.4 where swift.org's release list and download paths say 6.4.0.
    case "$version" in *.*.*) ;; *) version="$version.0" ;; esac
    # The release's tag (swift-6.4.0-RELEASE) names both its download directory and the
    # installed SDK, and its listing carries the checksum.
    read -r tag checksum < <(curl -sSfL https://www.swift.org/api/v1/install/releases.json | python3 -c '
import json, sys
release = next((r for r in json.load(sys.stdin) if r["name"] == sys.argv[1]), {})
sdk = next((p for p in release.get("platforms", []) if p.get("platform") == "android-sdk"), None)
print(release.get("tag", "-"), sdk["checksum"] if sdk else "-")' "$version")
    [ "$checksum" != - ] || { echo "error: swift.org lists no Android SDK for Swift $version" >&2; exit 1; }
    sdk="${tag}_android"
    if ! swift sdk list | grep -qx "$sdk"; then
      swift sdk install \
        "https://download.swift.org/${tag,,}/android-sdk/$tag/$sdk.artifactbundle.tar.gz" \
        --checksum "$checksum"
    fi
    bundle="$(swift sdk configure --show-configuration "$sdk" "$arch-unknown-linux-android28" \
      | sed -n 's|^sdkRootPath: \(.*\)/swift-android/ndk-sysroot$|\1|p')"
    [ -d "$bundle/swift-android" ] || { echo "error: cannot locate the $sdk bundle" >&2; exit 1; }
    # The SDK's own setup links its sysroot (ndk-sysroot, absent until it runs) and clang
    # resources into the NDK, and fails if run a second time.
    if [ ! -e "$bundle/swift-android/ndk-sysroot" ]; then
      ANDROID_NDK_HOME="$ndk" bash "$bundle/swift-android/scripts/setup-android-sdk.sh"
    fi
    # The native build system on every toolchain: 6.4 defaults to swift-build, which lays
    # the tests out as a test-runner executable and a shared library instead of the one
    # .xctest executable 6.3 writes, and this script stages one shape.
    scratch="$repo/swift/.build/android"
    swift build --package-path "$repo/swift" --build-tests --build-system native \
      --swift-sdk "$arch-unknown-linux-android28" --scratch-path "$scratch" \
      -Xlinker -z -Xlinker max-page-size=16384
    cp "$scratch/$arch-unknown-linux-android28/debug/HyperUuidPackageTests.xctest" "$stage/"
    cp "$bundle/swift-android/swift-resources/usr/lib/swift-$arch/android/"*.so "$stage/"
    cp "$prebuilt/sysroot/usr/lib/$arch-linux-android/libc++_shared.so" "$stage/"
    ;;
  *) echo "$usage" >&2; exit 2 ;;
esac

echo "staged $binding ($arch) in $stage: $(du -sh "$stage" | cut -f1)"
