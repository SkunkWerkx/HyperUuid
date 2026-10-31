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
PACKERS = ('nuget-org-pack', 'ruby-pack', 'pypi-pack', 'npm-pack', 'r-pack', 'c-pack', 'crate-pack', 'maven-pack')
ACTION_JOBS = {'find-ci-run': 'release-guard', 'ruby-pack': 'pack-gems', 'pypi-pack': 'verify-wheels',
               'npm-pack': 'pack-npm', 'r-pack': 'pack-r', 'c-pack': 'pack-c',
               'publication-gate': 'release-gate', 'nuget-org-publish': 'publish-nuget',
               'rubygems-publish': 'publish-rubygems', 'pypi-publish': 'publish-pypi',
               'npm-publish': 'publish-npm', 'finalize-release': 'finalize-release'}
FORGE_WORKFLOW = re.compile(r'SkunkWerkx/\.github/\.github/workflows/([\w-]+\.yml)@(\S+)')
PUBLISH_GATE = "publish: ${{ github.event_name == 'workflow_dispatch' }}"


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

    def test_tag_push_prepares_and_only_a_dispatch_publishes(self):
        self.assertIn('    tags: ["v*"]', self.text)
        self.assertIn('prepared_run_id:', self.text)
        self.assertIn('cancel-in-progress: false', self.text)
        for publisher in PUBLISHERS:
            with self.subTest(publisher=publisher):
                if publisher == 'pypi-publish':
                    # Gated by its own upload and wait steps (test_pypi_upload_is_a_step_of_this_workflow).
                    self.assertNotIn(PUBLISH_GATE, self.jobs[publisher])
                else:
                    self.assertIn(PUBLISH_GATE, self.jobs[publisher])
        self.assertIn("github.event_name == 'workflow_dispatch'", header(self.jobs['finalize-release']))

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
        self.assertRegex(job, r"if: steps\.pypi\.outputs\.pending == 'true' && github\.event_name == 'workflow_dispatch'\n"
                              r" +uses: pypa/gh-action-pypi-publish@[0-9a-f]{40} # v[0-9.]+\n +with:\n +packages-dir: pending-pypi\n")
        self.assertRegex(job, r"Verify every public wheel and sdist matches the prepared bytes\n"
                              r" +if: github\.event_name == 'workflow_dispatch'\n"
                              r' +run: python3 "\$FORGE_SCRIPTS/release_inventory\.py" registry-confirm --registry pypi\n')
        self.assertNotIn('pypa/', '\n'.join(j for name, j in self.jobs.items() if name != 'pypi-publish'))

    def test_every_registry_publisher_requires_the_global_gate(self):
        for publisher in PUBLISHERS:
            with self.subTest(publisher=publisher):
                head = header(self.jobs[publisher])
                self.assertIn('needs: [find-ci-run, publication-gate]', head)
                self.assertIn('!cancelled()', head)
                self.assertIn("needs.publication-gate.result == 'success'", head)
        for packer in PACKERS:
            with self.subTest(packer=packer):
                self.assertIn("if: inputs.prepared_run_id == ''", header(self.jobs[packer]))
        # The gate waits for every packer, and on a tag run for each one's success; a publish
        # dispatch skips the packers and takes the preserved packages instead.
        gate = header(self.jobs['publication-gate'])
        gate_needs = set(re.split(r'[\s,]+', re.search(r'^    needs: \[([^]]*)\]', gate, re.M)[1].strip()))
        self.assertEqual(gate_needs, {'find-ci-run', *PACKERS})
        self.assertIn("(inputs.prepared_run_id != '' ||", gate)
        for packer in PACKERS:
            self.assertIn(f"needs.{packer}.result == 'success'", gate)
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
        self.assertIn("github.event_name == 'workflow_dispatch'", head)
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
                 find.index('path: .hyperforge'), find.index('uses: ./.hyperforge/.github/actions/release-guard')]
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
                if job not in ('find-ci-run', 'ruby-pack', 'pypi-pack', 'npm-pack', 'r-pack', 'c-pack'):
                    # The actions that write the inventory or publish record the Forge commit.
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
        for name in ('maven-pack', 'maven-publish'):
            self.assertIn('      java_jni: true\n', self.jobs[name], name)


