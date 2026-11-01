#!/usr/bin/env bash
# The installation scenarios for the npm packages, against a local registry.
#
#   run.sh <tarballs-dir> <scenario...>
#
# Serves the packed tarballs (npm_package.py's output: the root, the platform packages and the
# WASI package) from a Verdaccio on port 4873, then runs each named scenario in a fresh
# consumer directory under work/<scenario>/ with its own .npmrc and npm cache, asserting which
# backend loaded (`target=native` or `target=wasm32-wasip1`, printed by smoke.cjs) and what
# stderr may carry. Exits non-zero on the first failure. Scenarios (the design spec's 1 to 9):
#
#   negative-unpublished    npm install fails while the WASI package is not yet published,
#                           naming it (runs before that package is published)
#   plain                   loads native; the WASI package is installed; stderr is empty
#   omit-optional           --omit=optional loads wasm32-wasip1; stderr carries only Node's
#                           ExperimentalWarning for WASI (from "work/consumer two/": the space)
#   absent-native           plain install, this platform's package deleted: wasm32-wasip1 loads
#                           and nativeVersion() is the release version
#   forced-wasi             NAPI_RS_FORCE_WASI=error on the plain install; Bun's result is
#                           printed as `bun: ...` and never fails the script
#   negative-deleted-package  --omit=optional install, then the WASI package directory deleted:
#                           require succeeds, isAvailable() is false, a door throws
#                           ERR_HYPERUUID_UNAVAILABLE whose cause chain names the native
#                           attempts and the WASI one
#   negative-deleted-wasm   the same with only the .wasm deleted, the loader left in place
#   mac-lockfile            npm/cli#4828 with the npm the Node floor bundles (LOCKFILE_NPM below):
#                           a darwin-arm64 install, the lockfile deleted and regenerated beside
#                           node_modules names darwin-arm64 only, and `npm ci` from it as a Linux
#                           x64 consumer installs no native and loads wasm32-wasip1 (needs the
#                           darwin-arm64 package in the registry; runs on any host)
#   floor                   container/consumer.sh in node:22.13.0-alpine with engine-strict,
#                           then node:22-alpine and node:24-alpine (Docker)
#   s390x                   container/consumer.sh in node:24-alpine for linux/s390x under QEMU, a
#                           big-endian host: the smoke's checks must pass, and the process must
#                           then fail to exit (killed by the bound), the documented upstream loss (the
#                           runtime reads its reference counts host-endian over little-endian
#                           wasm memory); an exit of 0 fails the scenario, since the docs would
#                           then be stale
#   bundlers                esbuild and webpack bundle bundled.cjs; the bundle runs from a
#                           directory outside the repository under NAPI_RS_FORCE_WASI=error,
#                           plain first, then with @skunkwerkx/hyperuuid external and installed beside it
#   all                     every scenario above, in that order
#
# Containers reach the registry over --network host at 127.0.0.1 on Linux; on Docker Desktop
# set REGISTRY_HOST=host.docker.internal (Verdaccio listens on 0.0.0.0 either way).
# FLOOR_IMAGE overrides the exact-floor image (the negative control uses node:22.12.0-alpine).
set -euo pipefail

# The npm the mac-lockfile scenario reproduces npm/cli#4828 with: the one Node 22.13.0, the
# floor, bundles, so it moves with the floor (CLAUDE.md, "The Node floor"). npm 11.20.0 has
# the fix on both sides (a regenerated lockfile names every platform, and `npm ci` refuses one
# that does not), so the scenario asserts the shape this npm writes and fails loudly on another.
LOCKFILE_NPM=10.9.2

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
# CORPUS_DIR overrides the corpus (the negative control points it at an empty directory).
corpus="${CORPUS_DIR:-$root/corpus}"
cd "$here"

