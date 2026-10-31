#!/usr/bin/env bash
# Stages what the R package builds and tests from: the wrapper crate's src/rust/Cargo.lock
# derived from the core's, the vendored crates in src/rust/vendor.tar.xz (CRAN builds
# offline) with the core itself among them, from this checkout's rust/, and the five binding
# corpus files under tests/testthat/corpus/.
#   (no argument)  the full run: stage, then regenerate inst/AUTHORS from the vendored crates
#   --stage        derive the lock, vendor the crates and the core, copy the corpus files
#   --verify       check the vendor tarball (with the core), the lock and the corpus are present
#
# The wrapper depends on the hyperuuid crate by exact version, as any crates.io consumer
# does, so the dependency graph CRAN builds is the published one. That version is not on
# crates.io while the release that publishes it is being packed, and `cargo vendor` leaves a
# `[patch]`ed crate out of the vendor directory, so: the lock is resolved with the core patched
# in from rust/ on the command line (never in a committed file), the crates.io dependencies are
# vendored, the core is added to the vendor directory as `cargo package` writes it (the form the
# crates.io release of that version is uploaded in), and the lock is rewritten offline against
# the vendored sources, so it names the crates.io source for the core and the tarball builds
# with no patch and no network. A pin that differs from rust/Cargo.toml's version is refused:
# with the patch unused, cargo would quietly vendor the published core instead.
set -euo pipefail

CRATE=hyperuuid

PKG="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO="$(cd "$PKG/.." && pwd)"
CORPUS_FILES=(v5.json sql_order.json sql_order_refused.json timestamp.json inspect.json)
# Cargo reads these relative to the working directory, src/rust.
VENDORED=(--config 'source.crates-io.replace-with="vendored-sources"'
          --config 'source.vendored-sources.directory="vendor"')

core_version() {
  sed -n 's/^version = "\(.*\)"/\1/p' "$REPO/rust/Cargo.toml" | head -1
}

# GNU tar, for a reproducible tarball: sorted names, a fixed mtime and owner, so the same
# sources always produce the same bytes. macOS's bsdtar lacks the flags, so gtar (brew
# install gnu-tar) is used when present.
gnu_tar() {
  local tar=tar
  command -v gtar > /dev/null && tar=gtar
  "$tar" --version 2> /dev/null | grep -q 'GNU tar' || {
    echo "vendor.sh: GNU tar is required (on macOS: brew install gnu-tar)" >&2
    exit 1
  }
  echo "$tar"
}

stage() {
  [ -f "$REPO/rust/Cargo.toml" ] || { echo "vendor.sh: no $CRATE core at $REPO/rust" >&2; exit 1; }
  local tar version pin patch pkgdir
  tar="$(gnu_tar)"
  version="$(core_version)"
  pin="$(sed -n "s/^$CRATE = { version = \"=\([^\"]*\)\".*/\1/p" "$PKG/src/rust/Cargo.toml")"
  [ "$pin" = "$version" ] || {
    echo "vendor.sh: src/rust/Cargo.toml pins $CRATE =${pin:-?} but rust/Cargo.toml is $version; the pin is a version file prepare-release bumps" >&2
    exit 1
  }
  patch="patch.crates-io.$CRATE.path=\"$REPO/rust\""
  cd "$PKG/src/rust"
  rm -rf vendor vendor.tar.xz
  # The wrapper's lock is derived, never committed: start from the core's lock and let
  # cargo prune it to the wrapper's own packages. --workspace touches only the wrapper, so
  # every crates.io package keeps the version rust/Cargo.lock pins and the vendored crates are
  # exactly the ones the core is built and tested with. Nothing can move the two locks apart,
  # so prepare-release and Dependabot have nothing to update here.
  cp "$REPO/rust/Cargo.lock" Cargo.lock
  cargo update --workspace -q --config "$patch"
  cargo vendor -q --locked --config "$patch" vendor
  # The core, as `cargo package` writes it. --no-verify: the verify build is the crate's own
  # CI; --allow-dirty: a working tree with edits is the normal local case.
  pkgdir="$(mktemp -d)"
  cargo package -q --no-verify --allow-dirty --manifest-path "$REPO/rust/Cargo.toml" --target-dir "$pkgdir"
  mkdir -p "vendor/$CRATE"
  tar -xzf "$pkgdir/package/$CRATE-$version.crate" --strip-components=1 -C "vendor/$CRATE"
  rm -rf "$pkgdir"
  # A directory source needs a checksum file per crate; with no files listed nothing is
  # checked, which is what `cargo vendor` itself writes for a git dependency.
  echo '{"files":{},"package":null}' > "vendor/$CRATE/.cargo-checksum.json"
  cargo update --workspace -q --offline "${VENDORED[@]}"
  "$tar" --sort=name --mtime='2000-01-01 00:00Z' --owner=0 --group=0 --numeric-owner \
    -cJf vendor.tar.xz vendor
  # vendor/ stays for authors(); the modes below remove it.
  cd "$PKG"
  mkdir -p tests/testthat/corpus
  for f in "${CORPUS_FILES[@]}"; do
    cp "$REPO/corpus/$f" "tests/testthat/corpus/$f"
  done
}

# inst/AUTHORS from the vendored crates' manifests, read offline from the vendor directory.
authors() {
  cd "$PKG"
  Rscript tools/vendor-authors.R
}

case "${1:-}" in
  "")
    stage
    authors
    rm -rf "$PKG/src/rust/vendor"
    ;;
  --stage)
    stage
    rm -rf "$PKG/src/rust/vendor"
    ;;
  --verify)
    for f in "${CORPUS_FILES[@]}"; do
      [ -s "$PKG/tests/testthat/corpus/$f" ] || {
        echo "vendor.sh: tests/testthat/corpus/$f is missing or empty; run tools/vendor.sh" >&2
        exit 1
      }
    done
    for f in src/rust/vendor.tar.xz src/rust/Cargo.lock; do
      [ -f "$PKG/$f" ] || {
        echo "vendor.sh: $f is missing; run tools/vendor.sh" >&2
        exit 1
      }
    done
    version="$(core_version)"
    tar -xJOf "$PKG/src/rust/vendor.tar.xz" "vendor/$CRATE/Cargo.toml" 2> /dev/null \
      | grep -q "^version = \"$version\"\$" || {
      echo "vendor.sh: src/rust/vendor.tar.xz does not carry $CRATE $version; run tools/vendor.sh" >&2
      exit 1
    }
    ;;
  *)
    echo "usage: vendor.sh [--stage | --verify]" >&2
    exit 2
    ;;
esac
