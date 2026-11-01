"""The PR board's classifier: which classes a pull request's files touch, and so which jobs run.

The rules are the design spec's table (docs/superpowers/specs/2026-10-31-pr-board-design.md,
"Change detection"): `ci` first, then docs and dotfiles, the core, one class per binding, and
`everything` for a path nothing names. Fail-safe: an unclassified path, an empty list, or a
list at the API's cap runs the whole board."""
import importlib.util
import io
from pathlib import Path
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
KEYS = ['core', 'ci', 'docs_only', 'csharp', 'java', 'python', 'ruby', 'php', 'swift', 'typescript', 'r', 'go']
BINDINGS = KEYS[3:]


def load():
    spec = importlib.util.spec_from_file_location('pr_changes', ROOT / '.github/scripts/pr_changes.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


pr_changes = load()


def classify(*paths):
    return pr_changes.classify([(p, None) if isinstance(p, str) else p for p in paths])


class ClassifyTests(unittest.TestCase):
    def assertEverything(self, result):
        self.assertEqual(result, {**{key: True for key in KEYS}, 'docs_only': False})

    def assertOnly(self, result, *on):
        self.assertEqual({key for key, value in result.items() if value}, set(on))

    def test_keys(self):
        self.assertEqual(list(classify('README.md')), KEYS)

    def test_root_readme_is_docs_only(self):
        result = classify('README.md')
        self.assertTrue(result['docs_only'])
        self.assertFalse(result['core'])
        self.assertOnly(result, 'docs_only')

    def test_nested_readme_is_docs_not_its_binding(self):
        result = classify('typescript/README.md')
        self.assertFalse(result['typescript'])
        self.assertTrue(result['docs_only'])

    def test_nested_readme_plus_source_is_the_binding(self):
        result = classify('typescript/README.md', 'typescript/src/index.ts')
        self.assertTrue(result['typescript'])
        self.assertFalse(result['docs_only'])
        self.assertOnly(result, 'typescript')

    def test_releasing_md_is_ci(self):
        result = classify('.github/RELEASING.md')
        self.assertTrue(result['ci'])
        self.assertFalse(result['docs_only'])
        for binding in BINDINGS:
            self.assertTrue(result[binding], binding)

    def test_workflow_and_shape_test_are_ci(self):
        for path in ('.github/workflows/ci.yml', '.github/scripts/tests/test_workflow_contracts.py', '.github/hyper.json'):
            with self.subTest(path=path):
                result = classify(path)
                self.assertTrue(result['ci'])
                self.assertTrue(result['core'])
                self.assertFalse(result['docs_only'])

    def test_rename_out_of_rust_is_core(self):
        result = classify(('docs/x.md', 'rust/src/x.rs'))
        self.assertTrue(result['core'])
        self.assertFalse(result['docs_only'])

    def test_rename_into_rust_is_core(self):
        result = classify(('rust/src/x.rs', 'docs/x.md'))
        self.assertTrue(result['core'])
        self.assertFalse(result['docs_only'])

    def test_unclassified_path_is_everything(self):
        self.assertEverything(classify('Makefile'))
        self.assertEverything(classify('README.md', 'tools/new.sh'))

    def test_empty_list_is_everything(self):
        self.assertEverything(classify())

    def test_capped_list_is_everything(self):
        self.assertEverything(pr_changes.classify([('README.md', None)] * 3000))
        self.assertOnly(pr_changes.classify([('README.md', None)] * 2999), 'docs_only')

    def test_one_binding(self):
        for binding, path in (('csharp', 'csharp/HyperUuid/Uuid.cs'), ('java', 'java/build.gradle.kts'),
                              ('python', 'python/tests/test_v7.py'), ('ruby', 'ruby/Rakefile'),
                              ('php', 'php/tests/HyperUuidTest.php'), ('swift', 'swift/Sources/HyperUuid/Uuid.swift'),
                              ('typescript', 'typescript/package.json'), ('r', 'r/DESCRIPTION'), ('go', 'go/uuid.go')):
            with self.subTest(binding=binding):
                self.assertOnly(classify(path), binding)

    def test_core_implies_every_binding(self):
        for path in ('rust/src/lib.rs', 'include/hyperuuid.hpp', 'c/smoke/run.sh', 'corpus/v7.json'):
            with self.subTest(path=path):
                result = classify(path)
                self.assertTrue(result['core'])
                self.assertFalse(result['ci'])
                self.assertFalse(result['docs_only'])
                for binding in BINDINGS:
                    self.assertTrue(result[binding], binding)

    def test_global_json_is_csharp(self):
        self.assertOnly(classify('global.json'), 'csharp')

    def test_package_swift_is_swift(self):
        self.assertOnly(classify('Package.swift'), 'swift')

    def test_composer_json_is_php(self):
        self.assertOnly(classify('composer.json'), 'php')

    def test_dotfiles_are_docs(self):
        for path in ('.editorconfig', '.gitattributes', '.gitignore', '.claude/settings.json', 'LICENSE',
                     'CRAN_CHECKLIST.md', 'ROADMAP.md', 'docs/benchmarks.md', 'docs/superpowers/specs/x.md', 'CHANGELOG.md'):
            with self.subTest(path=path):
                self.assertOnly(classify(path), 'docs_only')

    def test_docs_beside_a_binding_change_are_not_docs_only(self):
        result = classify('CHANGELOG.md', 'go/uuid.go')
        self.assertOnly(result, 'go')


class MainTests(unittest.TestCase):
    def run_main(self, argv, stdin=''):
        out = io.StringIO()
        with patch('sys.stdin', io.StringIO(stdin)), patch('sys.stdout', out):
            code = pr_changes.main(argv)
        return code, out.getvalue().splitlines()

    def test_all_flag(self):
        code, lines = self.run_main(['--all'])
        self.assertEqual(code, 0)
        self.assertEqual(lines, [f'{key}={"false" if key == "docs_only" else "true"}' for key in KEYS])

    def test_main_prints_every_key(self):
        code, lines = self.run_main([], 'typescript/src/index.ts\t\n')
        self.assertEqual(code, 0)
        self.assertEqual(len(lines), 12)
        self.assertEqual(dict(line.split('=') for line in lines),
                         {key: 'true' if key == 'typescript' else 'false' for key in KEYS})

    def test_main_reads_a_rename(self):
        code, lines = self.run_main([], 'docs/x.md\trust/src/x.rs\n')
        self.assertIn('core=true', lines)
        self.assertIn('docs_only=false', lines)

    def test_main_refuses_a_malformed_line(self):
        code, lines = self.run_main([], 'no tab here\n')
        self.assertNotEqual(code, 0)
        self.assertEqual(lines, [])

    def test_cli(self):
        import subprocess
        run = subprocess.run(['python3', '-I', str(ROOT / '.github/scripts/pr_changes.py')], input='README.md\t\n',
                             capture_output=True, text=True)
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertIn('docs_only=true', run.stdout.splitlines())


if __name__ == '__main__':
    unittest.main()
