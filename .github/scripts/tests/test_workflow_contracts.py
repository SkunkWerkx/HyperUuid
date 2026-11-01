"""Shape contracts for this repository's workflows, which call the Forge (SkunkWerkx/.github)
at master and keep only triggers, permissions, gates and the Forge-commit resolution here.

HyperCast and HyperTabular carry the same file; the three move together. When a HyperForge
checkout sits beside this repository, every action and reusable workflow referenced is also
checked against its declared inputs."""
import io
import json
from pathlib import Path
import re
import subprocess
import tarfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
WORKFLOWS = ROOT / '.github/workflows'
FORGE = ROOT.parent / 'HyperForge'
PUBLISHERS = ('nuget-org-publish', 'rubygems-publish', 'pypi-publish', 'npm-publish', 'pack-crates', 'maven-publish')
# release.yml packs nothing: the eight pack jobs are ci.yml's tail (TAIL below).
PACKERS = ()
ACTION_JOBS = {'find-ci-run': 'tag-guard', 'publication-gate': 'tag-gate', 'nuget-org-publish': 'publish-nuget',
               'rubygems-publish': 'publish-rubygems', 'pypi-publish': 'publish-pypi',
               'npm-publish': 'publish-npm', 'finalize-release': 'finalize-release'}
FORGE_WORKFLOW = re.compile(r'SkunkWerkx/\.github/\.github/workflows/([\w-]+\.yml)@(\S+)')


def jobs(path):
    text = path.read_text()
    body = text[text.index('\njobs:') + 6:]
    starts = list(re.finditer(r'^  ([a-z][a-z0-9-]*):\s*$', body, re.M))
    return {m[1]: body[m.start(): starts[i + 1].start() if i + 1 < len(starts) else len(body)]
            for i, m in enumerate(starts)}


def header(job):
    return job.split('    steps:')[0]


def with_block(text):
    """The `with:` keys of one job or step body."""
    match = re.search(r'^( +)with:\n((?:\1  .*\n?)+)', text, re.M)
    return set(re.findall(r'^ +([\w-]+):', match[2], re.M)) if match else set()


