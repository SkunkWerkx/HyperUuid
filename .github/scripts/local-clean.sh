#!/usr/bin/env bash
# Removes every binding's staged build artifact from this checkout, so nothing a suite loads
# or a pack picks up predates the core about to be built. Every path here is gitignored
# output that some earlier local build or pack rehearsal left behind and that a later run
# would load as if it were current: a C# AOT publish links whatever archive sits in
# staticlibs/, the Ruby specs load the bundle staged under lib/, Python's package imports the
# extension beside it, the TypeScript suite loads the addon and WASI module in typescript/,
# and R's Makevars build from the vendor tarball. CI never has this problem: each leg builds
# its own core into an empty tree. `local-core.sh` runs this first.
#
# Each line below names what rebuilds the artifact it removes. Cargo's own target/ trees
# (rust/, java/jni/, typescript/, r/src/rust/) stay: cargo rebuilds them from the sources, and
# the Go, Swift and PHP suites refuse a core older than rust/ on their own.
#
# usage: .github/scripts/local-clean.sh
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo"

remove() {
  local rebuild="$1"; shift
  local any=
  for path in "$@"; do
    [ -e "$path" ] || continue
    any=1
    rm -rf "$path"
  done
  [ -n "$any" ] && echo "removed $*  ->  $rebuild"
  return 0
}

shopt -s nullglob

# C#: the pack's staged natives and the AOT archives, both packed by the csproj's runtimes/**
# and staticlibs/** items and linked by the targets whenever present.
remove "local-core.sh stages the host's archive; a pack rehearsal stages the rest" \
  csharp/HyperUuid/runtimes csharp/HyperUuid/staticlibs
# Swift: every archive variant and the bundle manifest; include/ is tracked and stays.
variants=()
for d in swift/HyperUuidCore.artifactbundle/*/; do
  [ "$(basename "$d")" = include ] || variants+=("${d%/}")
done
remove ".github/scripts/local-core.sh" \
  swift/HyperUuidCore.artifactbundle/info.json ${variants[@]+"${variants[@]}"}
# Java: Gradle's generated-resources, where it stages the core and the JNI shim.
remove "./gradlew test (after cargo cdylib and cargo build --release --locked --lib in java/jni/)" \
  java/build
# Python: the extension maturin develop places beside the package.
remove "maturin develop --release in python/" \
  python/src/hyperuuid/_native*.so python/src/hyperuuid/_native*.pyd
# Ruby: the bundles rake native:dev stages per Ruby minor, and the flat pair beside them.
remove "bundle exec rake native:dev in ruby/" \
  ruby/lib/hyperuuid/[0-9]*/ ruby/lib/hyperuuid_native.so ruby/lib/hyperuuid_native.bundle
# Ruby's Fiddle backend: a cdylib staged per platform, which Fiddle loads ahead of
# rust/target/release, so a stale one runs an old core under a current suite.
remove "nothing to stage: Fiddle then loads rust/target/release (cargo cdylib)" \
  ruby/lib/hyperuuid/native/*/
# Ruby's wasm32-wasip1 archives, staged for rake wasm:gem.
remove "rake wasm:gem stages them from CI's ruby-wasm artifacts" \
  ruby/ext/hyperuuid_native/*/
# R: the vendor tarball and lockfile, the staged corpus, and the compiled objects.
remove "r/tools/vendor.sh" \
  r/src/rust/vendor.tar.xz r/src/rust/Cargo.lock r/tests/testthat/corpus r/src/*.o r/src/*.so r/src/*.dll
# TypeScript: the addon, the WASI module and its loaders, the browser page's bundle, and the
# consumer tests' work directory.
remove "bun run build and bun run build:wasi in typescript/" \
  typescript/*.node typescript/hyperuuid.wasm32-wasip1*.wasm typescript/hyperuuid.wasip1* typescript/browser.js \
  typescript/browser-smoke/dist typescript/consumer-tests/work

echo "local-clean: the checkout holds no staged binding artifact"

# The machine's cruft too, on every retry: old agent scratch folders and Docker's stopped
# containers, unused images and build cache, which once filled a 460 GB disk. Forge's
# dev_purge.sh keeps a scratch folder written in the last hour, running containers and volumes.
# A missing Forge checkout or a failed prune never stops a build. FORGE as in local-core.sh.
purge="${FORGE:-}/.github/scripts/dev_purge.sh"
[ -n "${FORGE:-}" ] || purge="$repo/../.github/.github/scripts/dev_purge.sh"
[ -f "$purge" ] || purge="$repo/../HyperForge/.github/scripts/dev_purge.sh"
if [ -f "$purge" ]; then
  bash "$purge" --yes --only scratch,docker || echo "local-clean: dev_purge.sh failed; carrying on" >&2
else
  echo "local-clean: dev_purge.sh not found; set FORGE to the SkunkWerkx/.github checkout" >&2
fi
