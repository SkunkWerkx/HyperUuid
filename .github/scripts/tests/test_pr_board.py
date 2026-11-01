"""The PR board's verdict: red on any failed or cancelled job, red on a job skipped while its
gate said run, green otherwise. The gates mirror ci.yml's `if:` lines, one per job."""
import importlib.util
import io
import json
from pathlib import Path
import re
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
BINDINGS = ['csharp', 'java', 'python', 'ruby', 'php', 'swift', 'typescript', 'r', 'go']


def load():
    spec = importlib.util.spec_from_file_location('pr_board', ROOT / '.github/scripts/pr_board.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


pr_board = load()


def outputs(**on):
    """`changes`' outputs as GitHub hands them over: strings, every key present."""
    keys = ['core', 'ci', 'docs_only'] + BINDINGS
    return {key: 'true' if on.get(key, False) else 'false' for key in keys}


def results(**overrides):
    base = {name: 'success' for name in pr_board.JOBS}
    base.update(overrides)
    return base


class GateTests(unittest.TestCase):
    def test_gate_names_are_the_boards_jobs(self):
        gates = pr_board.gates(outputs(), 'pull_request')
        self.assertEqual(set(gates), set(pr_board.JOBS))
        self.assertNotIn('pr-board', gates)
        self.assertNotIn('request-staging', gates)

    def test_core_gate_is_not_docs_only(self):
        self.assertFalse(pr_board.gates(outputs(docs_only=True), 'pull_request')['core'])
        self.assertTrue(pr_board.gates(outputs(go=True), 'pull_request')['core'])
        self.assertTrue(pr_board.gates(outputs(), 'pull_request')['core'], 'an empty classification is not docs-only')

    def test_one_binding_gates_its_lint_and_test_and_nothing_else(self):
        gates = pr_board.gates(outputs(java=True), 'pull_request')
        on = {name for name, gate in gates.items() if gate}
        self.assertEqual(on, {'changes', 'core', 'lint-java', 'test-java'})
        gates = pr_board.gates(outputs(csharp=True), 'pull_request')
        on = {name for name, gate in gates.items() if gate}
        self.assertEqual(on, {'changes', 'core', 'lint-csharp', 'test-csharp', 'build-static', 'build-wasm', 'test-nuget-package'})
        gates = pr_board.gates(outputs(typescript=True), 'pull_request')
        on = {name for name, gate in gates.items() if gate}
        self.assertEqual(on, {'changes', 'core', 'lint-typescript', 'test-typescript', 'build-wasm', 'test-npm-package'})
        gates = pr_board.gates(outputs(python=True), 'pull_request')
        self.assertTrue(gates['build-wheels'])
        gates = pr_board.gates(outputs(swift=True), 'pull_request')
        self.assertTrue(gates['build-static'])
        gates = pr_board.gates(outputs(go=True), 'pull_request')
        self.assertTrue(gates['lint-go-full'] and gates['build-wasm'])

    def test_core_gates_everything_but_the_release_jobs(self):
        gates = pr_board.gates(outputs(core=True), 'pull_request')
        off = {name for name, gate in gates.items() if not gate}
        self.assertEqual(off, {'test-apple-mobile', 'test-android'})

    def test_release_only_jobs_are_gated_off_on_pull_requests(self):
        everything = outputs(**{key: True for key in ['core', 'ci'] + BINDINGS})
        self.assertFalse(pr_board.gates(everything, 'pull_request')['test-apple-mobile'])
        self.assertFalse(pr_board.gates(everything, 'pull_request')['test-android'])
        for event in ('workflow_dispatch', 'schedule'):
            gates = pr_board.gates(everything, event)
            self.assertTrue(all(gates.values()), event)

    def test_gates_mirror_ci_yml(self):
        # Every job in ci.yml's `jobs:` but the two aggregates has a gate here, and a job's `if:`
        # names exactly the classes its gate reads.
        text = (ROOT / '.github/workflows/ci.yml').read_text()
        body = text[text.index('\njobs:') + 6:]
        starts = list(re.finditer(r'^  ([a-z][a-z0-9-]*):\s*$', body, re.M))
        jobs = {m[1]: body[m.start(): starts[i + 1].start() if i + 1 < len(starts) else len(body)] for i, m in enumerate(starts)}
        self.assertEqual(set(jobs) - {'pr-board', 'request-staging'}, set(pr_board.JOBS))
        for name in pr_board.JOBS:
            with self.subTest(job=name):
                head = jobs[name].split('    steps:')[0]
                gate = re.search(r'^    if: (.*)$', head, re.M)
                classes = set(re.findall(r"needs\.changes\.outputs\.([a-z_]+) == 'true'", gate[1])) if gate else set()
                if name == 'changes':
                    self.assertIsNone(gate)
                    continue
                if name in ('core', 'test-apple-mobile', 'test-android'):
                    continue
                on = {key for key in outputs() if pr_board.gates(outputs(**{key: True}), 'pull_request')[name]}
                self.assertEqual(on, classes)


class VerdictTests(unittest.TestCase):
    def test_all_green(self):
        gates = pr_board.gates(outputs(core=True), 'pull_request')
        self.assertEqual(pr_board.verdict(results(**{'test-apple-mobile': 'skipped', 'test-android': 'skipped'}), gates), (True, 'ok'))

    def test_failure_anywhere_is_red(self):
        gates = pr_board.gates(outputs(core=True), 'pull_request')
        for status in ('failure', 'cancelled'):
            ok, why = pr_board.verdict(results(**{'test-ruby': status, 'test-apple-mobile': 'skipped', 'test-android': 'skipped'}), gates)
            self.assertFalse(ok)
            self.assertEqual(why, 'test-ruby failed')

    def test_skipped_but_gated_on_is_red(self):
        gates = pr_board.gates(outputs(java=True), 'pull_request')
        ok, why = pr_board.verdict(results(**{name: 'skipped' for name in pr_board.JOBS if name not in ('changes', 'core', 'lint-java')}), gates)
        self.assertFalse(ok)
        self.assertEqual(why, 'test-java skipped but gated on')

    def test_skipped_and_gated_off_is_green(self):
        gates = pr_board.gates(outputs(java=True), 'pull_request')
        skipped = {name: 'skipped' for name in pr_board.JOBS if name not in ('changes', 'core', 'lint-java', 'test-java')}
        self.assertEqual(pr_board.verdict(results(**skipped), gates), (True, 'ok'))

    def test_docs_only_board_is_green_with_everything_skipped(self):
        gates = pr_board.gates(outputs(docs_only=True), 'pull_request')
        skipped = {name: 'skipped' for name in pr_board.JOBS if name != 'changes'}
        self.assertEqual(pr_board.verdict(results(**skipped), gates), (True, 'ok'))

    def test_lint_failure_is_named_before_the_skipped_suites(self):
        gates = pr_board.gates(outputs(java=True), 'pull_request')
        given = {'test-java': 'skipped', 'lint-java': 'failure'}
        given.update({name: 'skipped' for name in pr_board.JOBS if name not in given and name not in ('changes', 'core')})
        ok, why = pr_board.verdict(results(**given), gates)
        self.assertEqual((ok, why), (False, 'lint-java failed'))

    def test_a_failed_classifier_is_red(self):
        gates = pr_board.gates(outputs(), 'pull_request')
        ok, why = pr_board.verdict(results(**{name: 'skipped' for name in pr_board.JOBS if name != 'changes'}, changes='failure'), gates)
        self.assertEqual((ok, why), (False, 'changes failed'))

    def test_a_job_the_gates_do_not_know_is_gated_on(self):
        gates = pr_board.gates(outputs(docs_only=True), 'pull_request')
        given = results(**{name: 'skipped' for name in pr_board.JOBS if name != 'changes'})
        given['test-new-binding'] = 'skipped'
        ok, why = pr_board.verdict(given, gates)
        self.assertEqual((ok, why), (False, 'test-new-binding skipped but gated on'))


class MainTests(unittest.TestCase):
    def run_main(self, needs, changes, event):
        out = io.StringIO()
        env = {'NEEDS': json.dumps({name: {'result': status, 'outputs': {}} for name, status in needs.items()}),
               'CHANGES': json.dumps(changes), 'GITHUB_EVENT_NAME': event}
        with patch('sys.stdout', out), patch.dict('os.environ', env, clear=False):
            code = pr_board.main()
        return code, out.getvalue()

    def test_green_exit_0(self):
        needs = results(**{'test-apple-mobile': 'skipped', 'test-android': 'skipped'})
        code, out = self.run_main(needs, outputs(core=True), 'pull_request')
        self.assertEqual(code, 0, out)
        self.assertIn('ok', out)

    def test_red_exit_1_and_says_why(self):
        needs = results(**{'test-go': 'failure', 'test-apple-mobile': 'skipped', 'test-android': 'skipped'})
        code, out = self.run_main(needs, outputs(core=True), 'pull_request')
        self.assertEqual(code, 1)
        self.assertIn('test-go failed', out)

    def test_fork_pr_has_no_attest_inputs(self):
        # Every attesting caller takes its mode from the event, so a fork's pull request
        # (no OIDC) never reaches an attest step; none hard-codes release.
        text = (ROOT / '.github/workflows/ci.yml').read_text()
        body = text[text.index('\njobs:') + 6:]
        starts = list(re.finditer(r'^  ([a-z][a-z0-9-]*):\s*$', body, re.M))
        jobs = {m[1]: body[m.start(): starts[i + 1].start() if i + 1 < len(starts) else len(body)] for i, m in enumerate(starts)}
        for name, job in jobs.items():
            if 'attestations: write' not in job:
                continue
            with self.subTest(job=name):
                self.assertIn("      mode: ${{ github.event_name == 'pull_request' && 'pr' || 'release' }}\n", job)
                self.assertNotIn('mode: release', job)
                self.assertNotIn('attest_native', job, 'the Forge default, release mode only, stands')


if __name__ == '__main__':
    unittest.main()
