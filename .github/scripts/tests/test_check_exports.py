"""The TypeScript, R and Java sites of check_exports.py: the spelling map, the catch_unwind
rule, the R header, the JNI sites and the JNI no-panic attribute rule.

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


class JavaSiteTests(unittest.TestCase):
    def setUp(self):
        self.mod = load('check_exports')

    def test_java_sites_are_the_ffm_tree_and_the_four_jni_sites(self):
        sites = {s[0]: s for s in self.mod.SITES if s[0].startswith('Java')}
        self.assertEqual(
            list(sites),
            ['Java (FFM)', 'Java (JNI natives)', 'Java (JNI table)', 'Java (JNI no-panic)', 'Java (JNI imports)'],
        )
        base = 'java/src/main/java/io/github/skunkwerkx/hyperuuid/NativeCore.java'
        ffm = 'java/src/main/java25/io/github/skunkwerkx/hyperuuid/NativeCore.java'
        self.assertEqual([g for g, _ in sites['Java (FFM)'][1]], [ffm])
        self.assertEqual([g for g, _ in sites['Java (JNI natives)'][1]], [base])
        self.assertEqual([g for g, _ in sites['Java (JNI table)'][1]], ['java/jni/src/lib.rs'])
        self.assertEqual([g for g, _ in sites['Java (JNI no-panic)'][1]], ['java/jni/examples/no_panic.rs'])
        self.assertEqual([g for g, _ in sites['Java (JNI imports)'][1]], ['java/jni/src/lib.rs'])
        self.assertIs(sites['Java (JNI natives)'][2], self.mod.camel)
        self.assertIs(sites['Java (JNI table)'][2], self.mod.camel)
        self.assertEqual(sites['Java (JNI natives)'][3], ('jniNatives',))
        self.assertEqual(sites['Java (JNI table)'][3], ('jniNatives',))
        self.assertEqual(sites['Java (JNI no-panic)'][2:], ('all', ('jni_natives', 'JNI_OnLoad')))
        self.assertEqual(sites['Java (JNI imports)'][2:], ('all', ()))
        self.assertEqual(self.mod.UNIQUE_SITES, ('Java (JNI table)',))

    def test_jni_table_site_rejects_a_repeated_name(self):
        import contextlib
        import io
        import shutil
        mod = self.mod
        table = next(s for s in mod.SITES if s[0] == 'Java (JNI table)')
        exports = mod.core_exports()
        with tempfile.TemporaryDirectory() as tmp:
            lib = Path(tmp) / 'java/jni/src/lib.rs'
            lib.parent.mkdir(parents=True)
            shutil.copy(ROOT / 'java/jni/src/lib.rs', lib)
            text = lib.read_text(encoding='utf-8')
            line = '    native!("uuidNewV4", "([J)I", native_uuid_new_v4),\n'
            self.assertIn(line, text)
            lib.write_text(text.replace(line, line + line), encoding='utf-8')
            saved = (mod.ROOT, mod.SITES, mod.core_exports, mod.napi_catch_unwind)
            self.addCleanup(lambda: (setattr(mod, 'ROOT', saved[0]), setattr(mod, 'SITES', saved[1]),
                                     setattr(mod, 'core_exports', saved[2]),
                                     setattr(mod, 'napi_catch_unwind', saved[3])))
            mod.ROOT, mod.SITES = Path(tmp), [table]
            mod.core_exports = lambda: exports
            mod.napi_catch_unwind = lambda path: None
            out = io.StringIO()
            with contextlib.redirect_stdout(out), self.assertRaises(SystemExit) as caught:
                mod.main()
        self.assertEqual(caught.exception.code, 1)
        self.assertIn('repeats uuidNewV4', out.getvalue())

    def test_jni_import_site_ignores_native_fns(self):
        import re
        pattern = next(p for b, s, _, _ in self.mod.SITES if b == 'Java (JNI imports)' for _, p in s)
        self.assertEqual(re.findall(pattern, 'pub unsafe extern "system" fn native_uuid_new_v4('), [])
        self.assertEqual(re.findall(pattern, '    fn uuid_new_v4(out_ptr: *mut u8) -> i32;'), ['uuid_new_v4'])

    def test_jni_no_panic_requires_the_attribute_on_every_jni_fn(self):
        attr = '#[cfg_attr(feature = "no-panic", no_panic::no_panic)]\n'
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'lib.rs'
            path.write_text('#[unsafe(no_mangle)]\n' + attr + 'pub unsafe extern "system" fn JNI_OnLoad() {}\n'
                            + attr + 'pub unsafe extern "system" fn native_uuid_new_v4() {}\n', encoding='utf-8')
            self.mod.jni_no_panic(path)
            path.write_text(attr + 'pub unsafe extern "system" fn native_uuid_new_v4() {}\n'
                            '#[unsafe(no_mangle)]\npub unsafe extern "system" fn JNI_OnLoad() {}\n', encoding='utf-8')
            with self.assertRaisesRegex(SystemExit, 'JNI_OnLoad'):
                self.mod.jni_no_panic(path)

    def test_jni_no_panic_holds_on_the_shim(self):
        self.mod.jni_no_panic('java/jni/src/lib.rs')


if __name__ == '__main__':
    unittest.main()