usage() { sed -n '2,/^set -euo/p' "$0" | sed '$d' | sed 's/^# \{0,1\}//'; exit 2; }
[ $# -ge 2 ] || usage
tarballs="$(cd "$1" && pwd)"
shift
scenarios=("$@")
if [ "${scenarios[*]}" = all ]; then
  scenarios=(negative-unpublished plain omit-optional absent-native forced-wasi negative-deleted-package
             negative-deleted-wasm mac-lockfile floor s390x bundlers)
fi

# A relative require from the package's own directory: a POSIX path inside a quoted
# JavaScript string is not an argument Git Bash would convert, so Windows Node would read
# /d/a/... against its own drive root.
version="$(cd "$root/typescript" && node -p "require('./package.json').version")"
crate=hyperuuid
scope="@skunkwerkx/$crate"
wasi=wasm32-wasip1
case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) windows=1 ;; *) windows=0 ;; esac
# Node on Windows wants a Windows path. Git Bash converts POSIX paths in command arguments,
# and in environment variables by rules of its own (MSYS2_ENV_CONV_EXCL exists to turn that
# off), so every path that lands in a variable goes through native_path here: already in
# Windows form, it is the same whichever rule applies. Files take the path as a filesystem
# argument (pwd -W of the file's directory, then the name).
native_path() { if [ "$windows" = 1 ]; then (cd "$1" && pwd -W); else echo "$1"; fi; }
native_file() { echo "$(native_path "$(dirname "$1")")/$(basename "$1")"; }
corpus_env="$(native_path "$corpus")"

if [ ! -x node_modules/.bin/verdaccio ] && [ ! -f node_modules/.bin/verdaccio ]; then
  npm ci --no-audit --no-fund
fi
rm -rf work
mkdir -p work/storage
# Every npm here reads this user config and no other, so the developer's ~/.npmrc never
# reaches a scenario; the publisher's token lands in it below.
: > "$here/work/npmrc"
export NPM_CONFIG_USERCONFIG
NPM_CONFIG_USERCONFIG="$(native_file "$here/work/npmrc")"

registry_port=4873
registry="http://127.0.0.1:$registry_port"
container_registry="http://${REGISTRY_HOST:-127.0.0.1}:$registry_port"
if [ -z "${REGISTRY_HOST:-}" ]; then docker_net=(--network host); else docker_net=(); fi

node node_modules/verdaccio/bin/verdaccio --config verdaccio.yaml --listen "0.0.0.0:$registry_port" \
  > work/verdaccio.log 2>&1 &
registry_pid=$!
cleanup() { kill "$registry_pid" 2>/dev/null || true; }
trap cleanup EXIT
for _ in $(seq 1 60); do
  if curl -sf "$registry/-/ping" > /dev/null 2>&1; then break; fi
  sleep 0.5
done
curl -sf "$registry/-/ping" > /dev/null || { echo "Verdaccio did not come up"; cat work/verdaccio.log; exit 1; }
echo "registry: $registry (containers: $container_registry)"
echo "node: $(node --version), npm: $(npm --version)"

# npm refuses to publish with no credentials at all, so one publisher is created through the
# registry's own user endpoint and its token goes into the user config.
token="$(curl -sf -X PUT "$registry/-/user/org.couchdb.user:publisher" -H 'content-type: application/json' \
  -d '{"name":"publisher","password":"publisher-password"}' | node -p "JSON.parse(require('fs').readFileSync(0,'utf8')).token")"
[ -n "$token" ] || { echo "no publisher token"; exit 1; }
printf '//127.0.0.1:%s/:_authToken=%s\n' "$registry_port" "$token" >> "$NPM_CONFIG_USERCONFIG"

publish() { npm publish "$1" --registry "$registry" --access public > "work/publish-$(basename "$1").log" 2>&1 \
  || { cat "work/publish-$(basename "$1").log"; exit 1; }; echo "published $(basename "$1")"; }
shopt -s nullglob
natives=()
wasi_tarball=""
for f in "$tarballs"/skunkwerkx-"$crate"-*-"$version".tgz; do
  case "$f" in *"-$wasi-$version.tgz") wasi_tarball="$f" ;; *) natives+=("$f") ;; esac
done
shopt -u nullglob
root_tarball="$tarballs/skunkwerkx-$crate-$version.tgz"
[ -f "$root_tarball" ] || { echo "no root tarball $root_tarball"; exit 1; }
[ -n "$wasi_tarball" ] || { echo "no WASI tarball in $tarballs"; exit 1; }
for f in "${natives[@]}"; do publish "$f"; done
publish "$root_tarball"
echo "natives published: ${#natives[@]}"

ok() { echo "OK  $1"; }
die() { echo "FAIL $*" >&2; exit 1; }

