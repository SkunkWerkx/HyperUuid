#!/usr/bin/env bash
# Installs each APK on the running emulator, starts HyperUuid.AndroidSmokeTest's Activity
# and reads what it wrote to logcat (tag HyperUuidSmoke): every door's line, the probe naming
# the core it loaded, and the exit code it ends with.
#
#   android_smoke.sh <application-id> <expected-core-version> <apk>...
#
# A file rather than android-emulator-runner's `script:`, which runs each line as its own
# shell command and so cannot hold a loop.
set -euo pipefail

app="$1"
want="$2"
shift 2

echo "device: API $(adb shell getprop ro.build.version.sdk | tr -d '\r'), page size $(adb shell getconf PAGE_SIZE | tr -d '\r')"

for apk in "$@"; do
  echo "::group::$apk"
  adb uninstall "$app" >/dev/null 2>&1 || true
  adb install -r "$apk"
  adb logcat -c
  adb shell am start -W -n "$app/$app.SmokeActivity"
  log="$(mktemp)"
  for _ in $(seq 1 120); do
    adb logcat -d -s HyperUuidSmoke:V > "$log"
    grep -q "exit code" "$log" && break
    sleep 1
  done
  cat "$log"
  echo "::endgroup::"
  grep -q "native: available=True version=$want" "$log" \
    || { echo "::error::$apk: the probe did not report core $want"; exit 1; }
  grep -q "ALL NATIVE AOT CHECKS PASSED" "$log" \
    || { echo "::error::$apk: the smoke test did not pass"; exit 1; }
  grep -q "exit code 0" "$log" \
    || { echo "::error::$apk: the smoke test did not exit 0"; exit 1; }
  echo "$apk: passed"
done
