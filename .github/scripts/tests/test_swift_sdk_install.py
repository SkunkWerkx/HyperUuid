"""swift_sdk_install.sh: a Swift SDK install whose result is the bundle on disk, not the exit
status of `swift sdk install`, which can abort in Foundation's URLSession teardown after the
download, the checksum and the unpack have all completed (swift-corelibs-foundation #5469,
SwiftPM #10535; fixed by swift-corelibs-foundation #5491, not in 6.4.2; seen on test-android
in run 41191035655). Driven here through a fake `swift` on PATH."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / '.github/scripts/swift_sdk_install.sh'
SDK = 'swift-6.4.2-RELEASE_android'
TRIPLE = 'x86_64-unknown-linux-android28'

FAKE_SWIFT = r'''#!/usr/bin/env bash
# $FAKE_STATE/installed lists SDKs; FAKE_INSTALL says what `sdk install` does:
#   ok       install, exit 0
#   crash    install fully, then exit 134 (the Foundation teardown abort)
#   nothing  install nothing, exit 134
#   partial  list the SDK but leave no bundle tree, exit 134
set -eu
case "$1 ${2:-}" in
  "sdk list") cat "$FAKE_STATE/installed" 2>/dev/null; echo "other-sdk" ;;
  "sdk install")
    echo "INSTALL $*" >> "$FAKE_STATE/calls"
    case "$FAKE_INSTALL" in
      ok|crash) echo "$FAKE_SDK" >> "$FAKE_STATE/installed"; mkdir -p "$FAKE_STATE/bundle/swift-android" ;;
      partial) echo "$FAKE_SDK" >> "$FAKE_STATE/installed" ;;
      nothing) ;;
    esac
    [ "$FAKE_INSTALL" = ok ] && exit 0
    echo "Object 0x1 of class _MultiHandle deallocated with non-zero retain count 2" >&2
    exit 134 ;;
  "sdk remove") echo "REMOVE $*" >> "$FAKE_STATE/calls"; : > "$FAKE_STATE/installed"; rm -rf "$FAKE_STATE/bundle" ;;
  "sdk configure") echo "sdkRootPath: $FAKE_STATE/bundle/swift-android/ndk-sysroot" ;;
  *) echo "fake swift: unexpected $*" >&2; exit 99 ;;
esac
'''


class SwiftSdkInstall(unittest.TestCase):
    def run_install(self, behaviour, preinstalled=False):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        state = Path(tmp.name) / 'state'
        state.mkdir()
        if preinstalled:
            (state / 'installed').write_text(SDK + '\n')
            (state / 'bundle/swift-android').mkdir(parents=True)
        fake = Path(tmp.name) / 'bin/swift'
        fake.parent.mkdir()
        fake.write_text(FAKE_SWIFT)
        fake.chmod(0o755)
        env = dict(os.environ, PATH=f'{fake.parent}:{os.environ["PATH"]}', FAKE_STATE=str(state),
                   FAKE_INSTALL=behaviour, FAKE_SDK=SDK)
        result = subprocess.run(['bash', str(SCRIPT), SDK, 'https://example.invalid/sdk.tar.gz', 'abc123', TRIPLE],
                                env=env, capture_output=True, text=True)
        calls = (state / 'calls').read_text() if (state / 'calls').exists() else ''
        return result, calls

    def test_a_clean_install_passes_the_url_and_checksum_once(self):
        result, calls = self.run_install('ok')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls.count('INSTALL'), 1)
        self.assertIn('INSTALL sdk install https://example.invalid/sdk.tar.gz --checksum abc123', calls)

    def test_the_teardown_abort_after_a_complete_install_is_accepted(self):
        result, calls = self.run_install('crash')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls.count('INSTALL'), 1, 'a complete install is not retried')
        self.assertNotIn('REMOVE', calls)
        self.assertIn('exited 134', result.stderr)

    def test_an_install_that_left_nothing_is_tried_once_more_then_fails(self):
        result, calls = self.run_install('nothing')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(calls.count('INSTALL'), 2)
        self.assertIn('did not install', result.stderr)

    def test_a_listed_sdk_without_its_bundle_tree_is_removed_and_retried(self):
        result, calls = self.run_install('partial')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(calls.count('INSTALL'), 2)
        self.assertEqual(calls.count('REMOVE'), 2)

    def test_an_installed_sdk_is_not_installed_again(self):
        result, calls = self.run_install('nothing', preinstalled=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls, '')

    def test_usage(self):
        result = subprocess.run(['bash', str(SCRIPT), SDK], capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)
        self.assertIn('usage', result.stderr)


if __name__ == '__main__':
    unittest.main()
