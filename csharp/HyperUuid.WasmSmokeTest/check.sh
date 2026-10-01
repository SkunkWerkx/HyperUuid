#!/usr/bin/env bash
# Builds the wasm staticlib, publishes this Blazor app, serves it, loads it in headless
# Chromium and requires the page to report PASS. Exits non-zero on anything else.
#   Needs: rustup target wasm32-unknown-emscripten, emsdk on PATH, the .NET wasm-tools
#   workload, and chromium (CHROMIUM=/path/to/browser to override).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$here/../.."
chromium="${CHROMIUM:-$(command -v chromium-browser || command -v chromium || command -v google-chrome)}"
port="${PORT:-5099}"
out="$(mktemp -d)"
server=""
trap '[ -z "$server" ] || kill "$server" 2>/dev/null || true; rm -rf "$out"' EXIT

(cd "$repo/rust" && cargo wasm-staticlib)
dest="$here/../HyperUuid/runtimes/browser-wasm/nativeassets/net10.0"
mkdir -p "$dest"
cp "$repo/rust/target/wasm32-unknown-emscripten/release/libhyperuuid.a" "$dest/"

(cd "$here" && dotnet publish -c Release -o "$out/app")
(cd "$out/app/wwwroot" && exec python3 -m http.server "$port" >/dev/null 2>&1) &
server=$!
sleep 1

"$chromium" --headless=new --no-sandbox --disable-gpu --virtual-time-budget=60000 \
  --dump-dom "http://localhost:$port/" 2>/dev/null > "$out/dom.html"
sed -n '/id="results"/,/<\/pre>/p' "$out/dom.html"
grep -q '<title>PASS</title>' "$out/dom.html"
