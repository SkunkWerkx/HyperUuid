import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('release_guard', SCRIPTS / 'release_guard.py')
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class ReleaseGuardTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.git('init', '-q')
        self.git('config', 'commit.gpgsign', 'false')
        self.git('config', 'user.name', 'Test')
        self.git('config', 'user.email', 'test@example.com')
        self.write('rust/Cargo.toml', 'version = "0.9.0"\n')
        self.write('source.txt', 'source\n')
        self.commit('preparation')
        self.a = self.git('rev-parse', 'HEAD')
        self.payload = 'go/staticlib/linux_amd64/libhyperuuid.a'
        self.write(self.payload, 'verified bytes')
        self.commit(f'stage\n\nRelease-Source-SHA: {self.a}\nRelease-CI-Run-ID: 42')
        self.b = self.git('rev-parse', 'HEAD')
        self.run = dict(id=42, head_sha=self.a, head_branch='master',
                        path='.github/workflows/ci.yml', event='workflow_dispatch',
                        status='completed', conclusion='success',
                        repository={'full_name': 'SkunkWerkx/HyperUuid'})

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.root), *args], text=True).strip()

    def write(self, path, value):
        dest = self.root / path
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(value)

    def commit(self, message):
        self.git('add', '.')
        self.git('commit', '-qm', message)

    def gate(self, override=''):
        with patch.object(guard, 'api', return_value=self.run):
            return guard.release_record(self.root, self.b, 'v0.9.0', override,
                                        'SkunkWerkx/HyperUuid')

    def test_positive_parent_and_run(self):
        self.assertEqual(self.gate(), (self.a, '42'))
        self.assertEqual(self.gate('42'), (self.a, '42'))

    def test_reject_incorrect_run(self):
        for key, value in [('id', 43), ('head_sha', 'f' * 40), ('path', '.github/workflows/other.yml'),
                           ('event', 'pull_request'), ('status', 'in_progress'),
                           ('conclusion', 'failure'), ('head_branch', 'other')]:
            with self.subTest(key=key), patch.dict(self.run, {key: value}):
                with self.assertRaises(ValueError):
                    self.gate()
        with self.assertRaises(ValueError):
            self.gate('43')

    def test_reject_wrong_tag(self):
        with patch.object(guard, 'api', return_value=self.run), self.assertRaises(ValueError):
            guard.release_record(self.root, self.b, 'v0.8.0', '', 'SkunkWerkx/HyperUuid')

    def test_reject_unrelated_source_or_workflow_change(self):
        for path in ['source.txt', '.github/workflows/ci.yml',
                     'swift/HyperUuidCore.artifactbundle/include/hyperuuid.h',
                     'go/staticlib/linux_amd64/unexpected.a']:
            with self.subTest(path=path):
                self.git('reset', '--hard', self.b)
                self.write(path, 'unrelated')
                self.git('add', '.')
                self.git('commit', '--amend', '--no-edit', '-q')
                bad = self.git('rev-parse', 'HEAD')
                with patch.object(guard, 'api', return_value=self.run), self.assertRaises(ValueError):
                    guard.release_record(self.root, bad, 'v0.9.0', '', 'SkunkWerkx/HyperUuid')

    def test_reject_nonparent_source(self):
        self.write('another', 'new source')
        self.commit(f'stage\n\nRelease-Source-SHA: {self.a}\nRelease-CI-Run-ID: 42')
        self.b = self.git('rev-parse', 'HEAD')
        with self.assertRaises(ValueError):
            self.gate()

    def test_reject_branch_advancement(self):
        with patch.object(guard, 'git', return_value=self.b), self.assertRaises(ValueError):
            guard.require_master(self.root, self.a)
        with patch.object(guard, 'git', return_value=self.a):
            guard.require_master(self.root, self.a)

    def test_compare_all_artifact_bytes_and_missing_artifact(self):
        artifacts = self.root / 'artifacts'
        paths = {self.payload: artifacts / 'original.a'}
        paths[self.payload].parent.mkdir()
        paths[self.payload].write_text('verified bytes')
        guard.compare_payloads(self.root, paths)
        paths[self.payload].write_text('wrong run bytes')
        with self.assertRaises(ValueError):
            guard.compare_payloads(self.root, paths)
        paths[self.payload].unlink()
        with self.assertRaises((ValueError, FileNotFoundError)):
            guard.compare_payloads(self.root, paths)

    def test_wrong_source_attestation_is_rejected(self):
        def gh(args, **kwargs):
            self.assertIn('--source-digest', args)
            self.assertEqual(args[args.index('--source-digest') + 1], self.a)
            self.assertIn('--repo', args)
            self.assertIn('--signer-repo', args)
            raise subprocess.CalledProcessError(1, args, 'wrong source')
        with patch.object(guard.subprocess, 'run', side_effect=gh), self.assertRaises(subprocess.CalledProcessError):
            guard.verify_attestation(self.root / self.payload, self.a, 'SkunkWerkx/HyperUuid')


