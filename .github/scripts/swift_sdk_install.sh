#!/usr/bin/env bash
# Installs a Swift SDK bundle from swift.org, judged by what is on disk afterwards rather than
# by the exit status of `swift sdk install`: on Linux that command can abort in Foundation's
# URLSession teardown after the download, the checksum and the unpack have all completed
# (swift-corelibs-foundation #5469, SwiftPM #10535; fixed by swift-corelibs-foundation #5491,
# not in 6.4.2; seen on test-android in HyperUuid run 41191035655, where the log said
# "successfully installed" one line above the abort). The checksum is still the install's own:
# it is verified before the unpack, and a bundle that fails it is never written.
#
#   swift_sdk_install.sh <sdk-name> <url> <checksum> <triple>
#
# <sdk-name> is what `swift sdk list` prints for the bundle (the release tag plus its platform,
# e.g. swift-6.4.2-RELEASE_android); <triple> is one target the bundle serves, used to locate it.
# An SDK that is already listed with its tree in place is left alone. Otherwise: install; if the
# SDK is listed and its tree exists, done, whatever the exit status; if not, remove whatever was
# left and try once more; then fail.
set -euo pipefail

[ $# -eq 4 ] || { echo 'usage: swift_sdk_install.sh <sdk-name> <url> <checksum> <triple>' >&2; exit 2; }
sdk=$1
url=$2
checksum=$3
triple=$4

bundle_dir() {
  swift sdk configure --show-configuration "$sdk" "$triple" 2>/dev/null \
    | sed -n 's|^sdkRootPath: \(.*\)/swift-android/ndk-sysroot$|\1|p'
}

installed() {
  local listed bundle
  # Captured, not piped into grep -q: with pipefail a reader that stops early can hand the
  # writer a SIGPIPE and turn a present SDK into a missing one.
  listed="$(swift sdk list)"
  grep -qx "$sdk" <<<"$listed" || return 1
  bundle="$(bundle_dir)"
  [ -n "$bundle" ] && [ -d "$bundle/swift-android" ]
}

if installed; then
  echo "$sdk is installed"
  exit 0
fi
for attempt in 1 2; do
  status=0
  swift sdk install "$url" --checksum "$checksum" || status=$?
  if [ "$status" -ne 0 ]; then
    echo "warning: swift sdk install exited $status (attempt $attempt); checking the bundle on disk" >&2
  fi
  if installed; then
    echo "$sdk is installed"
    exit 0
  fi
  echo "warning: $sdk is not on disk after attempt $attempt; removing what is left" >&2
  swift sdk remove "$sdk" >/dev/null 2>&1 || true
done
echo "error: $sdk did not install" >&2
exit 1