# A fresh consumer directory with its own manifest, registry setting and cache. The manifest
# matters: npm takes its project config from the nearest directory holding a package.json or a
# node_modules, so without one the .npmrc here would be skipped for this harness's own.
consumer() {
  local dir="$here/work/$1"
  rm -rf "$dir"
  mkdir -p "$dir/cache"
  printf '{"name":"consumer","private":true}\n' > "$dir/package.json"
  printf 'registry=%s\naudit=false\nfund=false\nupdate-notifier=false\n' "$registry" > "$dir/.npmrc"
  cp "$here/smoke.cjs" "$dir/smoke.cjs"
  echo "$dir"
}
install_in() { # install_in <dir> <npm args...>
  local dir="$1"; shift
  (cd "$dir" && npm install --cache "$dir/cache" "$@" > "$dir/install.log" 2>&1) || { cat "$dir/install.log"; die "npm install failed in $dir"; }
}
# smoke <dir> [VAR=value...]: runs smoke.cjs there; stdout and stderr land in out.txt and err.txt.
smoke() {
  local dir="$1"; shift
  local status=0
  (cd "$dir" && env HYPERUUID_CORPUS="$corpus_env" "$@" node ./smoke.cjs > out.txt 2> err.txt) || status=$?
  sed 's/^/  out: /' "$dir/out.txt"
  sed 's/^/  err: /' "$dir/err.txt"
  return $status
}
expect_target() { grep -q "target=$2" "$1/out.txt" || die "$3: expected target=$2"; }
expect_stderr_empty() { [ ! -s "$1/err.txt" ] || die "$2: stderr is not empty"; }
expect_stderr_experimental_only() {
  if grep -v -E 'ExperimentalWarning: WASI|--trace-warnings' "$1/err.txt" | grep . >/dev/null; then
    die "$2: stderr carries more than the WASI ExperimentalWarning"
  fi
}
platform_packages() { # the installed native platform packages, if any
  local p
  for p in "$1/node_modules/@skunkwerkx/$crate"-*; do
    [ -d "$p" ] || continue
    case "$p" in *"-$wasi") ;; *) basename "$p" ;; esac
  done
}

scenario_negative_unpublished() {
  local dir; dir="$(consumer negative-unpublished)"
  if (cd "$dir" && npm install --cache "$dir/cache" "$scope@$version" > install.log 2>&1); then
    cat "$dir/install.log"; die "negative-unpublished: npm install succeeded without the WASI package"
  fi
  grep -q "$scope-$wasi" "$dir/install.log" || { cat "$dir/install.log"; die "negative-unpublished: the error does not name $scope-$wasi"; }
  grep -E "$scope-$wasi|E404|notarget" "$dir/install.log" | head -3 | sed 's/^/  /'
  ok "negative-unpublished: npm install fails and names $scope-$wasi"
}

scenario_plain() {
  local dir; dir="$(consumer plain)"
  install_in "$dir" "$scope@$version"
  [ -f "$dir/node_modules/$scope-$wasi/package.json" ] || die "plain: the WASI package is not installed"
  smoke "$dir" || die "plain: smoke failed"
  expect_target "$dir" native plain
  expect_stderr_empty "$dir" plain
  ok "plain: native loaded, WASI package present, stderr empty"
}

scenario_omit_optional() {
  local dir; dir="$(consumer "consumer two")"
  install_in "$dir" --omit=optional "$scope@$version"
  [ -z "$(platform_packages "$dir")" ] || die "omit-optional: a platform package was installed"
  smoke "$dir" || die "omit-optional: smoke failed"
  expect_target "$dir" "$wasi" omit-optional
  expect_stderr_experimental_only "$dir" omit-optional
  ok "omit-optional: $wasi loaded from \"work/consumer two\", only the ExperimentalWarning on stderr"
}

scenario_absent_native() {
  local dir; dir="$(consumer absent-native)"
  install_in "$dir" "$scope@$version"
  local removed; removed="$(platform_packages "$dir")"
  [ -n "$removed" ] || die "absent-native: no platform package to delete"
  for p in $removed; do rm -rf "$dir/node_modules/@skunkwerkx/$p"; done
  smoke "$dir" || die "absent-native: smoke failed"
  expect_target "$dir" "$wasi" absent-native
  grep -q "version=$version" "$dir/out.txt" || die "absent-native: nativeVersion() is not $version"
  expect_stderr_experimental_only "$dir" absent-native
  ok "absent-native: deleted $removed; $wasi loaded, version $version"
}

