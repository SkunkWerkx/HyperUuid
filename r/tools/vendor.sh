#!/usr/bin/env bash
# Stages what the R package builds and tests from: a copy of the Rust core under
# src/rust/core/, the wrapper crate's src/rust/Cargo.lock derived from the core's, and the
# five binding corpus files under tests/testthat/corpus/.
#   (no argument)  the full run: stage, then vendor the crates.io dependencies into
#                  src/rust/vendor.tar.xz (CRAN builds offline) and regenerate inst/AUTHORS
#   --stage        only copy the core, derive the lock and copy the corpus files
#   --verify       check the staged corpus, core and lock are present
set -euo pipefail

CRATE=hyperuuid

PKG="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO="$(cd "$PKG/.." && pwd)"
CORPUS_FILES=(v5.json sql_order.json sql_order_refused.json timestamp.json inspect.json)

stage() {
  [ -f "$REPO/rust/Cargo.toml" ] || { echo "vendor.sh: no $CRATE core at $REPO/rust" >&2; exit 1; }
  # A tar pipe rather than rsync: Git for Windows' bash (the CRAN and CI Windows shells)
  # ships tar but not rsync. The excludes match any path component, in GNU and BSD tar alike.
  rm -rf "$PKG/src/rust/core"
  mkdir -p "$PKG/src/rust/core"
  (cd "$REPO/rust" && tar -cf - \
    --exclude target --exclude .cargo --exclude browser-test --exclude fuzz \
    --exclude '*.node' --exclude node_modules .) \
    | (cd "$PKG/src/rust/core" && tar -xf -)
  # The wrapper's lock is derived, never committed: start from the core's lock and let
  # cargo prune it to the wrapper's own packages. --workspace touches only the workspace
  # members (the wrapper and the path dependency `core` beneath it), so every crates.io
  # package keeps the version rust/Cargo.lock pins and the vendored crates are exactly the
  # ones the core is built and tested with. Nothing can move the two locks apart, so
  # prepare-release and Dependabot have nothing to update here.
  cp "$REPO/rust/Cargo.lock" "$PKG/src/rust/Cargo.lock"
  cargo update --workspace -q --manifest-path "$PKG/src/rust/Cargo.toml"
  mkdir -p "$PKG/tests/testthat/corpus"
  for f in "${CORPUS_FILES[@]}"; do
    cp "$REPO/corpus/$f" "$PKG/tests/testthat/corpus/$f"
  done
}

# cargo vendor, then a reproducible tarball: sorted names, a fixed mtime and owner, so the
# same Cargo.lock always produces the same bytes. The flags are GNU tar's; macOS's bsdtar
# lacks them, so gtar (brew install gnu-tar) is used when present.
vendor() {
  local tar=tar
  command -v gtar > /dev/null && tar=gtar
  "$tar" --version 2> /dev/null | grep -q 'GNU tar' || {
    echo "vendor.sh: GNU tar is required (on macOS: brew install gnu-tar)" >&2
    exit 1
  }
  cd "$PKG/src/rust"
  rm -rf vendor
  cargo vendor --locked vendor > /dev/null
  "$tar" --sort=name --mtime='2000-01-01 00:00Z' --owner=0 --group=0 --numeric-owner \
    -cJf vendor.tar.xz vendor
  rm -rf vendor
  cd "$PKG"
  Rscript tools/vendor-authors.R
}

case "${1:-}" in
  "")
    stage
    vendor
    ;;
  --stage)
    stage
    ;;
  --verify)
    for f in "${CORPUS_FILES[@]}"; do
      [ -s "$PKG/tests/testthat/corpus/$f" ] || {
        echo "vendor.sh: tests/testthat/corpus/$f is missing or empty; run tools/vendor.sh --stage" >&2
        exit 1
      }
    done
    for f in src/rust/core/Cargo.toml src/rust/Cargo.lock; do
      [ -f "$PKG/$f" ] || {
        echo "vendor.sh: $f is missing; run tools/vendor.sh --stage" >&2
        exit 1
      }
    done
    ;;
  *)
    echo "usage: vendor.sh [--stage | --verify]" >&2
    exit 2
    ;;
esac
