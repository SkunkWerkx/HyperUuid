#!/usr/bin/env bash
# Builds c/smoke/main.c against an unpacked C release archive, every way a consumer would on
# this platform, runs each program, and holds the version each reports to the archive's:
#
#   CMake       find_package(hyperuuid), hyperuuid::hyperuuid and hyperuuid::hyperuuid_static,
#               from C and, where the C++ compiler has C++23's <expected>, from C++ through
#               hyperuuid.hpp (SMOKE_CXX=1 makes that required)
#   pkg-config  `pkg-config --cflags --libs hyperuuid`, and the archive in ${libdir} named
#               directly (not on Windows, where CMake is the way in)
#
# A program linked to the shared library must record the library's own name (@rpath/ on
# macOS, the soname elsewhere), never the path it was linked from, and one linked statically
# must not need the shared library at all. The WebAssembly archive has no shared library and
# no runner to execute it, so it is not smoke-tested here.
#
# usage: c/smoke/run.sh <unpacked archive> [<expected version>]
#        SMOKE_ARCH=x86_64 c/smoke/run.sh ...   on Apple silicon, osx-x64 under Rosetta
set -euo pipefail
pkg="$(cd "$1" && pwd)"
expected="${2:-}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*) os=windows ;;
  Darwin) os=macos ;;
  *) os=linux ;;
esac

fail() { echo "smoke: $*" >&2; exit 1; }

# macOS only: build for SMOKE_ARCH and run under it, so osx-x64 is tested on an arm64 runner.
arch_flags=() cmake_arch=() run_as=()
if [ -n "${SMOKE_ARCH:-}" ]; then
  [ "$os" = macos ] || fail "SMOKE_ARCH is for macOS only"
  arch_flags=(-arch "$SMOKE_ARCH") cmake_arch=(-DCMAKE_OSX_ARCHITECTURES="$SMOKE_ARCH") run_as=(arch "-$SMOKE_ARCH")
fi

# One version for the archive: the CMake package's, pkg-config's and the library's must agree,
# and with the caller's when one is given.
check_version() {
  local how="$1" reported="$2"
  echo "  $how: $reported"
  if [ -z "$expected" ]; then
    expected="$reported"
  elif [ "$reported" != "$expected" ]; then
    fail "$how reports $reported, expected $expected"
  fi
}

# What a program asks the dynamic loader for, one name per line.
needed() {
  case "$os" in
    macos) otool -L "$1" | tail -n +2 | awk '{print $1}' ;;
    linux) readelf -d "$1" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p' ;;
  esac
}

check_shared_link() {
  [ "$os" = windows ] && return 0
  local want=libhyperuuid.so
  [ "$os" = macos ] && want=@rpath/libhyperuuid.dylib
  needed "$1" | grep -qx "$want" || fail "$1 does not load $want: $(needed "$1" | tr '\n' ' ')"
}

check_static_link() {
  [ "$os" = windows ] && return 0
  if needed "$1" | grep -q hyperuuid; then
    fail "$1, linked statically, still loads $(needed "$1" | grep hyperuuid)"
  fi
}

# Runs a smoke program, after checking what it links, and holds its version to the archive's.
run_program() {
  local how="$1" program="$2" linkage="$3" output
  if [ "$linkage" = static ]; then check_static_link "$program"; else check_shared_link "$program"; fi
  output="$("${run_as[@]}" "$program")" || fail "$program ($how) failed"
  check_version "hyperuuid_version() ($how)" "$(printf '%s' "$output" | tr -d '\r')"
}

cmake_build() {
  local name="$1" static="$2" build="$work/cmake-$1"
  cmake -S "$here" -B "$build" -DCMAKE_PREFIX_PATH="$pkg" -DHYPERUUID_SMOKE_STATIC="$static" "${cmake_arch[@]}" \
    -DHYPERUUID_SMOKE_CXX="$([ "${SMOKE_CXX:-}" = 1 ] && echo ON || echo OFF)" \
    -DCMAKE_BUILD_TYPE=Release >"$work/$name.log" 2>&1 || { cat "$work/$name.log"; fail "cmake configure ($name)"; }
  cmake --build "$build" --config Release >>"$work/$name.log" 2>&1 || { cat "$work/$name.log"; fail "cmake build ($name)"; }
  check_version "CMake package ($name)" "$(cat "$build/package-version.txt")"
  local program linkage
  linkage="$([ "$static" = ON ] && echo static || echo shared)"
  program="$(cat "$build/smoke-path-Release.txt")"
  [ -f "$program" ] || fail "no smoke program at $program"
  run_program "$name" "$program" "$linkage"
  if [ -f "$build/smoke_cpp-path-Release.txt" ]; then
    program="$(cat "$build/smoke_cpp-path-Release.txt")"
    run_program "C++, $name" "$program" "$linkage"
  else
    echo "  (no C++23 <expected> here: C++ ($name) not built)"
  fi
}

echo "smoke: $pkg"
cmake_build shared OFF
cmake_build static ON

if [ "$os" != windows ]; then
  export PKG_CONFIG_PATH="$pkg/lib/pkgconfig"
  check_version "pkg-config --modversion" "$(pkg-config --modversion hyperuuid)"
  libdir="$(pkg-config --variable=libdir hyperuuid)"
  # shellcheck disable=SC2046 # pkg-config's flags are meant to split
  cc "${arch_flags[@]}" -o "$work/pc-shared" "$here/main.c" $(pkg-config --cflags --libs hyperuuid) -Wl,-rpath,"$libdir"
  run_program "pkg-config, shared" "$work/pc-shared" shared
  # shellcheck disable=SC2046
  cc "${arch_flags[@]}" -o "$work/pc-static" "$here/main.c" $(pkg-config --cflags hyperuuid) "$libdir/libhyperuuid.a"
  run_program "pkg-config, static" "$work/pc-static" static
fi

echo "smoke: every build of $pkg reports $expected"