class BinaryTests(unittest.TestCase):
    def test_version_and_conversion_return_contract(self):
        # Compile real shared/static fixtures against the current header; returning void or
        # accepting a rejected UUID must fail even when the names and version are right.
        root = SCRIPTS.parents[1]
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            for version, rejected, accepted in [('0x000900', 5, True), ('0x000800', 5, False),
                                                ('0x000900', 0, False)]:
                c = tmp / 'fixture.c'
                c.write_text(f'''#include <stdint.h>
uint32_t hyperuuid_version(void) {{return {version};}}
int32_t uuid_v7_to_sql_order(uint8_t *p) {{if(p[6]>>4 != 7) return {rejected}; p[6]=0; p[7]=0x80; return 0;}}
int32_t uuid_v7_to_rfc_order(uint8_t *p) {{if(p[7]>>4 != 8) return {rejected}; p[6]=0x7c; p[7]=0xc3; return 0;}}
''')
                obj, shared, archive = tmp / 'f.o', tmp / 'f.so', tmp / 'f.a'
                subprocess.run(['cc', '-fPIC', '-c', str(c), '-o', str(obj)], check=True)
                subprocess.run(['cc', '-shared', str(obj), '-o', str(shared)], check=True)
                subprocess.run(['ar', 'rcs', str(archive), str(obj)], check=True)
                for payload in [shared, archive]:
                    with self.subTest(version=version, rejected=rejected, payload=payload.suffix):
                        if accepted:
                            guard.check_runtime(root, payload, '0.9.0')
                        else:
                            with self.assertRaises(subprocess.CalledProcessError):
                                guard.check_runtime(root, payload, '0.9.0')


