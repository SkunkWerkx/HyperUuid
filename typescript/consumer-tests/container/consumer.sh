#!/bin/sh
# The consumer inside a Node image (Alpine carries no bash, so POSIX sh). Takes REGISTRY,
# VERSION, SCENARIO (omit-optional, floor or s390x) and HYPERUUID_CORPUS; sees this directory
# read-only at /ct, the corpus at /corpus and an empty /work. floor and omit-optional install
# with --omit=optional (the spec's scenario 2 inside the image); s390x installs plainly, since
# the point is that no native package fits there. floor adds engine-strict, so an engines
# mismatch anywhere in the packed graph fails the install. The copy of smoke.cjs makes
# require('@skunkwerkx/hyperuuid') resolve from the install, not from /ct.
set -eu
cd /work
cp /ct/smoke.cjs ./smoke.cjs
printf 'registry=%s\n' "$REGISTRY" > .npmrc
[ "$SCENARIO" = floor ] && printf 'engine-strict=true\n' >> .npmrc
case "$SCENARIO" in s390x) omit="" ;; *) omit="--omit=optional" ;; esac
node --version
npm --version
# shellcheck disable=SC2086 # $omit is empty or one flag
npm install $omit "@skunkwerkx/hyperuuid@$VERSION" --cache /work/cache --no-audit --no-fund
# No pipe into tee: under sh it would hide Node's exit status and let the grep pass over a
# failed assertion. s390x is a big-endian host, where the fallback's runtime misreads its own
# reference counts (host-endian typed arrays over little-endian wasm memory; the consumer
# tests' README has the trace), so the smoke's checks pass and the process then never exits:
# that scenario bounds the run and reports the exit status for run.sh to assert instead.
case "$SCENARIO" in
  s390x)
    set +e
    timeout 300 node ./smoke.cjs > result.txt 2>&1
    status=$?
    set -e
    cat result.txt
    echo "smoke exit=$status"
    grep -q 'checks: .* OK' result.txt
    ;;
  *)
    node ./smoke.cjs > result.txt 2>&1 || { cat result.txt; exit 1; }
    cat result.txt
    ;;
esac
grep -q 'target=wasm32-wasip1' result.txt
