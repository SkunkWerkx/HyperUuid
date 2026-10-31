#!/usr/bin/env bash
# Rebuilds the core as wasm from this checkout, translates it with the pinned wasm2go, builds
# the cgo control from the same core, checks every variant (sanity + the corpus), and writes
# benchmark results to ../results/<label>-*.txt.
#
#   ./run.sh                 label from `go env GOOS`-`go env GOARCH`
#   SPIKE_LABEL=wsl2-i9-11900H ./run.sh
#   HYPERUUID_CGO_ARCHIVE=<path to libhyperuuid.a> ./run.sh   (control archive override; Windows)
#   SPIKE_BROWSER=1 ./run.sh  also stock Go GOOS=js under node, and TinyGo wasm, if present
#   SPIKE_COUNT=6 (default)
set -euo pipefail
cd -P -- "$(dirname -- "$0")"
HARNESS=$PWD
ROOT=$(cd ../../../../.. && pwd)
RESULTS=$(cd .. && pwd)/results
LABEL=${SPIKE_LABEL:-$(go env GOOS)-$(go env GOARCH)}
COUNT=${SPIKE_COUNT:-6}
TAGS=hyperuuid_spike
mkdir -p build

echo "== wasm: the core as in the tree"
(cd "$ROOT/rust" && cargo cdylib --target wasm32-wasip1)
cp "$ROOT/rust/target/wasm32-wasip1/release/hyperuuid.wasm" build/base.wasm

echo "== wasm: the core with its ChaCha20 pool in the guest (entropy-in-guest.patch)"
rm -rf build/guest && mkdir -p build/guest
tar -C "$ROOT" --exclude=rust/target -cf - rust | tar -C build/guest -xf -
(cd build/guest && patch -s -p1 < "$HARNESS/../entropy-in-guest.patch")
(cd build/guest/rust && CARGO_TARGET_DIR="$HARNESS/build/guest-target" cargo cdylib --target wasm32-wasip1)
cp build/guest-target/wasm32-wasip1/release/hyperuuid.wasm build/guest.wasm

echo "== translate (wasm2go pinned in go.mod)"
for pkg in w2g w2g8 w2g20 w2gaes w2gfloor; do
	mkdir -p $pkg/core && go tool wasm2go -pkg core -tags $TAGS -o $pkg/core/core.go build/base.wasm
done
mkdir -p w2gc/core && go tool wasm2go -pkg core -tags $TAGS -o w2gc/core/core.go build/guest.wasm

echo "== cgo control archive"
if [[ -n "${HYPERUUID_CGO_ARCHIVE:-}" ]]; then
	ARCHIVE=$HYPERUUID_CGO_ARCHIVE
else
	HOST=$(rustc -vV | sed -n 's/^host: //p')
	(cd "$ROOT/rust" && cargo staticlib --target "$HOST")
	ARCHIVE="$ROOT/rust/target/$HOST/staticlib/libhyperuuid.a"
fi
echo "   $ARCHIVE"

echo "== correctness"
CGO_ENABLED=1 CGO_LDFLAGS="$ARCHIVE" go test -count=1 -tags $TAGS -run Sanity ./cgostatic
CGO_ENABLED=0 go test -count=1 -tags $TAGS -run 'Sanity|Corpus' -v ./w2g ./w2g8 ./w2g20 ./w2gaes ./w2gc | grep -E 'vectors|^ok|FAIL'

echo "== benchmarks (-count $COUNT) -> $RESULTS/$LABEL-*.txt"
{ go version; uname -a 2>/dev/null || true; } > "$RESULTS/$LABEL-env.txt"
CGO_ENABLED=1 CGO_LDFLAGS="$ARCHIVE" go test -tags $TAGS -run '^$' -bench . -benchmem -count "$COUNT" ./cgostatic > "$RESULTS/$LABEL-cgo.txt"
for pkg in w2g w2g8 w2g20 w2gaes w2gc w2gfloor; do
	CGO_ENABLED=0 go test -tags $TAGS -run '^$' -bench . -benchmem -count "$COUNT" ./$pkg > "$RESULTS/$LABEL-$pkg.txt"
done

if [[ -n "${SPIKE_BROWSER:-}" ]]; then
	if command -v node >/dev/null; then
		echo "== stock Go GOOS=js under node"
		PATH="$(go env GOROOT)/lib/wasm:$PATH" GOOS=js GOARCH=wasm go test -tags $TAGS -run '^$' \
			-bench 'NewV4$|NewV7$|V7Millis$|V7Batch1000$' -count 3 ./w2g ./w2g8 > "$RESULTS/$LABEL-js-node.txt"
	fi
	if command -v tinygo >/dev/null; then
		echo "== TinyGo wasm under node"
		tinygo test -tags $TAGS -target=wasm -run '^$' -bench 'NewV4$|NewV7$|V7Millis$|V7Batch1000$' -count 3 ./w2g \
			> "$RESULTS/$LABEL-tinygo-wasm.txt" 2>&1
	fi
fi

if command -v benchstat >/dev/null; then
	benchstat "$RESULTS/$LABEL-cgo.txt" | grep -E 'Cgo|sec/op'
	for pkg in w2g w2g8 w2g20 w2gaes w2gc w2gfloor; do benchstat "$RESULTS/$LABEL-$pkg.txt" | grep -E 'W2g' | head -6; done
fi