class DispatchTests(unittest.TestCase):
    def test_dispatch_uses_master_and_checks_this_dispatch_head(self):
        import sys
        with patch.dict(sys.modules, {'release_guard': guard}):
            spec = importlib.util.spec_from_file_location('dispatch_preparation', SCRIPTS / 'dispatch_preparation.py')
            dispatch = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(dispatch)
        sha = 'a' * 40
        run = dict(id=42, head_sha=sha, head_branch='master', path='.github/workflows/ci.yml',
                   event='workflow_dispatch', repository={'full_name': 'SkunkWerkx/HyperUuid'})
        with patch.object(dispatch.subprocess, 'check_output', return_value='42\n') as post, \
             patch.object(dispatch, 'api', return_value=run), \
             patch.object(dispatch.subprocess, 'run') as cancel:
            self.assertEqual(dispatch.dispatch('SkunkWerkx/HyperUuid', sha), '42')
            self.assertIn('ref=master', post.call_args.args[0])
            run['head_sha'] = 'b' * 40
            with self.assertRaises(ValueError):
                dispatch.dispatch('SkunkWerkx/HyperUuid', sha)
            cancel.assert_called_once_with(['gh', 'run', 'cancel', '42', '--repo',
                                            'SkunkWerkx/HyperUuid'], check=True)

    def test_unchanged_versions_capture_source_and_do_not_skip_dispatch(self):
        import re
        import textwrap
        workflow = (SCRIPTS.parents[1] / '.github/workflows/prepare-release.yml').read_text()
        step = workflow.split('      - name: Commit and push if anything actually changed\n')[1]
        script = textwrap.dedent(step.split('        run: |\n')[1].split('\n      # workflow_dispatch')[0])
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            subprocess.run(['git', 'init', '-q', str(root)], check=True)
            files = ['rust/Cargo.toml', 'rust/Cargo.lock', 'csharp/HyperUuid/HyperUuid.csproj',
                     'java/build.gradle.kts', 'ruby/hyperuuid.gemspec', 'ruby/lib/hyperuuid.rb']
            for path in files:
                (root / path).parent.mkdir(parents=True, exist_ok=True)
                (root / path).write_text('0.9.0')
            subprocess.run(['git', '-C', str(root), 'add', '.'], check=True)
            subprocess.run(['git', '-C', str(root), '-c', 'commit.gpgsign=false',
                            '-c', 'user.name=Test', '-c', 'user.email=test@example.com',
                            'commit', '-qm', 'A'], check=True)
            sha = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
            output = root / 'outputs'
            subprocess.run(['bash', '-c', script], cwd=root,
                           env={**os.environ, 'VERSION': '0.9.0', 'GITHUB_OUTPUT': str(output)}, check=True)
            self.assertEqual(output.read_text().strip(), f'source_sha={sha}')
        dispatch_step = workflow.split('      - name: Run ci.yml against the preparation source\n')[1]
        self.assertNotIn('if:', dispatch_step)
        self.assertIn('dispatch_preparation.py', dispatch_step)


class PlatformSymbolsTests(unittest.TestCase):
    def test_missing_and_removed_exports_across_object_formats(self):
        import shutil
        if not shutil.which('clang'):
            self.skipTest('cross-format fixtures require clang')
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            source = tmp / 'fixture.c'
            for target in ('x86_64-unknown-linux-gnu', 'aarch64-apple-darwin',
                           'x86_64-pc-windows-msvc', 'wasm32-wasi'):
                for names in ({'hyperuuid_version', 'uuid_new_v7'},
                              {'hyperuuid_version', 'uuid_new_v7', 'uuid_new_v6'},
                              {'hyperuuid_version'}):
                    source.write_text('\n'.join(f'int {name}(void) {{ return 0; }}' for name in names))
                    obj, archive = tmp / 'fixture.o', tmp / 'fixture.a'
                    subprocess.run(['clang', '-target', target, '-c', str(source), '-o', str(obj)], check=True)
                    subprocess.run(['ar', 'rcs', str(archive), str(obj)], check=True)
                    expected = {'hyperuuid_version', 'uuid_new_v7'}
                    with self.subTest(target=target, names=names):
                        if names == expected:
                            guard.check_symbols(archive, expected)
                        else:
                            with self.assertRaises(ValueError):
                                guard.check_symbols(archive, expected)

