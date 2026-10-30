#!/usr/bin/env bash
# Builds the wasm staticlib, publishes this Blazor app once per leg, serves each publish, loads
# it in headless Chromium and requires the page to report PASS. Exits non-zero on anything else.
#   Legs (all four by default, or name some as arguments): net11-release, net11-debug,
#   net10-release, net10-debug. The Debug legs keep the module unstripped
#   (WasmNativeStrip=false), where the SDK skips its post-link wasm-opt: the archive has to be
#   loadable as linked, with nothing rewriting it afterwards.
#   Needs: the .NET SDK global.json pins with the wasm-tools and wasm-tools-net10 workloads
#   (the second relinks the net10.0 legs), and chromium or Chrome
#   (CHROMIUM=/path/to/browser to override); plus stable Rust with rust-src and the
#   wasm32-unknown-emscripten target, and emsdk on PATH, unless STATICLIB names an already-built libhyperuuid.a to use instead — which is how CI
#   runs it, with the archive hyper-build-wasm.yml just built.
set -euo pipefail
legs=("$@")
[ "${#legs[@]}" -gt 0 ] || legs=(net11-release net11-debug net10-release net10-debug)
# Every name is checked before anything is built, so a typo costs nothing.
for leg in "${legs[@]}"; do
  case "$leg" in
    net11-release | net11-debug | net10-release | net10-debug) ;;
    *) echo "unknown leg: $leg (net11-release, net11-debug, net10-release, net10-debug)" >&2; exit 2 ;;
  esac
done
here="$(cd "$(dirname "$0")" && pwd)"
repo="$here/../.."
chromium="${CHROMIUM:-$(command -v chromium-browser || command -v chromium || command -v google-chrome || command -v google-chrome-stable)}"
port="${PORT:-5099}"
out="$(mktemp -d)"
server=""
trap '[ -z "$server" ] || kill "$server" 2>/dev/null || true; rm -rf "$out"' EXIT

staticlib="${STATICLIB:-}"
if [ -z "$staticlib" ]; then
  (cd "$repo/rust" && RUSTC_BOOTSTRAP=1 cargo wasm-staticlib)
  staticlib="$repo/rust/target/wasm32-unknown-emscripten/release/libhyperuuid.a"
fi
dest="$here/../HyperUuid/runtimes/browser-wasm/nativeassets/net11.0"
mkdir -p "$dest"
cp "$staticlib" "$dest/libhyperuuid.a"

# One leg: publish from clean with these arguments, serve, load, require PASS.
run() {
  local leg="$1"
  shift
  echo "== $leg: dotnet publish $*"
  # From clean, every time: the SDK recompiles runtime.c only when runtime.c changes, not when
  # the interp-to-native signature header it includes (wasm_m2n_invoke.g.h) gains an entry, so
  # after a binding adds a P/Invoke with a new signature shape a stale runtime.o makes the
  # first such call abort the runtime at startup (aot-runtime-wasm.c), with an empty page.
  # The legs share bin/ and obj/ as well.
  rm -rf "${here:?}/obj" "${here:?}/bin" "${out:?}/$leg"
  (cd "$here" && dotnet publish "$@" -o "$out/$leg")
  (cd "$out/$leg/wwwroot" && exec python3 -m http.server "$port" >/dev/null 2>&1) &
  server=$!
  sleep 1
  "$chromium" --headless=new --no-sandbox --disable-gpu --virtual-time-budget=60000 \
    --dump-dom "http://localhost:$port/" 2>/dev/null > "$out/$leg.html"
  kill "$server" 2>/dev/null || true
  wait "$server" 2>/dev/null || true
  server=""
  echo "== $leg: results"
  sed -n '/id="results"/,/<\/pre>/p' "$out/$leg.html"
  grep -q '<title>PASS</title>' "$out/$leg.html" || { echo "== $leg: FAIL"; return 1; }
}

for leg in "${legs[@]}"; do
  case "$leg" in
    net11-release) run "$leg" -c Release -f net11.0 ;;
    net11-debug) run "$leg" -c Debug -f net11.0 -p:WasmNativeStrip=false ;;
    net10-release) run "$leg" -c Release -f net10.0 ;;
    net10-debug) run "$leg" -c Debug -f net10.0 -p:WasmNativeStrip=false ;;
  esac
done
