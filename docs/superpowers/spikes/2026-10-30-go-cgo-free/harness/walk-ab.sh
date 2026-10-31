#!/usr/bin/env bash
# The v7 batch walk, before and after: the core at WALK_BASE (one walk, the native one, on
# every target) against the checkout (rust/src/v7.rs, `fill_batch`: the native walk off
# WebAssembly, the word walk on it), measured every way the core runs:
#
#   native      the core's own Criterion bench, batch_1000/v7_batch. The checkout should
#               match the base here: off WebAssembly it compiles the same walk.
#   translated  the core built for wasm32-wasip1 and translated by wasm2go, its batch with
#               random_get from AES-256-CTR (w2gaes) and from nothing (w2gfloor, the walk alone).
#   wasm        WALK_WASM=1: the raw wasm32-wasip1 module, untranslated, twice: the same
#               Criterion bench run by wasmtime (random_get from the host), and the module's
#               batch export timed in V8 by walkwasm/bench.mjs (random_get from
#               crypto.getRandomValues). Needs wasmtime and node on PATH.
#   browser     WALK_BROWSER=1: the translated batch (w2gaes, w2gfloor) under GOOS=js
#               GOARCH=wasm, run by node through Go's own wasm_exec.
#
# Rounds alternate base and checkout, so drift lands on both. Writes
# ../results/<label>-walk-ab-{native,translated,wasm,browser}.txt.
#
#   ./walk-ab.sh                 label from `go env GOOS`-`go env GOARCH`
#   SPIKE_LABEL=macos-arm64 ./walk-ab.sh
#   WALK_BASE=<rev> (default bb390d5, the last commit with one walk)   WALK_ROUNDS=2 (default)
set -euo pipefail
cd -P -- "$(dirname -- "$0")"
HARNESS=$PWD
ROOT=$(cd ../../../../.. && pwd)
RESULTS=$(cd .. && pwd)/results
LABEL=${SPIKE_LABEL:-$(go env GOOS)-$(go env GOARCH)}
BASE=${WALK_BASE:-bb390d5}
ROUNDS=${WALK_ROUNDS:-2}
TAGS=hyperuuid_spike
NATIVE=$RESULTS/$LABEL-walk-ab-native.txt
TRANSLATED=$RESULTS/$LABEL-walk-ab-translated.txt
WASM=$RESULTS/$LABEL-walk-ab-wasm.txt
BROWSER=$RESULTS/$LABEL-walk-ab-browser.txt
mkdir -p build

echo "== the core at $BASE"
rm -rf build/walk-base && mkdir -p build/walk-base
# Through a scratch index, not `git archive`: .gitattributes marks rust/ export-ignore (the
# Composer package is an archive of the repository), and the repository's own index is left alone.
export GIT_INDEX_FILE="$HARNESS/build/walk-base.index"
git -C "$ROOT" read-tree "$BASE"
git -C "$ROOT" ls-tree -r --name-only "$BASE" -- rust |
	git -C "$ROOT" checkout-index --prefix="$HARNESS/build/walk-base/" --stdin
unset GIT_INDEX_FILE
rm -f build/walk-base.index
src() { if [[ $1 == base ]]; then echo "$HARNESS/build/walk-base/rust"; else echo "$ROOT/rust"; fi; }

echo "== wasm, both sides"
for side in base tree; do
	(cd "$(src $side)" && CARGO_TARGET_DIR="$HARNESS/build/walk-target-$side" cargo cdylib --target wasm32-wasip1 >/dev/null)
	cp "build/walk-target-$side/wasm32-wasip1/release/hyperuuid.wasm" "build/walk-$side.wasm"
done

translate() {
	for pkg in w2gaes w2gfloor; do
		mkdir -p $pkg/core && go tool wasm2go -pkg core -tags $TAGS -o $pkg/core/core.go "build/walk-$1.wasm"
	done
}

echo "== correctness, both sides (the corpus through w2gaes)"
for side in base tree; do
	translate $side
	CGO_ENABLED=0 go test -count=1 -tags $TAGS -run 'Sanity|Corpus' ./w2gaes | sed "s/^/   $side: /"
done

{ go version; rustc -V; uname -a 2>/dev/null || true; echo "base: $BASE ($(git -C "$ROOT" rev-parse --short "$BASE"))"
  echo "tree: $(git -C "$ROOT" rev-parse --short HEAD)$(git -C "$ROOT" diff --quiet -- rust || echo ' + uncommitted rust/')"
  [[ -n ${WALK_WASM:-} ]] && { wasmtime -V; echo "node $(node -v)"; }
  [[ -n ${WALK_BROWSER:-} ]] && echo "browser: GOOS=js GOARCH=wasm under node $(node -v)"; true; } > "$RESULTS/$LABEL-walk-ab-env.txt"
: > "$NATIVE"
: > "$TRANSLATED"
[[ -n ${WALK_WASM:-} ]] && : > "$WASM"
[[ -n ${WALK_BROWSER:-} ]] && : > "$BROWSER"
for round in $(seq "$ROUNDS"); do
	for side in base tree; do
		echo "== round $round, $side"
		line=$(cd "$(src $side)" && CARGO_TARGET_DIR="$HARNESS/build/walk-target-$side" \
			cargo bench --bench uuid_benchmarks -- '^batch_1000/v7_batch$' --noplot 2>/dev/null | grep 'time:')
		echo "$side round $round $line" | tee -a "$NATIVE"
		translate $side
		{ echo "walk: $side"; CGO_ENABLED=0 go test -tags $TAGS -run '^$' -bench 'V7Batch1000$' -count 6 ./w2gaes ./w2gfloor; } >> "$TRANSLATED"
		if [[ -n ${WALK_WASM:-} ]]; then
			# Criterion writes its reports into target/; under wasmtime that fails and it says so
			# on stderr, which is dropped. The measurement is on stdout.
			line=$(cd "$(src $side)" && CARGO_TARGET_DIR="$HARNESS/build/walk-target-$side" \
				CARGO_TARGET_WASM32_WASIP1_RUNNER="wasmtime run" \
				cargo bench --target wasm32-wasip1 --bench uuid_benchmarks -- '^batch_1000/v7_batch$' --noplot 2>/dev/null | grep 'time:')
			echo "$side round $round wasmtime $line" | tee -a "$WASM"
			echo "$side round $round v8 $(node walkwasm/bench.mjs "build/walk-$side.wasm")" | tee -a "$WASM"
		fi
		if [[ -n ${WALK_BROWSER:-} ]]; then
			{ echo "walk: $side"; PATH="$PATH:$(go env GOROOT)/lib/wasm" GOOS=js GOARCH=wasm \
				go test -tags $TAGS -run '^$' -bench 'V7Batch1000$' -count 6 ./w2gaes ./w2gfloor; } >> "$BROWSER"
		fi
	done
done
translate tree

if command -v benchstat >/dev/null; then
	echo "== translated, ns per batch of 1000 (pkg: w2gaes AES-256-CTR, w2gfloor no entropy)"
	benchstat -col walk "$TRANSLATED"
	if [[ -n ${WALK_BROWSER:-} ]]; then
		echo "== browser (GOOS=js), ns per batch of 1000"
		benchstat -col walk "$BROWSER"
	fi
fi