class PreparationFlowTests(unittest.TestCase):
    def test_prepare_release_is_one_forge_call(self):
        text = (WORKFLOWS / 'prepare-release.yml').read_text()
        job = jobs(WORKFLOWS / 'prepare-release.yml')['prepare-release']
        self.assertIn('uses: SkunkWerkx/.github/.github/workflows/hyper-prepare-release.yml@master', job)
        self.assertIn('version: ${{ inputs.version }}', job)
        self.assertIn('group: release-source-${{ github.repository }}', text)
        self.assertIn('actions: write', job)

    def test_stage_natives_keeps_its_triggers_and_gate(self):
        text = (WORKFLOWS / 'stage-native-binaries.yml').read_text()
        job = jobs(WORKFLOWS / 'stage-native-binaries.yml')['stage-native-binaries']
        ci_name = re.search(r'^name: (.+)$', (WORKFLOWS / 'ci.yml').read_text(), re.M)[1]
        self.assertIn(f'workflows: ["{ci_name}"]', text)
        self.assertIn('uses: SkunkWerkx/.github/.github/workflows/hyper-stage-natives.yml@master', job)
        for clause in ("github.event.workflow_run.conclusion == 'success'",
                       "github.event.workflow_run.event == 'workflow_dispatch'"):
            self.assertIn(clause, header(job))
        self.assertIn('source_sha: ${{ github.event_name == \'workflow_run\' && github.event.workflow_run.head_sha || inputs.head_sha }}', job)
        self.assertIn('event_run_id: ${{ github.event.workflow_run.id || inputs.ci_run_id }}', job)
        self.assertRegex(text, r'\n      ci_run_id:\n(?:        .*\n)*        required: false\n')
        self.assertIn('group: release-source-${{ github.repository }}', text)

    def test_release_mode_ci_on_master_requests_staging_of_its_own_run(self):
        # prepare-release dispatches ci.yml with a GITHUB_TOKEN, and GitHub raises no
        # workflow_run for that run, so the run itself has to ask for staging.
        ci = jobs(WORKFLOWS / 'ci.yml')
        request = ci['request-staging']
        needs = re.search(r'^    needs: \[([^]]*)\]', request, re.M)[1]
        self.assertEqual(set(re.split(r'[\s,]+', needs.strip())), set(ci) - {'request-staging'},
                         'staging is requested only once every other job is green')
        self.assertIn("    if: github.event_name == 'workflow_dispatch' && github.ref == 'refs/heads/master'\n", header(request))
        self.assertNotRegex(header(request), r'always\(\)|failure\(\)|cancelled\(\)')
        self.assertEqual(re.findall(r'^      ([a-z-]+): (\w+)$', header(request), re.M), [('actions', 'write')])
        command = ' '.join(request.split('        run: >-\n')[1].split())
        self.assertEqual(command, 'gh workflow run stage-native-binaries.yml --repo "$GITHUB_REPOSITORY" --ref master '
                                  '-f head_sha="$GITHUB_SHA" -f ci_run_id="$GITHUB_RUN_ID"')

    def test_only_request_staging_may_start_a_workflow(self):
        text = (WORKFLOWS / 'ci.yml').read_text()
        self.assertIn('\npermissions:\n  contents: read\n', text)
        for name, job in jobs(WORKFLOWS / 'ci.yml').items():
            with self.subTest(job=name):
                grants = set(re.findall(r'^      ([a-z-]+): write$', header(job), re.M))
                allowed = {'actions'} if name == 'request-staging' else {'id-token', 'attestations'}
                self.assertLessEqual(grants, allowed)


