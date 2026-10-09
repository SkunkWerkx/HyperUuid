#!/usr/bin/env bash
# Builds the wasm staticlib, publishes this Blazor app, serves it, loads it in headless
# Chromium and requires the page to report PASS. Exits non-zero on anything else.
#   Needs: the .NET wasm-tools workload and chromium or Chrome (CHROMIUM=/path/to/browser to
#   override); plus rustup target wasm32-unknown-emscripten and emsdk on PATH, unless
#   STATICLIB names an already-built libhyperuuid.a to use instead — which is how CI runs it,
#   with the archive hyper-build-wasm.yml just built.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$here/../.."
chromium="${CHROMIUM:-$(command -v chromium-browser || command -v chromium || command -v google-chrome || command -v google-chrome-stable)}"
port="${PORT:-5099}"
out="$(mktemp -d)"
server=""
trap '[ -z "$server" ] || kill "$server" 2>/dev/null || true; rm -rf "$out"' EXIT

staticlib="${STATICLIB:-}"
if [ -z "$staticlib" ]; then
  (cd "$repo/rust" && cargo wasm-staticlib)
  staticlib="$repo/rust/target/wasm32-unknown-emscripten/release/libhyperuuid.a"
fi
dest="$here/../HyperUuid/runtimes/browser-wasm/nativeassets/net10.0"
mkdir -p "$dest"
cp "$staticlib" "$dest/libhyperuuid.a"

# From clean, every time: the SDK recompiles runtime.c only when runtime.c changes, not when
# the interp-to-native signature header it includes (wasm_m2n_invoke.g.h) gains an entry, so
# after a binding adds a P/Invoke with a new signature shape a stale runtime.o makes the
# first such call abort the runtime at startup (aot-runtime-wasm.c), with an empty page.
rm -rf "$here/obj" "$here/bin"
(cd "$here" && dotnet publish -c Release -o "$out/app")
(cd "$out/app/wwwroot" && exec python3 -m http.server "$port" >/dev/null 2>&1) &
server=$!
sleep 1

"$chromium" --headless=new --no-sandbox --disable-gpu --virtual-time-budget=60000 \
  --dump-dom "http://localhost:$port/" 2>/dev/null > "$out/dom.html"
sed -n '/id="results"/,/<\/pre>/p' "$out/dom.html"
grep -q '<title>PASS</title>' "$out/dom.html"