class ReleaseFlowTests(unittest.TestCase):
    def setUp(self):
        self.text = (WORKFLOWS / 'release.yml').read_text()
        self.jobs = jobs(WORKFLOWS / 'release.yml')

    def test_the_tag_push_publishes_and_a_dispatch_retries(self):
        # The release train's act two: the tag push verifies what ci.yml preserved, attests it
        # from the tag and publishes; a dispatch at the tag replays with the receipts.
        self.assertIn('    tags: ["v*"]', self.text)
        self.assertEqual(re.findall(r'^      ([a-z_]+):\n        description', self.text, re.M),
                         ['publication_run_id', 'maven_deployment_id'])
        self.assertNotIn('prepared_run_id', self.text.split('\njobs:')[0])   # no dispatch input
        self.assertNotIn('prepared_run_id', self.jobs['find-ci-run'])              # no guard override
        self.assertNotIn('ci_run_id:', self.text.split('\njobs:')[0])
        self.assertIn('cancel-in-progress: false', self.text)
        self.assertEqual([n for n in self.jobs if n.endswith('-pack')], [])
        for publisher in PUBLISHERS:
            with self.subTest(publisher=publisher):
                job = self.jobs[publisher]
                self.assertNotIn("event_name == 'workflow_dispatch'", job)
                if publisher != 'pypi-publish':
                    self.assertIn('publish: true', job)
                self.assertIn('publication_run_id: ${{ inputs.publication_run_id }}', job)
        gate = self.jobs['publication-gate']
        self.assertIn("attest: ${{ github.event_name == 'push' }}", gate)
        self.assertIn('packing_forge_sha: ${{ needs.find-ci-run.outputs.packing_forge_sha }}', gate)
        self.assertNotIn("event_name == 'workflow_dispatch'", header(self.jobs['finalize-release']))
        self.assertNotIn("event_name == 'workflow_dispatch'", header(self.jobs['pie-acceptance']))

    def test_pack_crates_and_maven_take_the_gate_s_verified_packages_at_a(self):
        # The preserved crate and bundle were attested in ci.yml at A; both reusable publishers
        # verify at inputs.source_sha and take the gate's re-uploaded copies from this run.
        for name in ('pack-crates', 'maven-publish'):
            with self.subTest(job=name):
                job = self.jobs[name]
                self.assertIn('prepared_run_id: ${{ github.run_id }}', job)
                self.assertIn('publish: true', job)
                self.assertIn('source_sha: ${{ needs.find-ci-run.outputs.source_sha }}', job)

    def test_pypi_upload_is_a_step_of_this_workflow(self):
        # Nested in the Forge's composite action, pypa/gh-action-pypi-publish looks for its image
        # at ghcr.io/<this repo>:<tag> (actions/runner#2473): v1.0.0's upload failed that way and
        # its wheels went up from another workflow, which PyPI shows as a change of provenance.
        # The PEP 740 publish attestations must name this file, so the upload is a step here.
        job = self.jobs['pypi-publish']
        steps = [job.index('uses: ./.hyperforge/.github/actions/publish-pypi'),
                 job.index('- name: Publish only missing preserved wheels and sdist (Trusted Publishing)'),
                 job.index('- name: Verify every public wheel and sdist matches the prepared bytes')]
        self.assertEqual(steps, sorted(steps))
        self.assertIn('      - id: pypi\n        uses: ./.hyperforge/.github/actions/publish-pypi', job)
        self.assertEqual(job.count('uses: pypa/gh-action-pypi-publish@'), 1)
        self.assertRegex(job, r"if: steps\.pypi\.outputs\.pending == 'true'\n"
                              r" +uses: pypa/gh-action-pypi-publish@[0-9a-f]{40} # v[0-9.]+\n +with:\n +packages-dir: pending-pypi\n")
        self.assertRegex(job, r"Verify every public wheel and sdist matches the prepared bytes\n"
                              r' +run: python3 "\$FORGE_SCRIPTS/release_inventory\.py" registry-confirm --registry pypi\n')
        self.assertNotIn('pypa/', '\n'.join(j for name, j in self.jobs.items() if name != 'pypi-publish'))

    def test_every_registry_publisher_requires_the_global_gate(self):
        for publisher in PUBLISHERS:
            with self.subTest(publisher=publisher):
                head = header(self.jobs[publisher])
                self.assertIn('needs: [find-ci-run, publication-gate]', head)
                self.assertIn('!cancelled()', head)
                self.assertIn("needs.publication-gate.result == 'success'", head)
        # The gate waits for the guard alone: nothing is packed here, on either event.
        gate = header(self.jobs['publication-gate'])
        self.assertEqual(CiTests.needs(self.jobs['publication-gate']), {'find-ci-run'})
        self.assertIn("needs.find-ci-run.result == 'success'", gate)
        self.assertNotIn('prepared_run_id', gate)
        self.assertEqual({k: v for k, v in re.findall(r'^          (npm_packages|cran_packages): (.*)$',
                                                        self.jobs['publication-gate'], re.M)},
                         {'npm_packages': 'true', 'cran_packages': 'true'})
        finalize = header(self.jobs['finalize-release'])
        finalize_needs = set(re.split(r'[\s,]+', re.search(r'^    needs: \[([^]]*)\]', finalize, re.M)[1].strip()))
        self.assertEqual(finalize_needs, {'find-ci-run', 'publication-gate', *PUBLISHERS})
        for publisher in PUBLISHERS:
            self.assertIn(f"needs.{publisher}.result == 'success'", finalize)

    def test_pie_acceptance_runs_last_against_the_published_release(self):
        # PIE reads the GitHub Release, so the acceptance waits for the finalizer that writes it,
        # on a publish dispatch only, and asks for this release's bare version.
        names = list(self.jobs)
        self.assertEqual(names[-1], 'pie-acceptance')
        job = self.jobs['pie-acceptance']
        head = header(job)
        self.assertEqual(set(re.split(r'[\s,]+', re.search(r'^    needs: \[([^]]*)\]', head, re.M)[1].strip())),
                         {'find-ci-run', 'finalize-release'})
        self.assertIn("needs.finalize-release.result == 'success'", head)
        self.assertIn('    uses: SkunkWerkx/.github/.github/workflows/hyper-pie-acceptance.yml@master\n', job)
        self.assertIn('      package: skunkwerkx/hyperuuid\n', job)
        self.assertIn('      extension: hyperuuid\n', job)
        self.assertIn('      package_version: ${{ needs.find-ci-run.outputs.version }}\n', job)
        self.assertIn('      version: ${{ steps.guard.outputs.version }}\n', self.jobs['find-ci-run'])
        self.assertEqual(json.loads((ROOT / 'composer.json').read_text())['name'], 'skunkwerkx/hyperuuid')

    def test_forge_commit_is_resolved_once_and_checked_out_before_every_action(self):
        find = self.jobs['find-ci-run']
        order = [find.index('name: Resolve the Forge commit this run uses'),
                 find.index('path: .hyperforge'), find.index('uses: ./.hyperforge/.github/actions/tag-guard')]
        self.assertEqual(order, sorted(order))
        self.assertIn('referenced_workflows', find)
        # The job's forge_sha output and the guard's input, the .hyperforge checkout, the one write.
        for line, count in (('forge_sha: ${{ steps.forge.outputs.revision }}', 2),
                            ('ref: ${{ steps.forge.outputs.revision }}', 1),
                            ('echo "revision=$shas" >> "$GITHUB_OUTPUT"', 1)):
            self.assertEqual(find.count(line), count, line)
        for job, action in ACTION_JOBS.items():
            with self.subTest(job=job):
                body = self.jobs[job]
                self.assertIn(f'uses: ./.hyperforge/.github/actions/{action}', body)
                self.assertIn('repository: SkunkWerkx/.github', body)
                self.assertIn('persist-credentials: false', body)
                if job != 'find-ci-run':
                    self.assertIn('ref: ${{ needs.find-ci-run.outputs.forge_sha }}', body)
                if job != 'find-ci-run':
                    # The actions that verify the inventory or publish record the Forge commit.
                    self.assertIn('forge_sha: ${{ needs.find-ci-run.outputs.forge_sha }}', body)

    def test_read_only_checkouts_are_not_named_like_a_pull_request_head(self):
        # CodeQL's actions/cache-poisoning/poisonable-step reads a checkout ref whose step id, needed
        # job or output name contains one of these words as a pull request head, and flags a job
        # without write permissions that then runs the checked-out code.
        heuristic = re.compile(r'head|sha|commit|branch|ref')
        for name, job in self.jobs.items():
            if re.search(r'^      [a-z-]+: write$', header(job), re.M):
                continue
            for expr in re.findall(r'^ +ref: \$\{\{ *(.*?) *\}\}', job, re.M):
                with self.subTest(job=name, ref=expr):
                    context, *parts = expr.split('.')
                    self.assertIn(context, ('steps', 'needs'))
                    self.assertFalse([p for p in parts if p != 'outputs' and heuristic.search(p)])

    def test_least_privilege(self):
        self.assertIn('\npermissions:\n  contents: read\n', self.text)
        for name, job in self.jobs.items():
            with self.subTest(job=name):
                grants = set(re.findall(r'^      ([a-z-]+): write$', header(job), re.M))
                allowed = {'id-token', 'attestations'} | ({'contents'} if name == 'finalize-release' else set())
                self.assertLessEqual(grants, allowed)
        self.assertNotIn('actions: write', self.text)

    def test_nuget_publisher_user_and_secrets_stay_in_the_caller(self):
        self.assertIn('trusted_publisher_user: ${{ vars.NUGET_TRUSTED_PUBLISHER_USER }}', self.jobs['nuget-org-publish'])
        for secret in ('MAVEN_CENTRAL_USERNAME', 'MAVEN_CENTRAL_PASSWORD', 'MAVEN_SIGNING_KEY', 'MAVEN_SIGNING_KEY_PASSWORD'):
            self.assertIn(f'{secret}: ${{{{ secrets.{secret} }}}}', self.jobs['maven-publish'])

    def test_maven_jobs_pack_the_jni_libraries(self):
        self.assertIn('      java_jni: true\n', self.jobs['maven-publish'])
        self.assertIn('      java_jni: true\n', jobs(WORKFLOWS / 'ci.yml')['maven-pack'])


