"""The project half of the release guard: .github/hyper.json and .github/scripts/abi_probe.c.

The generic half (trailers, payload paths, staging, provenance) lives in the Forge
(SkunkWerkx/.github, release_guard.py) and reads this repository's names from hyper.json.
These tests hold the two files the Forge is pointed at: the export rule in hyper.json must
read the same C ABI check_exports.py reads, and the probe must compile against the shipped
header and tell a right payload from a wrong one when linked the way the Forge links it."""
import importlib.util
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
CONFIG = ROOT / '.github/hyper.json'
BUNDLE_INCLUDE = ROOT / 'swift/HyperUuidCore.artifactbundle/include'


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / '.github/scripts' / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class HyperConfigTests(unittest.TestCase):
    def setUp(self):
        self.cfg = json.loads(CONFIG.read_text())

    def test_names_and_files_the_forge_is_pointed_at(self):
        self.assertEqual((self.cfg['crate'], self.cfg['project']), ('hyperuuid', 'HyperUuid'))
        self.assertEqual(self.cfg['cdylib_smoke'], '.github/scripts/cdylib_smoke.py')
        self.assertTrue((ROOT / self.cfg['cdylib_smoke']).is_file())
        self.assertTrue((ROOT / self.cfg.get('abi_probe', '.github/scripts/abi_probe.c')).is_file())
        self.assertTrue((BUNDLE_INCLUDE / 'hyperuuid.h').is_file())

    def test_export_rule_reads_the_same_abi_as_check_exports(self):
        exports = self.cfg['exports']
        names = set()
        for source in exports['sources']:
            text = (ROOT / source).read_text()
            for pattern in exports['patterns']:
                self.assertIsInstance(pattern, str, 'HyperUuid declares its exports one per line')
                names |= set(re.findall(pattern, text))
        names -= set(exports.get('exclude', ()))
        self.assertEqual(names, load('check_exports').core_exports())
        self.assertIn('hyperuuid_version', names)
        self.assertGreater(len(names), 1)
        prefixes = tuple(exports['prefixes'])
        self.assertEqual({n for n in names if not n.startswith(prefixes)}, set(),
                         'every export must carry a prefix the symbol check recognises')


class AbiProbeTests(unittest.TestCase):
    """abi_probe.c compiled and run the way the Forge's check_runtime does it."""
    def setUp(self):
        if not shutil.which('cc'):
            self.skipTest('needs a C compiler')
        self.probe = ROOT / '.github/scripts/abi_probe.c'

    def run_probe(self, payload, version):
        packed = sum(int(n) << shift for n, shift in zip(version.split('.'), (16, 8, 0)))
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / 'abi'
            subprocess.run(['cc', f'-DEXPECTED_VERSION={packed}', '-I', str(BUNDLE_INCLUDE), str(self.probe),
                            str(payload.resolve()), '-ldl', '-lpthread', '-lm', '-o', str(exe)], check=True)
            subprocess.run([str(exe)], check=True)

    def test_version_and_conversion_return_contract(self):
        # Shared and static fixtures with the header's prototypes: a wrong version, a reorder
        # that accepts a v6, or one that refuses a v7 must each fail; the right one passes.
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
                archive.unlink(missing_ok=True)
                subprocess.run(['ar', 'rcs', str(archive), str(obj)], check=True)
                for payload in (shared, archive):
                    with self.subTest(version=version, rejected=rejected, payload=payload.suffix):
                        if accepted:
                            self.run_probe(payload, '0.9.0')
                        else:
                            with self.assertRaises(subprocess.CalledProcessError):
                                self.run_probe(payload, '0.9.0')

    def test_probe_matches_the_shipped_header(self):
        # A declaration the probe uses that the header no longer has, or a changed return type,
        # is a compile error here before the Forge meets it at staging time.
        subprocess.run(['cc', '-fsyntax-only', '-Werror', '-DEXPECTED_VERSION=0', '-I', str(BUNDLE_INCLUDE),
                        str(self.probe)], check=True)
        with tempfile.TemporaryDirectory() as tmp:
            broken = Path(tmp) / 'broken.c'
            broken.write_text(self.probe.read_text().replace('uuid_v7_to_sql_order', 'uuid_v7_to_sql_ordering'))
            with self.assertRaises(subprocess.CalledProcessError):
                subprocess.run(['cc', '-fsyntax-only', '-Werror', '-DEXPECTED_VERSION=0', '-I', str(BUNDLE_INCLUDE),
                                str(broken)], check=True, stderr=subprocess.DEVNULL)


if __name__ == '__main__':
    unittest.main()