scenario_forced_wasi() {
  local dir; dir="$(consumer forced-wasi)"
  install_in "$dir" "$scope@$version"
  smoke "$dir" NAPI_RS_FORCE_WASI=error || die "forced-wasi: smoke failed"
  expect_target "$dir" "$wasi" forced-wasi
  expect_stderr_experimental_only "$dir" forced-wasi
  ok "forced-wasi: $wasi loaded under NAPI_RS_FORCE_WASI=error on Node"
  if command -v bun > /dev/null 2>&1; then
    local result
    if result="$(cd "$dir" && HYPERUUID_CORPUS="$corpus_env" NAPI_RS_FORCE_WASI=error bun ./smoke.cjs 2>&1)"; then
      echo "bun: $(bun --version): $(head -1 <<< "$result")"
    else
      echo "bun: $(bun --version): failed: $(grep -m1 -E 'rror|not a function' <<< "$result" || head -1 <<< "$result")"
    fi
  else
    echo "bun: not installed"
  fi
}

scenario_negative_deleted_package() {
  local dir; dir="$(consumer negative-deleted-package)"
  install_in "$dir" --omit=optional "$scope@$version"
  rm -rf "$dir/node_modules/$scope-$wasi"
  smoke "$dir" || die "negative-deleted-package: smoke exited non-zero"
  grep -q 'available=false target=none' "$dir/out.txt" || die "negative-deleted-package: expected available=false"
  grep -q "cause: .*$crate-\(darwin\|linux\|win32\)\|cause: .*$crate\.\(darwin\|linux\|win32\)" "$dir/out.txt" \
    || die "negative-deleted-package: the cause chain names no native attempt"
  grep -q "cause: .*$wasi\|cause: .*wasip1" "$dir/out.txt" || die "negative-deleted-package: the cause chain names no WASI attempt"
  ok "negative-deleted-package: ERR_HYPERUUID_UNAVAILABLE, the chain names the native and WASI attempts"
}

scenario_negative_deleted_wasm() {
  local dir; dir="$(consumer negative-deleted-wasm)"
  install_in "$dir" --omit=optional "$scope@$version"
  rm "$dir/node_modules/$scope-$wasi/$crate.$wasi.wasm"
  smoke "$dir" || die "negative-deleted-wasm: smoke exited non-zero"
  grep -q 'available=false target=none' "$dir/out.txt" || die "negative-deleted-wasm: expected available=false"
  grep -q "cause: .*$crate\.$wasi\.wasm\|cause: .*\.wasm" "$dir/out.txt" || die "negative-deleted-wasm: the cause chain lacks the loader's own error about the .wasm"
  ok "negative-deleted-wasm: ERR_HYPERUUID_UNAVAILABLE with the loader's error in the chain"
}

scenario_mac_lockfile() {
  npm view "$scope-darwin-arm64@$version" version --registry "$registry" > /dev/null 2>&1 \
    || die "mac-lockfile: $scope-darwin-arm64@$version is not in the registry, and the lockfile has to name it"
  local prefix="$here/work/npm-lockfile"
  npm install -g "npm@$LOCKFILE_NPM" --prefix "$prefix" --registry "$registry" > work/npm-lockfile-install.log 2>&1 \
    || { cat work/npm-lockfile-install.log; die "mac-lockfile: could not install npm@$LOCKFILE_NPM"; }
  local lnpm="$prefix/bin/npm"
  [ -e "$lnpm" ] || lnpm="$prefix/npm"
  [ "$("$lnpm" --version)" = "$LOCKFILE_NPM" ] || die "mac-lockfile: the pinned npm is $("$lnpm" --version), not $LOCKFILE_NPM"
  local dir; dir="$(consumer mac-lockfile)"
  (cd "$dir" && "$lnpm" install --cache "$dir/cache" --os=darwin --cpu=arm64 "$scope@$version" > install1.log 2>&1) || { cat "$dir/install1.log"; die "mac-lockfile: first install"; }
  local natives_first; natives_first="$(grep -c "node_modules/$scope-" "$dir/package-lock.json" || true)"
  rm "$dir/package-lock.json"
  (cd "$dir" && "$lnpm" install --cache "$dir/cache" --os=darwin --cpu=arm64 > install2.log 2>&1) || { cat "$dir/install2.log"; die "mac-lockfile: second install"; }
  grep -q "node_modules/$scope-darwin-arm64\"" "$dir/package-lock.json" || die "mac-lockfile: the regenerated lockfile does not name darwin-arm64"
  for suffix in linux-x64-gnu linux-arm64-gnu linux-x64-musl linux-arm64-musl darwin-x64 win32-x64-msvc win32-arm64-msvc; do
    if grep -q "node_modules/$scope-$suffix\"" "$dir/package-lock.json"; then
      die "mac-lockfile: npm $LOCKFILE_NPM repaired the lockfile (it names $suffix); npm/cli#4828 no longer reproduces with the floor's npm, see README.md"
    fi
  done
  echo "  lockfile: the first install named $natives_first @skunkwerkx packages, the regenerated one names darwin-arm64 and the WASI package only (npm $LOCKFILE_NPM reproduces npm/cli#4828)"
  rm -rf "$dir/node_modules"
  # A Linux x64 consumer: what CI's leg is, and what lets a Mac run the scenario without its
  # own native getting installed.
  (cd "$dir" && "$lnpm" ci --cache "$dir/cache" --os=linux --cpu=x64 > ci.log 2>&1) || { cat "$dir/ci.log"; die "mac-lockfile: npm ci"; }
  [ -z "$(platform_packages "$dir")" ] || die "mac-lockfile: npm ci installed a platform package: $(platform_packages "$dir")"
  smoke "$dir" || die "mac-lockfile: smoke failed"
  expect_target "$dir" "$wasi" mac-lockfile
  expect_stderr_experimental_only "$dir" mac-lockfile
  ok "mac-lockfile: npm ci from the damaged lockfile installs no native and $wasi loads"
}