class PreparationFlowTests(unittest.TestCase):
    def test_prepare_release_is_one_forge_call(self):
        text = (WORKFLOWS / 'prepare-release.yml').read_text()
        job = jobs(WORKFLOWS / 'prepare-release.yml')['prepare-release']
        self.assertIn('uses: SkunkWerkx/.github/.github/workflows/hyper-prepare-release.yml@master', job)
        self.assertIn('version: ${{ inputs.version }}', job)
        self.assertIn('group: release-source-${{ github.repository }}', text)
        self.assertIn('actions: write', job)

    def test_codeql_swift_leg_stages_the_preparation_runs_core(self):
        # No checkout carries a Swift core since it left git (2026-10-30), so the Swift leg builds
        # against the zips a release-mode ci.yml run attested: the one the preparation run names,
        # or the newest green one on master for the weekly scan.
        text = (WORKFLOWS / 'codeql.yml').read_text()
        self.assertRegex(text, r'\n      ci_run_id:\n(?:        .*\n)*        required: false\n')
        self.assertIn('attestations: read', text)
        job = jobs(WORKFLOWS / 'codeql.yml')['analyze']
        names = re.findall(r'^      - name: (.*)$', job, re.M)
        wanted = ['Resolve the release-mode run whose core to build against', "Stage that run's attested Swift core",
                  'Build the Swift library']
        self.assertEqual([n for n in names if n in wanted], wanted)
        # The Swift leg runs where the core legs run: the xcode-27 image carries the Swift
        # toolchain the build step needs; macos-latest tracks whichever Xcode GitHub promotes.
        self.assertIn('          - language: swift\n            build-mode: manual\n            runner: xcode-27\n', text)
        self.assertNotIn('macos-latest', text)
        for needle in ("if: matrix.build-mode == 'manual'", 'name: swift-release-assets', 'run-id: ${{ steps.run.outputs.id }}',
                       '--signer-repo SkunkWerkx/.github', '--source-digest', 'swift_release.py" unpack --assets'):
            with self.subTest(needle=needle):
                self.assertIn(needle, job)
        resolve = job.split('      - name: Resolve the release-mode run whose core to build against\n')[1].split('\n\n')[0]
        self.assertIn('inputs.ci_run_id', resolve)
        for needle in ('--workflow ci.yml', '--branch master', '--status success', '"workflow_dispatch" or .event == "schedule"'):
            self.assertIn(needle, resolve)
        self.assertIn('repository: SkunkWerkx/.github', job)
        self.assertEqual(job.count("if: matrix.build-mode == 'manual'"), 5, 'the Forge checkout, resolve, download, stage and build')

    def test_only_codeql_may_start_a_workflow(self):
        # Least privilege across the board: the attesting callers and the pack jobs hold the two
        # attestation scopes, codeql alone may start a workflow (codeql.yml, on this run), and
        # stage-release alone may write the repository (B).
        text = (WORKFLOWS / 'ci.yml').read_text()
        self.assertIn('\npermissions:\n  contents: read\n', text)
        for name, job in jobs(WORKFLOWS / 'ci.yml').items():
            with self.subTest(job=name):
                grants = set(re.findall(r'^      ([a-z-]+): write$', header(job), re.M))
                allowed = {'codeql': {'actions'}, 'stage-release': {'contents'}}.get(name, {'id-token', 'attestations'})
                self.assertLessEqual(grants, allowed)


BINDINGS = ['csharp', 'java', 'python', 'ruby', 'php', 'swift', 'typescript', 'r', 'go']
LABELS = {'ubuntu-26.04', 'ubuntu-26.04-arm', 'xcode-27', 'windows-2025-vs2026', 'windows-11-vs2026-arm'}
ALLOWED_EXCEPTIONS = set()
MODE = "${{ github.event_name == 'pull_request' && 'pr' || 'release' }}"
# The Forge workflows with an attestation step (or a composite that attests) in their jobs;
# ForgeInterfaceTests.test_attesting_set_is_current holds this to the checkout.
ATTESTING = {'hyper-core.yml', 'hyper-build-static.yml', 'hyper-build-android.yml', 'hyper-build-wasm.yml',
             'hyper-build-wheels.yml', 'hyper-build-php-extension.yml', 'hyper-test-java.yml',
             'hyper-test-typescript.yml', 'hyper-test-ruby.yml',
             'hyper-pack-nuget.yml', 'hyper-pack-crate.yml', 'hyper-pack-maven.yml'}
NO_MODE = {'hyper-core-checks.yml', 'hyper-pack-nuget.yml', 'hyper-pack-crate.yml', 'hyper-pack-maven.yml'}
# The release tail of ci.yml, in dependency order (pr_board.py's TAIL): the Forge resolver, the
# eight pack jobs, the inventory, CodeQL and B; gated on prepare-release.yml's master dispatch.
TAIL = ['forge', 'nuget-org-pack', 'crate-pack', 'maven-pack', 'ruby-pack', 'npm-pack', 'r-pack', 'c-pack', 'pypi-pack',
        'release-prepare', 'codeql', 'stage-release']
TAIL_GATE = "github.event_name == 'workflow_dispatch' && github.ref == 'refs/heads/master'"


