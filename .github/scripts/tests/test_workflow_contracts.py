"""Security and coverage contracts for all three coordinated consumer DAGs."""
import os
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[3]
BASE = ROOT.parent
PUBLISHERS = ('nuget-org-publish','rubygems-publish','pypi-publish','pack-crates','maven-publish')


def jobs(path):
    text = path.read_text()
    starts = list(re.finditer(r'^  ([a-z][a-z0-9-]*):\s*$', text[text.index('\njobs:')+6:], re.M))
    body = text[text.index('\njobs:')+6:]
    return {m[1]:body[m.start(): starts[i+1].start() if i+1<len(starts) else len(body)] for i,m in enumerate(starts)}


class ConsumerFlowTests(unittest.TestCase):
    def test_every_registry_publisher_requires_global_gate(self):
        for repo in (ROOT.name,):
            workflow = jobs(BASE/repo/'.github/workflows/release.yml')
            for publisher in PUBLISHERS:
                with self.subTest(repo=repo,publisher=publisher):
                    self.assertIn('publication-gate', workflow[publisher].split('steps:')[0])
                    self.assertIn('!cancelled()', workflow[publisher].split('steps:')[0])

    def test_recovery_and_finalization_are_exact_and_preserved(self):
        for repo in (ROOT.name,):
            text = (BASE/repo/'.github/workflows/release.yml').read_text()
            workflow = jobs(BASE/repo/'.github/workflows/release.yml')
            with self.subTest(repo=repo):
                self.assertIn('prepared_run_id:',text)
                self.assertIn('cancel-in-progress: false',text)
                self.assertIn('release-packages',workflow['publication-gate'])
                self.assertIn('inventory.py verify',workflow['publication-gate'])
                self.assertIn('inventory.py run',workflow['find-ci-run'])
                self.assertIn('retention-days: 90',workflow['publication-gate'])
                for publisher in PUBLISHERS:
                    self.assertIn(publisher,workflow['finalize-release'].split('steps:')[0])

    def test_ci_preserves_native_suites_but_narrows_mobile_dependencies(self):
        for repo in (ROOT.name,):
            workflow = jobs(BASE/repo/'.github/workflows/ci.yml')
            with self.subTest(repo=repo):
                self.assertIn('static_libs_artifact: false',workflow['build-native'])
                self.assertIn('swift_static_tests: false',workflow['build-native'])
                self.assertIn('android_shared_artifact: false',workflow['build-native'])
                self.assertIn('php_extension_schedule: parallel',workflow['build-native'])
                self.assertIn('needs: build-static',workflow['test-apple-mobile'])
                self.assertIn('needs: [build-static, build-android]',workflow['test-android'])
                self.assertIn('needs: build-static',workflow['test-swift-static'])

    def test_no_consumer_forge_call_uses_mutable_master(self):
        for repo in (ROOT.name,):
            for path in (BASE/repo/'.github/workflows').glob('*.yml'):
                for reference in re.findall(r'SkunkWerkx/\.github/\.github/[^\s]+',path.read_text()):
                    with self.subTest(repo=repo,reference=reference):
                        self.assertRegex(reference,r'@[0-9a-f]{40}$')


if __name__=='__main__': unittest.main()