container_run() { # container_run <scenario> <image> [docker run flags...]
  local scenario="$1" image="$2"; shift 2
  local ct="$here/work/ct"
  mkdir -p "$ct"
  cp "$here/container/consumer.sh" "$here/smoke.cjs" "$ct/"
  echo "  docker: $image ($scenario)"
  docker run --rm "$@" "${docker_net[@]}" --tmpfs /work:exec \
    -v "$ct:/ct:ro" -v "$corpus:/corpus:ro" \
    -e "REGISTRY=$container_registry" -e "VERSION=$version" -e "SCENARIO=$scenario" -e HYPERUUID_CORPUS=/corpus \
    "$image" sh /ct/consumer.sh > "$here/work/container-$scenario-${image//[:\/]/_}.log" 2>&1 \
    || { cat "$here/work/container-$scenario-${image//[:\/]/_}.log"; return 1; }
  sed 's/^/    /' "$here/work/container-$scenario-${image//[:\/]/_}.log" | grep -E 'target=|smoke exit=|^    v[0-9]|^    [0-9]+\.' || true
}

scenario_floor() {
  command -v docker > /dev/null || die "floor: docker is not available"
  container_run floor "${FLOOR_IMAGE:-node:22.13.0-alpine}" || die "floor: the exact floor image failed"
  ok "floor: ${FLOOR_IMAGE:-node:22.13.0-alpine} with engine-strict installs and loads $wasi"
  for image in node:22-alpine node:24-alpine; do
    container_run floor "$image" || die "floor: $image failed"
    ok "floor: $image installs and loads $wasi"
  done
}

scenario_s390x() {
  command -v docker > /dev/null || die "s390x: docker is not available"
  container_run s390x node:24-alpine --platform linux/s390x || die "s390x: failed"
  local status
  status="$(sed -n 's/^smoke exit=//p' "$here/work/container-s390x-node_24-alpine.log")"
  # Killed by the bound: busybox's timeout (Alpine) reports the child's SIGTERM as 143, GNU's
  # would report 124.
  case "$status" in
    124|143) ok "s390x: emulated linux/s390x installs plainly, loads $wasi and passes the smoke's checks; the process then does not exit on its own (killed by the bound, status $status: the documented big-endian loss)" ;;
    0) die "s390x: the smoke exited on its own: the documented big-endian exit hang is gone upstream; update typescript/README.md, docs/platforms.md and this scenario" ;;
    *) die "s390x: the smoke exited $status, neither killed by the bound (124 or 143) nor 0" ;;
  esac
}