class CiTests(unittest.TestCase):
    def setUp(self):
        self.jobs = jobs(WORKFLOWS / 'ci.yml')

    @staticmethod
    def needs(job):
        match = re.search(r'^    needs: \[([^]]*)\]', header(job), re.M)
        if match:
            return set(re.split(r'[\s,]+', match[1].strip()))
        match = re.search(r'^    needs: (\S+)$', header(job), re.M)
        return {match[1]} if match else set()

    @staticmethod
    def gate(job):
        match = re.search(r'^    if: (.*)$', header(job), re.M)
        return match[1].strip() if match else None

    @staticmethod
    def g(*keys):
        return ' || '.join(f"needs.changes.outputs.{key} == 'true'" for key in keys)

    def test_mobile_dependencies(self):
        # Release board only, both of them; the C smoke's simulator run joins test-apple-mobile,
        # which is why it waits for the packed archives.
        apple, android = self.jobs['test-apple-mobile'], self.jobs['test-android']
        self.assertEqual(self.needs(apple), {'changes', 'build-static', 'pack-c'})
        self.assertEqual(self.needs(android), {'changes', 'build-static', 'build-android'})
        for job in (apple, android):
            self.assertIn("    if: github.event_name != 'pull_request'\n", header(job))
        self.assertNotIn('test-swift-static', self.jobs, "Swift's static suite is its Linux test legs' now")
        self.assertNotIn('build-native', self.jobs)
        self.assertNotIn('check-core', self.jobs)
        self.assertNotIn('test-c', self.jobs)

    def test_php_floor_is_explicit_and_matches_both_composer_files(self):
        # The Forge's php_floor_version default (8.2) serves the siblings; this repo says its own.
        m = re.search(r'^      php_floor_version: "(\d+\.\d+)"$', self.jobs['test-php'], re.M)
        self.assertIsNotNone(m)
        floor = m.group(1)
        self.assertEqual(floor, '8.3')
        for composer in (ROOT / 'composer.json', ROOT / 'php' / 'composer.json'):
            self.assertEqual(json.loads(composer.read_text())['require']['php'], '>=' + floor)

    def test_root_composer_is_a_pie_extension(self):
        composer = json.loads((ROOT / 'composer.json').read_text())
        self.assertEqual(composer['type'], 'php-ext')
        ext = composer['php-ext']
        self.assertEqual(ext['extension-name'], 'hyperuuid')
        self.assertEqual(ext['download-url-method'], ['pre-packaged-binary'])
        self.assertIs(ext['support-zts'], False)
        self.assertEqual(ext['os-families'], ['linux', 'darwin', 'windows'])
        self.assertNotIn('autoload', composer)
        self.assertNotIn('ext-ffi', composer['require'])

    def test_git_archive_is_metadata_only(self):
        # PIE fetches the dist archive for its metadata. Git applies directory exclusion itself,
        # and --worktree-attributes reads the edited .gitattributes without a commit. go/ stays
        # in: the Go module proxy archives it.
        archive = subprocess.run(['git', 'archive', '--worktree-attributes', '--format=tar', 'HEAD'],
                                 cwd=ROOT, check=True, stdout=subprocess.PIPE).stdout
        with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
            top = {name.split('/')[0] for name in tar.getnames() if name != 'pax_global_header'}
        self.assertEqual(top, {'composer.json', 'LICENSE', 'README.md', 'go'})

    def test_no_php_sources_in_working_tree(self):
        self.assertFalse((ROOT / 'php' / 'src').exists())

    def test_no_php_native_references(self):
        # Nothing stages or reads PHP libraries from git any more: PIE installs the extension
        # from the release's zips. This file names the path to look for it, so it is skipped.
        this = Path(__file__).resolve()
        found = []
        for directory in (ROOT / '.github/workflows', ROOT / '.github/scripts'):
            for path in sorted(directory.rglob('*')):
                if not path.is_file() or path.resolve() == this or '__pycache__' in path.parts:
                    continue
                if 'php/src/native' in path.read_text(errors='replace'):
                    found.append(str(path.relative_to(ROOT)))
        self.assertEqual(found, [])

    def test_nuget_package_job_proves_the_packed_layout(self):
        job = self.jobs['test-nuget-package']
        self.assertEqual(self.needs(job), {'changes', 'core', 'build-static'})
        self.assertIn('    runs-on: ubuntu-26.04\n', header(job))
        self.assertEqual(self.gate(job), self.g('csharp', 'core', 'ci'))
        self.assertIn('setup-dotnet', job)
        self.assertIn('global-json-file: global.json', job)
        self.assertNotIn('dotnet-version', job)
        for needle in ('HYPERUUID003', 'NU1202', '.nobuild', 'readelf -d'):
            self.assertIn(needle, job)
        self.assertIn('test-nuget-package', self.needs(self.jobs['forge']))

    def test_npm_package_job_proves_the_fallback_in_consumers(self):
        job = self.jobs['test-npm-package']
        self.assertEqual(self.needs(job), {'changes', 'core', 'build-wasm', 'test-typescript'},
                         'node-<rid> comes from the TypeScript legs, node-wasm32-wasip1 from build-wasm')
        self.assertIn('    runs-on: ${{ matrix.os }}\n', header(job))
        self.assertIn('        os: [ubuntu-26.04, xcode-27, windows-2025-vs2026]\n', header(job))
        self.assertIn('      fail-fast: false\n', header(job))
        self.assertEqual(self.gate(job), self.g('typescript', 'core', 'ci'))
        for needle in ('npm_package.py" pack', 'NAPI_RS_FORCE_WASI=error npm test', 'npm run typecheck',
                       'node-wasm32-wasip1', 'run.sh', 'setup-qemu-action', 'path: .hyperforge',
                       'repository: SkunkWerkx/.github', 'ref: master', 'persist-credentials: false',
                       'node-version: "26"', 'oven-sh/setup-bun@', 'merge-multiple: false', 'GITHUB_STEP_SUMMARY'):
            with self.subTest(needle=needle):
                self.assertIn(needle, job)
        self.assertNotIn('--allow-missing-natives', job)
        # PR mode has no osx-x64 build (hyper-core.yml skips it), so the pack leaves that
        # platform package out by name, as test-c does; release mode skips nothing. The consumer
        # harness needs no list of absent natives: its lockfile scenario names darwin-arm64 only.
        pack = next(step for step in re.split(r'\n      - ', job)[1:] if 'npm_package.py" pack' in step)
        self.assertIn("MODE: ${{ github.event_name == 'pull_request' && 'pr' || 'release' }}", pack)
        self.assertIn('release) skip=() ;;', pack)
        self.assertIn('pr) skip=(--skip osx-x64) ;;', pack)
        # macOS's /bin/bash is 3.2, where `set -u` rejects an empty array expanded as
        # "${skip[@]}": release mode (the empty case) died on xcode-27 in the 1.1.0 preparation.
        self.assertIn('${skip[@]+"${skip[@]}"}', pack)
        self.assertNotIn(' "${skip[@]}"', pack)
        self.assertNotIn('SKIP_NATIVES', job, 'the harness no longer takes a list of absent natives')
        bun = next(step for step in re.split(r'\n      - ', job)[1:] if 'test:bun' in step)
        self.assertIn('continue-on-error: true', bun, "Bun's forced-WASI result is recorded, never a failure")
        # Bun's runner prints each failure as a `::error` workflow command on GitHub Actions, and
        # Bun 1.4.4 cannot load the WASI module: without this the tolerated step posted ten error
        # annotations per leg on the 1.1.0 preparation. Commands are off for its output, resumed
        # before the exit status is returned, so the step after still records the result.
        self.assertIn('echo "::stop-commands::$token"', bun)
        self.assertIn('NAPI_RS_FORCE_WASI=error npm run test:bun || status=$?', bun)
        self.assertIn('echo "::$token::"\n          exit "$status"', bun)
        self.assertEqual(job.count('continue-on-error: true'), 1, 'nothing but Bun is tolerated')
        for step in re.split(r'\n      - ', job)[1:]:
            if 'run:' in step:
                self.assertIn('shell: bash', step, step.splitlines()[0])
        linux = next(step for step in re.split(r'\n      - ', job)[1:] if 'run.sh' in step and ' all' in step)
        self.assertIn("matrix.os == 'ubuntu-26.04'", linux)
        others = next(step for step in re.split(r'\n      - ', job)[1:] if 'run.sh' in step and 'bundlers' in step)
        self.assertIn("matrix.os != 'ubuntu-26.04'", others)
        self.assertNotIn('mac-lockfile', others)
        self.assertNotIn('floor', others)
        wasm = self.jobs['build-wasm']
        self.assertIn('      node_wasi: ${{ ' + self.g('core', 'ci', 'typescript') + ' }}\n', wasm)
        self.assertIn('test-npm-package', self.needs(self.jobs['forge']))

    def test_gate_admits_blazor_webassembly_on_net10_and_net11(self):
        # The build-std archive links on both toolchains, so HYPERUUID003 no longer names Blazor.
        job = self.jobs['test-nuget-package']
        self.assertNotIn('Blazor WebAssembly needs .NET 11', job)
        name = '      - name: The gate admits Blazor WebAssembly on .NET 10 and 11\n'
        self.assertIn(name, job)
        step = job.split(name, 1)[1].split('\n      - ', 1)[0]
        self.assertIn('for f in net10.0 net11.0; do', step)
        self.assertIn('-getProperty:_HyperUuidNet10Unsupported -p:GateFramework="$f" -p:RuntimeIdentifier=browser-wasm', step)
        self.assertIn('-p:WasmNativeWorkloadAvailable=false', step)
        self.assertIn('grep -qF "error HYPERUUID002"', step)
        self.assertNotIn('grep -qF "error HYPERUUID003"', step)
        self.assertIn('if grep -qF "HYPERUUID003"', step)

    def test_typescript_and_r_callers(self):
        # The Forge's lint_node and lint_r default false until the siblings carry the bindings;
        # the test workflows' toggles default true.
        self.assertIn('uses: SkunkWerkx/.github/.github/workflows/hyper-test-typescript.yml@master', self.jobs['test-typescript'])
        self.assertIn('uses: SkunkWerkx/.github/.github/workflows/hyper-test-r.yml@master', self.jobs['test-r'])
        self.assertIn('      lint_node: true\n', self.jobs['lint-typescript'])
        self.assertIn('      lint_r: true\n', self.jobs['lint-r'])

    def test_go_caller_and_wasm_legs(self):
        # Go's core is Go source (go/internal/core, translated from the wasm32-wasip1 build):
        # no archive is staged for it anywhere, its suites wait for nothing but its lint, and
        # the wasm workflow runs its whole suite under TinyGo and stock Go beside the browser build.
        ci = (WORKFLOWS / 'ci.yml').read_text()
        self.assertNotIn('go/staticlib', ci)
        go = self.jobs['test-go']
        self.assertIn('uses: SkunkWerkx/.github/.github/workflows/hyper-test-go.yml@master', go)
        self.assertEqual(self.needs(go), {'changes', 'lint-go'})
        self.assertIn('      linters: fast\n', self.jobs['lint-go'])
        self.assertIn('      linters: full\n', self.jobs['lint-go-full'])
        self.assertEqual({name for name, job in self.jobs.items() if 'lint-go-full' in self.needs(job)},
                         {'forge', 'pr-board'}, 'the full Go lint gates nothing but the two aggregates')
        wasm = self.jobs['build-wasm']
        go_gate = self.g('core', 'ci', 'go')
        self.assertIn('      go_wasm_suite: ${{ ' + go_gate + ' }}\n', wasm)
        self.assertIn("      go_tinygo_smoke: ${{ (" + go_gate + ") && 'go/internal/tinygosmoke' || '' }}\n", wasm)
        # iOS: Go itself requires external linking on ios/arm64, so the simulator run keeps
        # Go's own clang wrapper; the device build is built and must exist, and nothing is
        # inspected for a core symbol, since there is no linked core.
        apple = self.jobs['test-apple-mobile']
        simulator = apple.split('- name: Go \u2014 the suite in the iOS simulator')[1].split('\n      - ')[0]
        self.assertIn('GOOS=ios GOARCH=arm64 CGO_ENABLED=1 CC="$(go env GOROOT)/misc/ios/clangwrap.sh"', simulator)
        self.assertNotIn('-tags', simulator)
        device = apple.split('- name: Go \u2014 an iOS device build')[1].split('\n      - ')[0]
        self.assertIn('go test -c -o "$RUNNER_TEMP/go-ios/hyperuuid-device.test" .', device)
        self.assertIn('[ -s "$RUNNER_TEMP/go-ios/hyperuuid-device.test" ]', device)
        for gone in ('nm ', '_uuid_new_v7', 'links the core'):
            self.assertNotIn(gone, device)
        # Android: the script links the test binary through the NDK because Go requires it on
        # android/amd64; the workflow names no compiler and no archive for Go.
        android = self.jobs['test-android']
        self.assertIn('.github/scripts/android_build_suite.sh go "$arch" "stage/go-$arch"', android)
        self.assertNotIn('CC=', android)
        self.assertNotIn('static-libs artifact lands them in go', android)
        # The NDK's lld leaves dynamic-relocation addends out of the data unless asked, and Go's
        # linker reads the FIPS section addresses back from the file after external linking:
        # without the flag, GODEBUG=fips140=on panics at start with "verification mismatch".
        script = (ROOT / '.github/scripts/android_build_suite.sh').read_text()
        self.assertIn('-ldflags=-extldflags=-Wl,-z,max-page-size=16384,--apply-dynamic-relocs', script)

    def test_java_caller(self):
        # The Forge's java_jni input defaults false; the jar's floor is JDK 21 beside the primary 25.
        java = self.jobs['test-java']
        self.assertIn('      java_jni: true\n', java)
        self.assertIn('      jdk_floor_version: "21"\n', java)
        self.assertIn('      lint_java_jni: true\n', self.jobs['lint-java'])

    def test_core_checks_is_the_forge_matrix_with_the_core_inputs(self):
        core = self.jobs['core-checks']
        self.assertIn('uses: SkunkWerkx/.github/.github/workflows/hyper-core-checks.yml@master', core)
        self.assertEqual(self.needs(core), {'changes'})
        self.assertEqual(self.gate(core), self.g('core', 'ci'))
        self.assertEqual(with_block(core), {'no_std_rustflags', 'extra_features', 'no_panic_command',
                                             'clippy_targets', 'big_endian'})
        self.assertIn('big_endian: s390x-unknown-linux-gnu', core)
        self.assertIn('no_std_rustflags: --cfg getrandom_backend="custom"', core)
        self.assertIn('extra_features: uuid1', core)
        self.assertIn('no_panic_command: cargo build --release --example no_panic --features no-panic,uuid1', core)
        # The second link proves the WebAssembly batch walk, which only that cfg builds on a host.
        self.assertIn(' && RUSTFLAGS="--cfg hyperuuid_words_walk" cargo build --release --example no_panic'
                      ' --features no-panic,uuid1\n', core)
        self.assertIn('clippy_targets: aarch64-linux-android', core)
        inline = [name for name in self.jobs if re.match(r'(check|lint|format)-', name) and not re.match(r'lint-[a-z]+(-full)?$', name)]
        self.assertEqual(inline, [], 'static checks live in hyper-core-checks.yml and the lint callers, once each')
        for name in self.jobs:
            if name.startswith('lint-'):
                self.assertIn('    uses: SkunkWerkx/.github/.github/workflows/hyper-lint-', self.jobs[name], name)

    # The board's shape: every gate reads `changes`, every suite waits for its lint and the core,
    # the two consumer tests and the two mobile jobs keep their inputs, and pr-board reads it all.

    def test_changes_and_pr_board_are_the_only_unconditional_jobs(self):
        for name, job in self.jobs.items():
            with self.subTest(job=name):
                if name in ('changes',):
                    self.assertIsNone(self.gate(job))
                elif name == 'pr-board':
                    self.assertEqual(self.gate(job), 'always()')
                else:
                    self.assertIsNotNone(self.gate(job), 'every other job is gated')

    def test_every_job_that_reads_changes_needs_it(self):
        for name, job in self.jobs.items():
            if name == 'changes':
                continue
            with self.subTest(job=name):
                self.assertIn('changes', self.needs(job))

    def test_core_is_one_forge_core_call_gated_on_docs_only(self):
        core = self.jobs['core']
        self.assertIn('uses: SkunkWerkx/.github/.github/workflows/hyper-core.yml@master', core)
        self.assertEqual(self.needs(core), {'changes'})
        self.assertEqual(self.gate(core), "needs.changes.outputs.docs_only != 'true'")
        self.assertEqual(with_block(core), {'mode', 'crate', 'project'})
        self.assertEqual(set(re.findall(r'^      ([a-z-]+): write$', header(core), re.M)), {'id-token', 'attestations'})
        self.assertEqual([name for name, job in self.jobs.items() if 'workflows/hyper-core.yml@master' in job], ['core'])

    def test_every_lint_caller_needs_only_changes(self):
        for binding in BINDINGS:
            with self.subTest(binding=binding):
                lint = self.jobs[f'lint-{binding}']
                self.assertEqual(self.needs(lint), {'changes'})
                self.assertEqual(self.gate(lint), self.g(binding, 'core', 'ci'))
                self.assertIn(f'uses: SkunkWerkx/.github/.github/workflows/hyper-lint-{binding}.yml@master', lint)
                self.assertFalse(re.findall(r'^      [a-z-]+: write$', header(lint), re.M), 'a lint attests nothing')
        full = self.jobs['lint-go-full']
        self.assertEqual(self.needs(full), {'changes'})
        self.assertEqual(self.gate(full), self.g('go', 'core', 'ci'))

    def test_every_test_caller_needs_its_lint_and_core(self):
        extra = {'csharp': {'build-static'}, 'swift': {'build-static'}}
        workflow = {binding: f'hyper-test-{binding}.yml' for binding in BINDINGS}
        workflow['php'] = 'hyper-build-php-extension.yml'
        for binding in BINDINGS:
            with self.subTest(binding=binding):
                test = self.jobs[f'test-{binding}']
                core = set() if binding == 'go' else {'core'}
                self.assertEqual(self.needs(test), {'changes', f'lint-{binding}'} | core | extra.get(binding, set()))
                self.assertEqual(self.gate(test), self.g(binding, 'core', 'ci'))
                self.assertIn(f'uses: SkunkWerkx/.github/.github/workflows/{workflow[binding]}@master', test)

    def test_attesting_callers_hold_the_write_scopes(self):
        # A reusable workflow cannot escalate past its caller: the ones that attest get the two
        # scopes, and no other job does (least privilege; test_only_codeql_may_start_a_workflow).
        for name, job in self.jobs.items():
            with self.subTest(job=name):
                grants = set(re.findall(r'^      ([a-z-]+): write$', header(job), re.M))
                workflow = FORGE_WORKFLOW.search(job)
                if workflow and workflow[1] in ATTESTING:
                    self.assertEqual(grants, {'id-token', 'attestations'})
                elif name not in TAIL:
                    self.assertEqual(grants, set())

    def test_every_forge_call_passes_mode_when_declared(self):
        for name, job in self.jobs.items():
            workflow = FORGE_WORKFLOW.search(job)
            if not workflow or workflow[1] in NO_MODE:
                continue
            with self.subTest(job=name):
                self.assertIn('      mode: ' + MODE + '\n', job)
                self.assertNotIn("mode: release", job)

    def test_pack_c_runs_the_pack_alone_in_this_runs_mode(self):
        pack = self.jobs['pack-c']
        self.assertIn('uses: SkunkWerkx/.github/.github/workflows/hyper-test-c.yml@master', pack)
        self.assertEqual(self.needs(pack), {'changes', 'core', 'build-static', 'build-android'})
        self.assertEqual(self.gate(pack), self.g('core', 'ci'))
        self.assertEqual(re.findall(r'^      (smokes|lint): (\w+)$', pack, re.M), [('smokes', 'false'), ('lint', 'false')])
        self.assertIn('      mode: ' + MODE + '\n', pack)

    def test_build_static_runs_for_the_bindings_that_link_it(self):
        static = self.jobs['build-static']
        self.assertEqual(self.needs(static), {'changes'})
        self.assertEqual(self.gate(static), self.g('core', 'ci', 'csharp', 'swift'))
        android = self.jobs['build-android']
        self.assertEqual(self.needs(android), {'changes'})
        self.assertEqual(self.gate(android), self.g('core', 'ci'))

    def test_php_pr_legs_are_two_paths(self):
        php = self.jobs['test-php']
        self.assertIn('      pr_legs: two-paths\n', php)
        self.assertIn('      php_floor_version: "8.3"\n', php)

    def test_build_wasm_inputs_are_gated(self):
        wasm = self.jobs['build-wasm']
        self.assertEqual(self.needs(wasm), {'changes'})
        self.assertEqual(self.gate(wasm), self.g('core', 'ci', 'csharp', 'typescript', 'go'))
        self.assertIn('      archive: ${{ ' + self.g('core', 'ci', 'csharp') + ' }}\n', wasm, 'the archive itself, not only its check')
        self.assertIn("      blazor_check: ${{ (" + self.g('core', 'ci', 'csharp') + ") && 'csharp/HyperUuid.WasmSmokeTest/check.sh' || '' }}\n", wasm)
        self.assertIn("      rust_browser_test: ${{ (" + self.g('core', 'ci') + ") && 'rust/browser-test' || '' }}\n", wasm)
        self.assertIn('      ruby_wasm_smoke: ruby/wasm-smoke\n', wasm)
        wheels = self.jobs['build-wheels']
        self.assertEqual(self.needs(wheels), {'changes'})
        self.assertEqual(self.gate(wheels), self.g('python', 'core', 'ci'))

    def test_no_job_excludes_release_mode(self):
        for name, job in self.jobs.items():
            with self.subTest(job=name):
                self.assertNotIn("event_name == 'pull_request'", self.gate(job) or '')

    def test_the_tail_replaces_request_staging(self):
        self.assertNotIn('request-staging', self.jobs)
        self.assertFalse((WORKFLOWS / 'stage-native-binaries.yml').exists())
        board = set(self.jobs) - set(TAIL) - {'pr-board'}
        self.assertEqual(self.needs(self.jobs['forge']), board)
        for name in TAIL:
            with self.subTest(job=name):
                self.assertEqual(self.gate(self.jobs[name]), TAIL_GATE)
                self.assertIn('changes', self.needs(self.jobs[name]))
        for packer in TAIL[1:9]:
            self.assertEqual(self.needs(self.jobs[packer]), {'changes', 'forge'})
        self.assertEqual(self.needs(self.jobs['release-prepare']), {'changes', 'forge', *TAIL[1:9]})
        self.assertEqual(self.needs(self.jobs['codeql']), {'changes', 'forge'})
        self.assertEqual(self.needs(self.jobs['stage-release']), {'changes', 'forge', 'release-prepare', 'codeql'})
        self.assertEqual(list(self.jobs)[-len(TAIL) - 1:], TAIL + ['pr-board'])

    def test_pack_jobs_pack_this_run_at_this_commit(self):
        for name in ('ruby-pack', 'npm-pack', 'c-pack', 'pypi-pack'):
            job = self.jobs[name]
            self.assertIn('run_id: ${{ github.run_id }}', job)
            self.assertIn('source_sha: ${{ github.sha }}', job)
        self.assertIn('source_sha: ${{ github.sha }}', self.jobs['r-pack'])
        for name in ('nuget-org-pack', 'maven-pack'):
            self.assertIn('source_run_id: ${{ github.run_id }}', self.jobs[name])
        for name in [*TAIL[4:9], 'release-prepare', 'stage-release']:   # the jobs that run Forge code; codeql only dispatches
            with self.subTest(job=name):
                self.assertIn('ref: ${{ needs.forge.outputs.revision }}', self.jobs[name])
                self.assertIn('persist-credentials: false', self.jobs[name])
        self.assertNotIn('actions/checkout', self.jobs['codeql'])
        prepare = self.jobs['release-prepare']
        for line in ('version: ${{ needs.forge.outputs.version }}', 'source_sha: ${{ github.sha }}', 'ci_run_id: ${{ github.run_id }}',
                     'forge_sha: ${{ needs.forge.outputs.revision }}', 'npm_packages: true', 'cran_packages: true'):
            self.assertIn(line, prepare)

    def test_the_tail_holds_its_write_scopes_and_nothing_more(self):
        def grants(name):
            return set(re.findall(r'^      ([a-z-]+): write$', header(self.jobs[name]), re.M))
        for name in TAIL[1:10]:
            # verify-wheels attests nothing (the wheels were attested where they were built):
            # pypi-pack reads attestations and writes none.
            self.assertEqual(grants(name), {'id-token'} if name == 'pypi-pack' else {'id-token', 'attestations'}, name)
        self.assertIn('      attestations: read\n', header(self.jobs['pypi-pack']))
        self.assertEqual(grants('forge'), set())
        self.assertEqual(grants('codeql'), {'actions'})
        self.assertEqual(grants('stage-release'), {'contents'})

    def test_codeql_is_dispatched_on_this_run_and_must_leave_no_open_alert(self):
        step = self.jobs['codeql']
        for needle in ('actions/workflows/codeql.yml/dispatches', 'ref=master', 'inputs[ci_run_id]=${{ github.run_id }}',
                       'X-GitHub-Api-Version: 2026-03-10', '.workflow_run_id', 'gh run watch', '--exit-status',
                       'code-scanning/alerts?state=open&ref=refs/heads/master', 'security-events: read',
                       # The dispatched run must have scanned this commit: master must still be A.
                       '--jq .head_sha', '"$scanned" = "$GITHUB_SHA"',
                       # The alert count is jq and wc alone; no calculator the image may lack.
                       "--jq '.[].number' | wc -l"):
            with self.subTest(needle=needle):
                self.assertIn(needle, step)
        self.assertNotIn('| bc', step)

    def test_pr_board_needs_every_job_but_itself(self):
        board = self.jobs['pr-board']
        self.assertEqual(self.needs(board), set(self.jobs) - {'pr-board'})
        self.assertEqual(self.gate(board), 'always()')
        self.assertIn('    permissions:\n      contents: read\n', header(board))
        self.assertIn('persist-credentials: false', board)
        self.assertEqual(list(self.jobs)[-1], 'pr-board')

    def test_changes_job_wiring(self):
        job = self.jobs['changes']
        self.assertEqual(self.needs(job), set())
        self.assertIn('    runs-on: ubuntu-26.04\n', header(job))
        self.assertIn('    permissions:\n      contents: read\n      pull-requests: read\n', header(job))
        self.assertIn('persist-credentials: false', job)
        outputs = re.findall(r'^      ([a-z_]+): \$\{\{ steps\.classify\.outputs\.([a-z_]+) \}\}$', header(job), re.M)
        self.assertEqual([a for a, _ in outputs], ['core', 'ci', 'docs_only', 'csharp', 'java', 'python', 'ruby', 'php',
                                                   'swift', 'typescript', 'r', 'go'])
        self.assertTrue(all(a == b for a, b in outputs))
        step = next(step for step in re.split(r'\n      - ', job)[1:] if 'id: classify' in step)
        for needle in ('set -euo pipefail', '--paginate', 'tee -a "$GITHUB_OUTPUT"', 'python3 .github/scripts/pr_changes.py',
                       'GH_TOKEN: ${{ github.token }}', 'PR: ${{ github.event.pull_request.number }}'):
            with self.subTest(needle=needle):
                self.assertIn(needle, step)
        self.assertIn('--all', step.split('else')[1])
        self.assertNotIn('--all', step.split('else')[0])
        self.assertNotIn('GH_TOKEN', header(job), "the token is the step's, not the job's")

    def test_pr_board_wiring(self):
        board = self.jobs['pr-board']
        self.assertEqual(self.gate(board), 'always()')
        self.assertEqual(self.needs(board), set(self.jobs) - {'pr-board'})
        self.assertIn('persist-credentials: false', board)
        step = next(step for step in re.split(r'\n      - ', board)[1:] if 'pr_board.py' in step)
        self.assertIn('NEEDS: ${{ toJSON(needs) }}', step)
        self.assertIn('CHANGES: ${{ toJSON(needs.changes.outputs) }}', step)
        self.assertIn('run: python3 .github/scripts/pr_board.py', step)

    def test_one_simulator_boot_on_the_full_board(self):
        # The C archives' Apple mobile smoke runs in test-apple-mobile's simulator, not a second
        # one of its own: one boot on the whole board, after the packed archives are in.
        text = (WORKFLOWS / 'ci.yml').read_text()
        self.assertEqual(text.count('xcrun simctl boot '), 1)
        apple = self.jobs['test-apple-mobile']
        self.assertIn('pack-c', self.needs(apple))
        steps = re.split(r'\n      - ', apple)[1:]
        names = [step.splitlines()[0] for step in steps]
        download = next(i for i, step in enumerate(steps) if 'name: c-packages-ci' in step)
        boot = next(i for i, n in enumerate(names) if n == 'name: Boot an iPhone simulator')
        smoke = next(i for i, n in enumerate(names) if n == 'name: C — the four Apple mobile archives, built against and run')
        self.assertLess(download, smoke)
        self.assertLess(boot, smoke)
        self.assertIn('c/smoke/apple-mobile.sh unpacked "$VERSION"', steps[smoke])
        self.assertIn('${{ !cancelled() }}', steps[smoke])
        self.assertNotIn('smoke-apple-mobile', text)

    def test_runner_labels_are_explicit(self):
        text = (WORKFLOWS / 'ci.yml').read_text()
        for name, job in self.jobs.items():
            for value in re.findall(r'^    runs-on: (.*)$', job, re.M):
                with self.subTest(job=name, runs_on=value):
                    if value == '${{ matrix.os }}':
                        values = re.search(r'^        os: \[(.*)\]$', job, re.M)[1].split(', ')
                    else:
                        values = [value]
                    self.assertLessEqual(set(values) - ALLOWED_EXCEPTIONS, LABELS, values)
        self.assertNotRegex(text, r'runs-on: .*latest')


