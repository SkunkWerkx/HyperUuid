"""The TypeScript and R sites of check_exports.py: the spelling map, the catch_unwind rule
and the R header.

Every napi function must carry `catch_unwind`, or a panic in the core aborts the Node
process instead of surfacing as a JavaScript error. The R site is pinned too: its header is
the package's only ABI surface, so a site that reads nothing would hide a drift."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / '.github/scripts' / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class TypeScriptSiteTests(unittest.TestCase):
    def setUp(self):
        self.mod = load('check_exports')
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def source(self, text):
        path = Path(self.tmp.name) / 'lib.rs'
        path.write_text(text, encoding='utf-8')
        return path

    def test_napi_site_requires_catch_unwind(self):
        path = self.source('#[napi]\npub fn new_v4() {}\n')
        with self.assertRaises(SystemExit) as caught:
            self.mod.napi_catch_unwind(path)
        self.assertIn('new_v4', str(caught.exception))
        self.assertIn('lib.rs', str(caught.exception))
        self.mod.napi_catch_unwind(self.source('#[napi(catch_unwind)]\npub fn new_v4() {}\n'))

    def test_napi_catch_unwind_flags_a_function_with_other_options_only(self):
        path = self.source('#[napi(js_name = "x")]\n\npub fn new_v7() {}\n')
        with self.assertRaises(SystemExit):
            self.mod.napi_catch_unwind(path)

    def test_napi_catch_unwind_ignores_consts_and_enums(self):
        self.mod.napi_catch_unwind(
            self.source('#[napi]\npub const MAX: u32 = 1;\n#[napi]\npub enum Kind { A }\n')
        )

    def test_ts_spelling_merges_the_layout_pair(self):
        self.assertIsNone(self.mod.ts_spelling('uuid_v7_unix_millis_in'))
        self.assertEqual(self.mod.ts_spelling('hyperuuid_version'), 'native_version')
        self.assertEqual(self.mod.ts_spelling('uuid_new_v4'), 'new_v4')

    def test_camel(self):
        self.assertEqual(self.mod.camel('v7_to_sql_order'), 'v7ToSqlOrder')
        self.assertEqual(self.mod.camel('native_version'), 'nativeVersion')

    def test_napi_site_regex_allows_a_lifetime(self):
        import re
        pattern = next(p for b, s, _, _ in self.mod.SITES if b == 'TypeScript (napi)' for _, p in s)
        text = "#[napi(catch_unwind)]\npub fn v7_timestamp<'env>(a: u8) {}\n#[napi(catch_unwind)]\npub fn new_v4() {}\n"
        self.assertEqual(re.findall(pattern, text), ['v7_timestamp', 'new_v4'])


class RSiteTests(unittest.TestCase):
    def test_r_header_site_reads_every_core_export(self):
        import re
        mod = load('check_exports')
        site = next(s for s in mod.SITES if s[0] == 'R (core.h)')
        self.assertEqual(site[1:], ([('r/src/core.h', mod.C_PROTOTYPE)], 'all', ()))
        text = (ROOT / 'r/src/core.h').read_text(encoding='utf-8')
        self.assertEqual(set(re.findall(mod.C_PROTOTYPE, text)), mod.core_exports())


if __name__ == '__main__':
    unittest.main()
