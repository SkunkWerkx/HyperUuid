#!/usr/bin/env bash
# The esbuild half of the bundler scenario: esbuild.sh <entry> <outfile> [external]
# `external` leaves @skunkwerkx/hyperuuid out of the bundle, to be installed beside it.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
entry="$1"
out="$2"
form="${3:-plain}"
args=("$entry" --bundle --platform=node --log-level=warning "--outfile=$out")
if [ "$form" = external ]; then args+=(--external:@skunkwerkx/hyperuuid); fi
"$here/node_modules/.bin/esbuild" "${args[@]}"