class CiTests(unittest.TestCase):
    def setUp(self):
        self.jobs = jobs(WORKFLOWS / 'ci.yml')

    def test_native_suites_preserved_and_mobile_dependencies_narrow(self):
        build = self.jobs['build-native']
        # The Forge has none of these inputs: the static, Android and swift-static work is
        # its own workflows', called below, and the PHP extension builds inline.
        for gone in ('static_libs_artifact', 'swift_static_tests', 'android_shared_artifact',
                     'php_extension_schedule', 'apple_mobile', 'android:'):
            self.assertNotIn(gone, build)
        self.assertIn('needs: build-static', self.jobs['test-apple-mobile'])
        self.assertIn('needs: [build-static, build-android]', self.jobs['test-android'])
        self.assertIn('needs: build-static', self.jobs['test-swift-static'])

    def test_php_floor_is_explicit_and_matches_both_composer_files(self):
        # The Forge's php_floor_version default (8.2) serves the siblings; this repo says its own.
        m = re.search(r'^      php_floor_version: "(\d+\.\d+)"$', self.jobs['build-native'], re.M)
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
        self.assertIn('    needs: [build-static, build-native]\n', header(job))
        self.assertIn('    runs-on: ubuntu-26.04\n', header(job))
        self.assertNotRegex(header(job), r'^    if:', 'it runs on pull requests, like test-android')
        self.assertIn('setup-dotnet', job)
        self.assertIn('global-json-file: global.json', job)
        self.assertNotIn('dotnet-version', job)
        for needle in ('HYPERUUID003', 'NU1202', '.nobuild', 'readelf -d'):
            self.assertIn(needle, job)
        request = self.jobs['request-staging']
        needs = re.search(r'^    needs: \[([^]]*)\]', request, re.M)[1]
        self.assertIn('test-nuget-package', re.split(r'[\s,]+', needs.strip()))

    def test_npm_package_job_proves_the_fallback_in_consumers(self):
        job = self.jobs['test-npm-package']
        self.assertIn('    needs: [build-native, build-wasm]\n', header(job))
        self.assertIn('    runs-on: ${{ matrix.os }}\n', header(job))
        self.assertIn('        os: [ubuntu-26.04, macos-26, windows-2025]\n', header(job))
        self.assertIn('      fail-fast: false\n', header(job))
        self.assertNotRegex(header(job), r'^    if:', 'it runs on pull requests, like test-nuget-package')
        for needle in ('npm_package.py" pack', 'NAPI_RS_FORCE_WASI=error npm test', 'npm run typecheck',
                       'node-wasm32-wasip1', 'run.sh', 'setup-qemu-action', 'path: .hyperforge',
                       'repository: SkunkWerkx/.github', 'ref: master', 'persist-credentials: false',
                       'node-version: "26"', 'oven-sh/setup-bun@', 'merge-multiple: false', 'GITHUB_STEP_SUMMARY'):
            with self.subTest(needle=needle):
                self.assertIn(needle, job)
        self.assertNotIn('--allow-missing-natives', job)
        # PR mode has no osx-x64 build (hyper-build-native skips it), so the pack leaves that
        # platform package out by name, as test-c does, and the consumer harness is told which
        # native is absent so its mac-lockfile guard excludes it; release mode skips nothing.
        pack = next(step for step in re.split(r'\n      - ', job)[1:] if 'npm_package.py" pack' in step)
        self.assertIn("MODE: ${{ github.event_name == 'pull_request' && 'pr' || 'release' }}", pack)
        self.assertIn('release) skip=() ;;', pack)
        self.assertIn('pr) skip=(--skip osx-x64) ;;', pack)
        self.assertIn('"${skip[@]}"', pack)
        for step in re.split(r'\n      - ', job)[1:]:
            if 'run.sh' in step:
                self.assertIn("SKIP_NATIVES: ${{ github.event_name == 'pull_request' && 'darwin-x64' || '' }}", step)
        bun = next(step for step in re.split(r'\n      - ', job)[1:] if 'test:bun' in step)
        self.assertIn('continue-on-error: true', bun, "Bun's forced-WASI result is recorded, never a failure")
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
        self.assertIn('      node_wasi: true\n', wasm)
        request = self.jobs['request-staging']
        needs = re.search(r'^    needs: \[([^]]*)\]', request, re.M)[1]
        self.assertIn('test-npm-package', re.split(r'[\s,]+', needs.strip()))

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

    def test_typescript_and_r_legs_are_on(self):
        # The Forge's node and r inputs default false until the siblings carry the bindings.
        self.assertEqual(re.findall(r'^      (node|r): (.*)$', self.jobs['build-native'], re.M),
                         [('node', 'true'), ('r', 'true')])

    def test_go_runs_from_the_translated_core_on_every_leg(self):
        # Go's core is Go source (go/internal/core, translated from the wasm32-wasip1 build):
        # no archive is staged for it anywhere, and the wasm workflow runs its whole suite
        # under TinyGo and stock Go beside the browser build.
        ci = (WORKFLOWS / 'ci.yml').read_text()
        self.assertNotIn('go/staticlib', ci)
        self.assertNotIn('go/staticlib', (WORKFLOWS / 'stage-native-binaries.yml').read_text())
        wasm = self.jobs['build-wasm']
        self.assertIn('      go_wasm_suite: true\n', wasm)
        self.assertIn('      go_tinygo_smoke: go/internal/tinygosmoke\n', wasm)
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

    def test_java_jni_legs_are_on(self):
        # The Forge's java_jni input defaults false; the jar's floor is JDK 21 beside the primary 25.
        build = self.jobs['build-native']
        self.assertIn('      java_jni: true\n', build)
        self.assertIn('      jdk_floor_version: "21"\n', build)

    def test_static_checks_run_in_the_forge_with_this_repositorys_eight_inputs(self):
        core = self.jobs['check-core']
        self.assertIn('uses: SkunkWerkx/.github/.github/workflows/hyper-check-core.yml@master', core)
        self.assertEqual(with_block(core), {'no_std_rustflags', 'extra_features', 'no_panic_command',
                                             'clippy_targets', 'lint_node', 'lint_r', 'big_endian',
                                             'lint_java_jni'})
        self.assertIn('big_endian: s390x-unknown-linux-gnu', core)
        self.assertIn('no_std_rustflags: --cfg getrandom_backend="custom"', core)
        self.assertIn('extra_features: uuid1', core)
        self.assertIn('no_panic_command: cargo build --release --example no_panic --features no-panic,uuid1', core)
        # The second link proves the WebAssembly batch walk, which only that cfg builds on a host.
        self.assertIn(' && RUSTFLAGS="--cfg hyperuuid_words_walk" cargo build --release --example no_panic'
                      ' --features no-panic,uuid1\n', core)
        self.assertIn('clippy_targets: aarch64-linux-android', core)
        self.assertIn('lint_node: true', core)
        self.assertIn('lint_r: true', core)
        self.assertIn('lint_java_jni: true', core)
        inline = [name for name in self.jobs if re.match(r'(check|lint|format)-', name) and name != 'check-core']
        self.assertEqual(inline, [], 'static checks live in hyper-check-core.yml, once')


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
