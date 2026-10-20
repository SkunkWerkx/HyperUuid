"""The v0.9 release's A (build source) -> B (staged bytes) checks. No rebuilding.

Only the generated paths already staged by stage-native-binaries.yml may change.
Build provenance names A; packages assembled by release.yml name B.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

PHP = {rid: ('hyperuuid.dll' if rid.startswith('win') else
             'libhyperuuid.dylib' if rid.startswith('osx') else 'libhyperuuid.so')
       for rid in ('linux-x64', 'linux-arm64', 'linux-musl-x64', 'linux-musl-arm64',
                   'osx-x64', 'osx-arm64', 'win-x64')}
BUNDLE = 'swift/HyperUuidCore.artifactbundle'
APPLE = 'swift/HyperUuidCoreApple.xcframework'
GO_TARGETS = ('android_amd64 android_arm64 darwin_amd64 darwin_arm64 ios_arm64 '
              'iossimulator_arm64 linux_amd64 linux_arm64 maccatalyst_amd64 '
              'maccatalyst_arm64 wasm windows_amd64 windows_arm64').split()
SWIFT_TARGETS = ('aarch64-swift-linux-musl aarch64-unknown-linux-android '
                 'aarch64-unknown-linux-gnu aarch64-unknown-windows-msvc arm64-apple-macosx '
                 'wasm32-unknown-wasip1 x86_64-apple-macosx x86_64-swift-linux-musl '
                 'x86_64-unknown-linux-android x86_64-unknown-linux-gnu '
                 'x86_64-unknown-windows-msvc').split()
SLICES = ('ios-arm64', 'ios-arm64-simulator', 'ios-arm64-maccatalyst')


def command(args, cwd=None):
    return subprocess.check_output(args, cwd=cwd, text=True).strip()


def git(root, *args):
    return command(['git', *args], cwd=root)


def api(repo, endpoint):
    return json.loads(command(['gh', 'api', f'repos/{repo}/{endpoint}']))


def payload_paths(artifacts):
    paths = {f'php/src/native/{rid}/{name}': artifacts / 'native' / f'native-{rid}' / name
             for rid, name in PHP.items()}
    static = []
    for target in GO_TARGETS:
        static.append(f'go/staticlib/{target}/libhyperuuid.a')
    for target in SWIFT_TARGETS:
        name = 'hyperuuid.lib' if 'windows' in target else 'libhyperuuid.a'
        static.append(f'{BUNDLE}/{target}/{name}')
    static += [f'{BUNDLE}/info.json', f'{APPLE}/Info.plist']
    for target in SLICES:
        static.append(f'{APPLE}/{target}/libhyperuuid.a')
        static += [f'{APPLE}/{target}/Headers/HyperUuidCore/{name}'
                   for name in ('hyperuuid.h', 'module.modulemap')]
    paths.update({path: artifacts / 'static-libs' / path for path in static})
    return paths


def require_master(root, sha):
    if git(root, 'rev-parse', 'origin/master') != sha:
        raise ValueError(f'master advanced beyond preparation source {sha}; prepare again')


def validate_run(run, sha, run_id, repo, completed=True):
    expected = {'id': int(run_id), 'head_sha': sha, 'head_branch': 'master',
                'path': '.github/workflows/ci.yml'}
    if completed:
        expected.update(status='completed', conclusion='success')
    for key, value in expected.items():
        if run.get(key) != value:
            raise ValueError(f'CI run {run_id}: expected {key}={value}, got {run.get(key)}')
    if run.get('event') not in ('workflow_dispatch', 'schedule'):
        raise ValueError('CI run must use the full release-mode matrix')
    if run.get('repository', {}).get('full_name', '').lower() != repo.lower():
        raise ValueError('CI run belongs to a different repository')


def version_at(root, sha):
    return re.search(r'^version = "([^"]+)"', git(root, 'show', f'{sha}:rust/Cargo.toml'), re.M)[1]


def release_record(root, commit, tag, override, repo):
    message = git(root, 'show', '-s', '--format=%B', commit)
    def trailer(key, pattern):
        values = re.findall(rf'^{key}: ({pattern})$', message, re.M)
        if len(values) != 1:
            raise ValueError(f'staging commit requires exactly one {key} trailer')
        return values[0]
    sha = trailer('Release-Source-SHA', '[0-9a-f]{40}')
    run_id = trailer('Release-CI-Run-ID', '[1-9][0-9]*')
    if git(root, 'show', '-s', '--format=%P', commit) != sha:
        raise ValueError('staging commit B must have exactly one parent, preparation source A')
    if override and override != run_id:
        raise ValueError('ci_run_id must equal the run recorded by the staging commit')
    want = version_at(root, commit)
    if tag and tag != f'v{want}':
        raise ValueError(f'tag {tag} does not match version {want}')
    if version_at(root, sha) != want:
        raise ValueError('preparation and staging versions differ')
    allowed = payload_paths(Path('artifacts'))
    changed = git(root, 'diff', '--name-only', '--no-renames', sha, commit).splitlines()
    for path in changed:
        if path not in allowed:
            raise ValueError(f'unrelated A -> B change: {path}')
        entry = git(root, 'ls-tree', commit, '--', path).split()
        if not entry:
            raise ValueError(f'staged payload was removed: {path}')
        mode = entry[0]
        expected_mode = '100755' if path.startswith('php/') else '100644'
        if mode != expected_mode:
            raise ValueError(f'invalid staged file mode for {path}: {mode}')
    validate_run(api(repo, f'actions/runs/{run_id}'), sha, run_id, repo)
    return sha, run_id


def verify_attestation(path, sha, repo):
    subprocess.run(['gh', 'attestation', 'verify', str(path), '--repo', repo,
                    '--signer-repo', 'SkunkWerkx/.github', '--source-digest', sha], check=True)


def compare_payloads(root, paths):
    for dest, src in paths.items():
        if src.is_symlink() or (root / dest).is_symlink():
            raise ValueError(f'staged payload must be a regular file: {dest}')
        if src.read_bytes() != (root / dest).read_bytes():
            raise ValueError(f'staged bytes do not match the recorded CI run: {dest}')


def core_exports(root):
    names = set(re.findall(r'pub extern "C" fn (\w+)', (root / 'rust/src/ffi.rs').read_text()))
    if 'hyperuuid_version' not in names or len(names) < 2:
        raise ValueError('could not read the current C export set')
    return names


def check_symbols(path, expected):
    # LLVM understands ELF, Mach-O, COFF and WASM archives. GNU nm cannot read them all.
    if path.suffix == '.dll':
        output = command(['llvm-readobj', '--coff-exports', str(path)])
        symbols = re.findall(r'^  Name: (\S+)$', output, re.M)
    else:
        flags = ['--dynamic'] if path.suffix == '.so' else []
        output = command(['llvm-nm', *flags, '--defined-only', '--extern-only', '--format=posix', str(path)])
        symbols = [line.split()[0] for line in output.splitlines() if len(line.split()) >= 2]
    names = set()
    for symbol in symbols:
        name = symbol.removeprefix('_')  # Mach-O C symbol decoration
        if name.startswith(('uuid_', 'hyperuuid_')):
            names.add(name)
    if names != expected:
        raise ValueError(f'{path}: missing exports {sorted(expected - names)}; stale exports {sorted(names - expected)}')


def check_runtime(root, payload, version):
    # Compile against the current declarations, including int32_t conversion results.
    # Each process loads/links just one payload, avoiding dynamic loader caching.
    packed = sum(int(n) << shift for n, shift in zip(version.split('.'), (16, 8, 0)))
    source = r'''
#include <string.h>
#include "hyperuuid.h"
int main(void) {
    if (hyperuuid_version() != EXPECTED_VERSION) return 1;
    uint8_t v7[16] = {0x01,0x7f,0x22,0xe2,0x79,0xb0,0x7c,0xc3,0x98,0xc4,0xdc,0x0c,0x0c,0x07,0x39,0x8f};
    uint8_t v6[16] = {0x1e,0xc9,0x41,0x4c,0x23,0x2a,0x6b,0x00,0xb3,0xc8,0x9f,0x6b,0xde,0xce,0xd8,0x46};
    uint8_t out[16];
    memcpy(out, v7, 16);
    if (uuid_v7_to_rfc_order(out) != 5 || memcmp(out, v7, 16)) return 2;
    if (uuid_v7_to_sql_order(out) != 0) return 3;
    if ((out[7] >> 4) != 8 || (out[8] >> 6) != 2) return 4;
    if (uuid_v7_to_rfc_order(out) != 0 || memcmp(out, v7, 16)) return 5;
    memcpy(out, v6, 16);
    if (uuid_v7_to_sql_order(out) != 5 || memcmp(out, v6, 16)) return 6;
    return 0;
}
'''
    with tempfile.TemporaryDirectory() as tmp:
        c, exe = Path(tmp) / 'abi.c', Path(tmp) / 'abi'
        c.write_text(source)
        subprocess.run(['cc', f'-DEXPECTED_VERSION={packed}', '-I', str(root / BUNDLE / 'include'),
                        str(c), str(payload.resolve()), '-ldl', '-lpthread', '-lm', '-o', str(exe)], check=True)
        subprocess.run([str(exe)], check=True)


def validate_payloads(root, paths, sha, repo, compare=False, attest=True):
    exports = core_exports(root)
    version = re.search(r'^version = "([^"]+)"', (root / 'rust/Cargo.toml').read_text(), re.M)[1]
    # Check the complete explicit platform set, including generated metadata, before copying.
    for dest, src in paths.items():
        if not src.is_file() or src.is_symlink():
            raise ValueError(f'missing artifact payload: {src}')
        if src.suffix in ('.a', '.lib', '.so', '.dll', '.dylib'):
            if attest:
                verify_attestation(src, sha, repo)
            check_symbols(src, exports)
    for dest in ('php/src/native/linux-x64/libhyperuuid.so',
                 f'{BUNDLE}/x86_64-unknown-linux-gnu/libhyperuuid.a',
                 'go/staticlib/linux_amd64/libhyperuuid.a'):
        check_runtime(root, paths[dest], version)
    if compare:
        compare_payloads(root, paths)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('record', 'run', 'master', 'payloads', 'compare'))
    args = parser.parse_args()
    root = Path.cwd()
    repo, sha = os.environ['GITHUB_REPOSITORY'], os.environ.get('SOURCE_SHA', '')
    run_id = os.environ.get('RUN_ID', '')
    if args.mode == 'record':
        tag = os.environ['GITHUB_REF_NAME'] if os.environ.get('GITHUB_REF_TYPE') == 'tag' else ''
        sha, run_id = release_record(root, os.environ['GITHUB_SHA'], tag,
                                     os.environ.get('CI_RUN_ID_OVERRIDE', ''), repo)
        with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
            output.write(f'source_sha={sha}\nrun_id={run_id}\n')
    elif args.mode == 'run':
        validate_run(api(repo, f'actions/runs/{run_id}'), sha, run_id, repo)
    elif args.mode == 'master':
        require_master(root, sha)
    else:
        # Staging's existing verify-and-copy step checks provenance once, after these
        # ABI checks. At release entry, verify it here while comparing the shipped bytes.
        validate_payloads(root, payload_paths(root / 'artifacts'), sha, repo,
                          compare=args.mode == 'compare', attest=args.mode == 'compare')


if __name__ == '__main__':
    main()