class PinningTests(unittest.TestCase):
    def test_every_forge_call_follows_master_and_every_third_party_action_is_pinned(self):
        for path in sorted(WORKFLOWS.glob('*.yml')):
            for line in path.read_text().splitlines():
                match = re.search(r'uses:\s*(\S+)(.*)$', line)
                if not match or match[1].startswith('./'):
                    continue
                reference, rest = match[1], match[2]
                with self.subTest(file=path.name, uses=reference):
                    if reference.startswith('SkunkWerkx/.github/'):
                        self.assertTrue(reference.endswith('@master'))
                    else:
                        self.assertRegex(reference, r'@[0-9a-f]{40}$')
                        self.assertRegex(rest, r'# (v\d|stable|master)')


@unittest.skipUnless(FORGE.is_dir(), 'needs the HyperForge checkout beside this repository')
class ForgeInterfaceTests(unittest.TestCase):
    """Every Forge action and reusable workflow this repository calls exists there, and every
    `with:` key is one it declares. Local only: CI has no Forge checkout in this job."""
    def declared_inputs(self, text):
        match = re.search(r'^(?:    |)inputs:\n((?:(?:      |  )\S.*\n|\s*\n|.*\n)*?)(?=^(?:  |)[a-z]|^\S)', text, re.M)
        indent = '      ' if '\n    inputs:' in text else '  '
        return set(re.findall(rf'^{indent}([\w-]+):\s*$', text, re.M)) if match else set()

    def test_attesting_set_is_current(self):
        called = set()
        for path in sorted(WORKFLOWS.glob('*.yml')):
            called |= {workflow for workflow, _ in FORGE_WORKFLOW.findall(path.read_text())}
        composites = ('musl-native', 'ruby-magnus', 'ruby-magnus-musl', 'static-libs',
                      'prepare-crate-package', 'prepare-maven-package')
        attesting = set()
        for workflow in called:
            body = (FORGE / '.github/workflows' / workflow).read_text().split('\njobs:')[1]
            if re.search(r'^ +- name: Attest|uses: actions/attest', body, re.M) or any(f'actions/{c}\n' in body for c in composites):
                attesting.add(workflow)
        ci_called = {w for w, _ in FORGE_WORKFLOW.findall((WORKFLOWS / 'ci.yml').read_text())}
        self.assertEqual(attesting & ci_called, ATTESTING & ci_called)

    def test_calls_match_declared_inputs(self):
        for path in sorted(WORKFLOWS.glob('*.yml')):
            text = path.read_text()
            for name, job in jobs(path).items():
                for workflow, ref in FORGE_WORKFLOW.findall(job):
                    target = FORGE / '.github/workflows' / workflow
                    with self.subTest(file=path.name, job=name, workflow=workflow):
                        self.assertTrue(target.is_file(), f'{workflow} is not in the Forge')
                        declared = set(re.findall(r'^      ([\w-]+):\s*$', target.read_text().split('\njobs:')[0], re.M))
                        self.assertLessEqual(with_block(job), declared)
                for step in re.split(r'\n      - ', job)[1:]:
                    action = re.search(r'uses: \./\.hyperforge/\.github/actions/([\w-]+)', step)
                    if not action:
                        continue
                    target = FORGE / '.github/actions' / action[1] / 'action.yml'
                    with self.subTest(file=path.name, job=name, action=action[1]):
                        self.assertTrue(target.is_file(), f'{action[1]} is not in the Forge')
                        declared = set(re.findall(r'^  ([\w-]+):\s*$', target.read_text().split('\nruns:')[0].split('\ninputs:')[1], re.M))
                        self.assertLessEqual(with_block(step), declared)
                        required = set(re.findall(r'^  ([\w-]+):\n(?:    .*\n)*?    required: true', target.read_text().split('\nruns:')[0], re.M))
                        self.assertLessEqual(required, with_block(step))


if __name__ == '__main__':
    unittest.main()