class StagingIntegrationTests(unittest.TestCase):
    def test_positive_stages_exact_downloaded_bytes_and_rejects_incomplete_set(self):
        import shutil
        import textwrap
        if not shutil.which('lld-link'):
            self.skipTest('DLL fixture requires LLVM lld-link')
        repository = SCRIPTS.parents[1]
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for name in ('rust/src/ffi.rs', 'rust/Cargo.toml',
                         guard.BUNDLE + '/include/hyperuuid.h'):
                (root / name).parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(repository / name, root / name)
            source = root / 'fixture.c'
            exports = guard.core_exports(root)
            definitions = """#include <stdint.h>
#ifdef _WIN32
#define EXPORT __declspec(dllexport)
#else
#define EXPORT
#endif
EXPORT uint32_t hyperuuid_version(void) {return 0x000900;}
EXPORT int32_t uuid_v7_to_sql_order(uint8_t *p) {if(p[6]>>4 != 7) return 5; p[6]=0; p[7]=0x80; return 0;}
EXPORT int32_t uuid_v7_to_rfc_order(uint8_t *p) {if(p[7]>>4 != 8) return 5; p[6]=0x7c; p[7]=0xc3; return 0;}
"""
            definitions += '\n'.join(f'EXPORT int {n}(void) {{return 0;}}' for n in exports - {
                'hyperuuid_version', 'uuid_v7_to_sql_order', 'uuid_v7_to_rfc_order'})
            source.write_text(definitions)
            obj, shared, archive, dll = [root / name for name in ('f.o', 'f.so', 'f.a', 'f.dll')]
            subprocess.run(['cc', '-fPIC', '-c', str(source), '-o', str(obj)], check=True)
            subprocess.run(['cc', '-shared', str(obj), '-o', str(shared)], check=True)
            subprocess.run(['ar', 'rcs', str(archive), str(obj)], check=True)
            subprocess.run(['clang', '-target', 'x86_64-pc-windows-msvc', '-ffreestanding',
                            '-fuse-ld=lld', '-nostdlib', '-shared', '-Wl,/noentry',
                            str(source), '-o', str(dll)], check=True)
            paths = guard.payload_paths(root / 'artifacts')
            for dest, src in paths.items():
                src.parent.mkdir(parents=True, exist_ok=True)
                if src.suffix in ('.a', '.lib'):
                    shutil.copyfile(archive, src)
                elif src.suffix == '.dll':
                    shutil.copyfile(dll, src)
                elif src.suffix in ('.so', '.dylib'):
                    shutil.copyfile(shared, src)
                else:
                    src.write_text('generated metadata')
            # The original staging check also requires the non-PHP Windows ARM library.
            extra = root / 'artifacts/native/native-win-arm64/hyperuuid.dll'
            extra.parent.mkdir(parents=True)
            shutil.copyfile(dll, extra)
            sha = 'a' * 40
            with patch.object(guard, 'verify_attestation') as verify:
                guard.validate_payloads(root, paths, sha, 'SkunkWerkx/HyperUuid')
                self.assertEqual(verify.call_count, 34)
            workflow = (repository / '.github/workflows/stage-native-binaries.yml').read_text()
            tail = workflow.split('      - name: Verify attestations, then stage into php/, swift/, go/\n')[1]
            script = textwrap.dedent(tail.split('        run: |\n')[1].split('      - name: Commit and push')[0])
            subprocess.run(['git', 'init', '-q', str(root)], check=True)
            gh = root / 'bin/gh'
            gh.parent.mkdir()
            gh.write_text('#!/bin/sh\ncase "$*" in *"--source-digest ' + sha + '"*) test -f "$3";; *) exit 1;; esac\n')
            gh.chmod(0o755)
            result = subprocess.run(['bash', '-c', script], cwd=root, env={**os.environ,
                'PATH': f'{gh.parent}:{os.environ["PATH"]}', 'SOURCE_SHA': sha,
                'GITHUB_REPOSITORY': 'SkunkWerkx/HyperUuid', 'GITHUB_STEP_SUMMARY': str(root / 'summary')},
                text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            self.assertEqual(result.returncode, 0, result.stderr)
            guard.compare_payloads(root, paths)
            self.assertEqual(len((root / 'digests.txt').read_text().splitlines()), 35)
            missing = paths['go/staticlib/linux_amd64/libhyperuuid.a']
            missing.unlink()
            with patch.object(guard, 'verify_attestation'), self.assertRaisesRegex(ValueError, 'missing artifact'):
                guard.validate_payloads(root, paths, sha, 'SkunkWerkx/HyperUuid')


if __name__ == '__main__':
    unittest.main()
