#!/usr/bin/env bash
# Builds c/smoke/main.c and main.cpp against the four Apple mobile C archives through
# find_package, linked to hyperuuid::hyperuuid_static (they carry no shared library), on a Mac
# with Xcode, whose libc++ has C++23's <expected>:
#
#   ios-arm64           linked for an iOS device and checked to be one; no device runs it
#   iossimulator-arm64  run in the booted iOS simulator (`xcrun simctl spawn booted`)
#   maccatalyst-arm64   run as a Mac Catalyst process
#   maccatalyst-x64     run as a Mac Catalyst process under Rosetta 2
#
# Each run must print the archive's version. A cross build re-roots find_package's search
# paths into the SDK, so the archive is named by hyperuuid_DIR, the way an iOS toolchain
# file's user would name it.
#
# usage: c/smoke/apple-mobile.sh <directory holding the four unpacked archives> <version>
set -euo pipefail
root="$(cd "$1" && pwd)"
expected="$2"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

fail() { echo "smoke: $*" >&2; exit 1; }

build() {
  local rid="$1"
  shift
  local pkg
  pkg="$(find "$root" -maxdepth 1 -type d -name "*-$expected-$rid" | head -n 1)"
  [ -n "$pkg" ] || fail "no $rid archive at $expected under $root"
  [ ! -e "$pkg/lib/libhyperuuid.dylib" ] || fail "$rid carries a shared library"
  cmake -S "$here" -B "$work/$rid" -DHYPERUUID_SMOKE_STATIC=ON -DHYPERUUID_SMOKE_CXX=ON -DCMAKE_BUILD_TYPE=Release \
    -Dhyperuuid_DIR="$pkg/lib/cmake/hyperuuid" "$@" >"$work/$rid.log" 2>&1 \
    || { cat "$work/$rid.log"; fail "cmake configure ($rid)"; }
  cmake --build "$work/$rid" >>"$work/$rid.log" 2>&1 || { cat "$work/$rid.log"; fail "cmake build ($rid)"; }
  [ "$(cat "$work/$rid/package-version.txt")" = "$expected" ] \
    || fail "$rid: the CMake package is $(cat "$work/$rid/package-version.txt"), expected $expected"
  echo "$work/$rid"
}

# The platform a Mach-O was linked for, from LC_BUILD_VERSION, which otool prints as a number
# (mach-o/loader.h: 2 iOS, 6 Mac Catalyst, 7 the iOS simulator).
platform() {
  case "$(otool -l "$1" | awk '/LC_BUILD_VERSION/ { found = 1 } found && $1 == "platform" { print $2; exit }')" in
    2 | IOS) echo IOS ;;
    6 | MACCATALYST) echo MACCATALYST ;;
    7 | IOSSIMULATOR) echo IOSSIMULATOR ;;
    *) echo unknown ;;
  esac
}

# Checks both programs a build made, C and C++.
check() {
  local rid="$1" build="$2" want="$3"
  shift 3
  check_program "$rid" "$(cat "$build/smoke-path-Release.txt")" "$want" "$@"
  check_program "$rid (C++)" "$(cat "$build/smoke_cpp-path-Release.txt")" "$want" "$@"
}

check_program() {
  local rid="$1" program="$2" want="$3"
  shift 3
  local got output
  got="$(platform "$program")"
  [ "$got" = "$want" ] || fail "$rid: linked for platform $got, expected $want"
  if [ "$#" -gt 0 ]; then
    output="$("$@" "$program")" || fail "$rid: $program failed"
    output="$(printf '%s' "$output" | tr -d '\r')"
    [ "$output" = "$expected" ] || fail "$rid: hyperuuid_version() reports $output, expected $expected"
    echo "  $rid: $output ($want)"
  else
    echo "  $rid: linked ($want)"
  fi
}

ios=(-DCMAKE_SYSTEM_NAME=iOS -DCMAKE_OSX_ARCHITECTURES=arm64 -DCMAKE_OSX_DEPLOYMENT_TARGET=15.0)
catalyst_arm64=(-DCMAKE_OSX_ARCHITECTURES=arm64 "-DCMAKE_C_FLAGS=-target arm64-apple-ios15.0-macabi"
  "-DCMAKE_CXX_FLAGS=-target arm64-apple-ios15.0-macabi")
catalyst_x64=(-DCMAKE_OSX_ARCHITECTURES=x86_64 "-DCMAKE_C_FLAGS=-target x86_64-apple-ios15.0-macabi"
  "-DCMAKE_CXX_FLAGS=-target x86_64-apple-ios15.0-macabi")

echo "smoke: Apple mobile archives under $root"
build_dir="$(build ios-arm64 "${ios[@]}" -DCMAKE_OSX_SYSROOT=iphoneos)"
check ios-arm64 "$build_dir" IOS
build_dir="$(build iossimulator-arm64 "${ios[@]}" -DCMAKE_OSX_SYSROOT=iphonesimulator)"
check iossimulator-arm64 "$build_dir" IOSSIMULATOR xcrun simctl spawn booted
build_dir="$(build maccatalyst-arm64 "${catalyst_arm64[@]}")"
check maccatalyst-arm64 "$build_dir" MACCATALYST env
build_dir="$(build maccatalyst-x64 "${catalyst_x64[@]}")"
check maccatalyst-x64 "$build_dir" MACCATALYST arch -x86_64
echo "smoke: every Apple mobile archive reports $expected"