run_bundle() { # run_bundle <bundle> <label> [external]: deploys the bundle to a fresh directory outside the repository and runs it
  local bundle="$1" label="$2" form="${3:-plain}"
  local deploy; deploy="$(mktemp -d)"
  cp "$bundle" "$deploy/bundle.cjs"
  if [ "$form" = external ]; then
    printf '{"name":"consumer","private":true,"dependencies":{"%s":"%s"}}\n' "$scope" "$version" > "$deploy/package.json"
    printf 'registry=%s\naudit=false\nfund=false\nupdate-notifier=false\n' "$registry" > "$deploy/.npmrc"
    (cd "$deploy" && npm install --omit=dev --cache "$deploy/cache" > install.log 2>&1) || { cat "$deploy/install.log"; return 1; }
  fi
  local status=0
  (cd "$deploy" && env -u NODE_PATH HYPERUUID_CORPUS="$corpus_env" NAPI_RS_FORCE_WASI=error node ./bundle.cjs > out.txt 2> err.txt) || status=$?
  sed "s/^/  $label $form out: /" "$deploy/out.txt"
  sed "s/^/  $label $form err: /" "$deploy/err.txt" | head -5
  [ $status -eq 0 ] && grep -q 'available=true' "$deploy/out.txt"
}

scenario_bundlers() {
  local dir; dir="$(consumer bundlers)"
  install_in "$dir" "$scope@$version"
  cp "$here/bundled.cjs" "$dir/bundled.cjs"
  # esbuild
  local passed=""
  if "$here/bundlers/esbuild.sh" "$dir/bundled.cjs" "$dir/esbuild-plain.cjs" > "$dir/esbuild-plain.log" 2>&1 \
      && run_bundle "$dir/esbuild-plain.cjs" esbuild; then
    passed=plain
  else
    echo "  esbuild plain: $(grep -m1 -E 'rror|FAIL' "$dir/esbuild-plain.log" 2>/dev/null || echo 'bundle built, run failed')"
    "$here/bundlers/esbuild.sh" "$dir/bundled.cjs" "$dir/esbuild-external.cjs" external > "$dir/esbuild-external.log" 2>&1 \
      || { cat "$dir/esbuild-external.log"; die "bundlers: esbuild external build failed"; }
    run_bundle "$dir/esbuild-external.cjs" esbuild external && passed=external
  fi
  [ -n "$passed" ] || die "bundlers: esbuild passed in no form"
  echo "bundler: esbuild $passed"
  # webpack
  passed=""
  if (cd "$dir" && BUNDLE_ENTRY="$(native_file "$dir/bundled.cjs")" BUNDLE_OUT="$(native_file "$dir/webpack-plain.cjs")" \
        "$here/node_modules/.bin/webpack" --config "$here/bundlers/webpack.config.cjs" > "$dir/webpack-plain.log" 2>&1) \
      && run_bundle "$dir/webpack-plain.cjs" webpack; then
    passed=plain
  else
    echo "  webpack plain: $(grep -m1 -E "Module not found|rror|FAIL" "$dir/webpack-plain.log" 2>/dev/null | cut -c1-120 || echo 'bundle built, run failed')"
    (cd "$dir" && BUNDLE_ENTRY="$(native_file "$dir/bundled.cjs")" BUNDLE_OUT="$(native_file "$dir/webpack-external.cjs")" BUNDLE_EXTERNAL=1 \
        "$here/node_modules/.bin/webpack" --config "$here/bundlers/webpack.config.cjs" > "$dir/webpack-external.log" 2>&1) \
      || { cat "$dir/webpack-external.log"; die "bundlers: webpack external build failed"; }
    run_bundle "$dir/webpack-external.cjs" webpack external && passed=external
  fi
  [ -n "$passed" ] || die "bundlers: webpack passed in no form"
  echo "bundler: webpack $passed"
  ok "bundlers: both bundlers have a passing form"
}

for s in "${scenarios[@]}"; do
  if [ "$s" = negative-unpublished ]; then
    echo "== $s"
    scenario_negative_unpublished
  fi
done
publish "$wasi_tarball"
for s in "${scenarios[@]}"; do
  [ "$s" = negative-unpublished ] && continue
  echo "== $s"
  case "$s" in
    plain) scenario_plain ;;
    omit-optional) scenario_omit_optional ;;
    absent-native) scenario_absent_native ;;
    forced-wasi) scenario_forced_wasi ;;
    negative-deleted-package) scenario_negative_deleted_package ;;
    negative-deleted-wasm) scenario_negative_deleted_wasm ;;
    mac-lockfile) scenario_mac_lockfile ;;
    floor) scenario_floor ;;
    s390x) scenario_s390x ;;
    bundlers) scenario_bundlers ;;
    *) die "unknown scenario $s" ;;
  esac
done
echo "all requested scenarios passed: ${scenarios[*]}"
