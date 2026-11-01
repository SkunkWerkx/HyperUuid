# The Release Train Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One `prepare-release.yml` dispatch bumps, builds, packs, attests, preserves and commits the staging commit B; one tag push at B verifies the preserved bytes, signs the release attestation and publishes to six registries idempotently, so a retry is the same workflow dispatched at the tag and the PyPI and npm publisher never changes again.

**Architecture:** The Forge (`../HyperForge`, `SkunkWerkx/.github`) gains four composites (`release-prepare`, `stage-release`, `tag-guard`, `tag-gate`), two script helpers and an identity split in `release_inventory.py`, all additive: `release-guard`, `release-gate` and the old stager keep their shape for the frozen siblings. This repository moves the eight pack jobs from `release.yml` into `ci.yml`'s release-mode tail, rewrites `release.yml` around the two new composites, deletes `stage-native-binaries.yml`, and stages the preparation run's Swift core for CodeQL.

**Tech Stack:** GitHub Actions composite actions and reusable workflows, Python 3 scripts with `unittest` (the Forge's `yaml_lite`), bash, `gh`.

**Spec:** `docs/superpowers/specs/2026-11-01-release-train-design.md`

Part A (Tasks 1–9) is in `../HyperForge` on a branch named `release-train`; Part B (Tasks 10–15) is in this repository on a branch named `release-train`. Land order: the Forge PR merges first (the consumer's contract tests read the Forge checkout beside it); this repository's PR second, with the uncommitted work already in its working tree (the 1.1.0 changelog, the README sweep, the npm scoped root) committed on the same branch before the train's commits.

## Global Constraints

- The owner drives git: commit on the branch as the steps say, never push, tag, open a pull request or dispatch a workflow.
- Additive only in the Forge: `release-guard`, `release-gate`, `hyper-stage-natives.yml`, every publisher and every existing input, default and step stay as they are. HyperCast and HyperTabular call them at master.
- Every third-party action is pinned to a commit with the version in a trailing comment; copy the pins from the file being edited (`actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333 # v8.0.2`, `actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2`, `actions/attest-build-provenance@4d101475d8b20a2381f78447822ac1eab6504dd8 # v4.2.2`, `actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6 # v4.2.2`, `actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1`).
- Forge calls from this repository's workflows are `@master`; `.hyperforge` checkouts use the resolved revision.
- The PyPI upload (`pypa/gh-action-pypi-publish`) stays a step of `release.yml` itself, never inside a Forge action (actions/runner#2473). `npm publish` stays inside the Forge's `publish-npm` composite: a composite keeps the caller's OIDC identity, so npm's provenance already names `release.yml`, as RubyGems' did at 1.0.0.
- Nothing is ever re-attested, repacked, moved or overwritten: the tag push makes the release attestation once; a dispatch verifies it.
- The inventory's preparation identity: `package_workflow` is `.github/workflows/ci.yml`, `package_run_id` equals `ci_run_id`, `source_sha` is A, every entry's `source_sha` is A, no `staging_sha`. Today's shape (`staging_sha`, no `package_workflow`) validates exactly as before.
- Runner labels in `ci.yml` are explicit: `ubuntu-26.04` for every new job.
- The release attestation predicate type is `https://github.com/SkunkWerkx/.github/predicates/release/v1`; `release-packages` is preserved for 90 days.
- Forge tests: `cd ../HyperForge && find .github -name __pycache__ -exec rm -rf {} + ; LLVM_BIN=/opt/homebrew/opt/llvm/bin python3 -B -m unittest discover -s .github/scripts -p 'test_*.py'` (370 today, 3 skipped). This repository's: `python3 -B -m unittest discover -s .github/scripts/tests -p 'test_*.py'` (127 today). `actionlint` on every workflow edited.
- Commit messages carry `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>` as their last line.

## Review Focus

- A `ci.yml` run on `schedule` (the weekly board) must not run the tail and must stay green on `pr-board`: the gate is a master dispatch only, and `pr_board.py` must gate the tail off on schedule (Task 10's `test_the_tail_is_gated_off_on_a_schedule`).
- A tag pushed at A instead of B, or at B after a second preparation moved master, must fail in the guard before any download: `release_record`'s parent and trailer checks (Task 7's refusal test).
- A preserved inventory whose `forge_sha` differs from B's `Release-Forge-SHA`, or is not an ancestor of the Forge the tag run resolved, must fail the gate before the release attestation is signed, with a message naming both commits (Task 7's two forge tests; Task 8's step order).
- A `codeql.yml` run with no `ci_run_id` on a master whose newest release-mode run has expired artifacts must fail with a clear message, not build against nothing (Task 13's test pins the lookup and the download's `if-no-files-found`-equivalent: the unpack step fails on an empty directory).
- A publisher re-run after a partial publication must find receipts in the tag run itself (`publication_run_id` is the earlier tag-run attempt): `package_preparation.require_unattempted_publication` is bound to `release.yml` at B, which the new flow keeps (Task 12 asserts every publisher passes `publication_run_id: ${{ inputs.publication_run_id }}` and no `prepared_run_id` remains).

---

## Part A: the Forge

### Task 1: The inventory's two identities

**Files:**
- Modify: `../HyperForge/.github/scripts/release_inventory.py` (IDENTITY block lines 56–61, `validate_identity` 75–90, `build_inventory` 191–216, `validate_inventory` 218–224, `verify_release_attestation` 236–262, `validate_package_run` 265–276, `registry_plan` nuget receipt 393–398, `main` 500–554)
- Test: `../HyperForge/.github/scripts/test_release_inventory.py`

**Interfaces:**
- Produces: module constants `CI_WORKFLOW = '.github/workflows/ci.yml'`, `RELEASE_WORKFLOW = '.github/workflows/release.yml'`; `IDENTITY` gains `'package_workflow'`; `normalize_identity(identity) -> dict` (a copy with `package_workflow` defaulting to `RELEASE_WORKFLOW`, so every caller that builds today's identity without the key, the siblings' actions and the existing tests included, is unchanged); `staging_commit(inventory) -> str` (the inventory's `staging_sha`, else `os.environ['STAGING_SHA']`); `inventory_commit(inventory) -> str` (the commit its attestation was signed at: `source_sha` when `package_workflow` is `CI_WORKFLOW`, else `staging_sha`); `validate_package_run(run, identity, current_run_id='')` now reads `identity['package_workflow']`; the `prepared` CLI command (Task 8 consumes it).

- [ ] **Step 1: Write the failing tests** in `test_release_inventory.py`, class `InventoryTests` (its `setUp` builds `self.identity` with `staging_sha`; copy and adapt):

```python
def preparation_identity(self):
    return {**self.identity, 'package_workflow': self.m.CI_WORKFLOW,
            'package_run_id': self.identity['ci_run_id'], 'staging_sha': self.identity['source_sha']}

def test_preparation_inventory_carries_no_staging_sha_and_every_file_at_a(self):
    result = self.m.build_inventory(self.root, self.preparation_identity(), [], self.workspace)
    self.assertNotIn('staging_sha', result)
    self.assertEqual(result['package_workflow'], self.m.CI_WORKFLOW)
    self.assertEqual(result['package_run_id'], self.identity['ci_run_id'])
    self.assertTrue(all(e['source_sha'] == self.identity['source_sha'] for e in result['files']))

def test_todays_inventory_shape_is_unchanged(self):
    result = self.build()                      # self.identity has no package_workflow, as every caller today
    self.assertNotIn('package_workflow', result)
    self.assertEqual(result['staging_sha'], self.identity['staging_sha'])
    self.assertEqual(self.m.normalize_identity(self.identity)['package_workflow'], self.m.RELEASE_WORKFLOW)
    self.m.validate_identity(self.identity)    # the exact-key check takes the legacy dict

def test_preparation_inventory_verifies_against_its_own_forge_and_run_across_drift(self):
    saved = self.m.build_inventory(self.root, self.preparation_identity(), [], self.workspace)
    drifted = {**self.preparation_identity(), 'forge_sha': 'f' * 40, 'package_run_id': '999'}
    self.m.validate_inventory(self.root, saved, drifted, self.workspace)  # forge and run come from the inventory
    for key in ('repository', 'version', 'source_sha', 'ci_run_id'):
        with self.subTest(key=key), self.assertRaises(ValueError):
            self.m.validate_inventory(self.root, saved, {**drifted, key: 'x' * 40 if key.endswith('sha') else '1.2.3' if key == 'version' else 'Other/Repo' if key == 'repository' else '7'}, self.workspace)

def test_package_run_check_follows_the_inventory_workflow(self):
    identity = self.preparation_identity()
    run = dict(id=int(identity['ci_run_id']), head_sha=identity['source_sha'], head_branch='master',
               path=self.m.CI_WORKFLOW, event='workflow_dispatch', status='completed',
               repository={'full_name': identity['repository']})
    self.m.validate_package_run(run, identity)
    for bad in ({'path': self.m.RELEASE_WORKFLOW}, {'head_sha': 'b' * 40}, {'event': 'schedule'}):
        with self.subTest(bad=bad), self.assertRaises(ValueError):
            self.m.validate_package_run({**run, **bad}, identity)

def test_staging_commit_comes_from_the_identity_when_the_inventory_has_none(self):
    with unittest.mock.patch.dict(os.environ, {'STAGING_SHA': 'b' * 40}):
        self.assertEqual(self.m.staging_commit({'package_workflow': self.m.CI_WORKFLOW}), 'b' * 40)
        self.assertEqual(self.m.staging_commit({'staging_sha': 'c' * 40}), 'c' * 40)
    self.assertEqual(self.m.inventory_commit({'package_workflow': self.m.CI_WORKFLOW, 'source_sha': 'a' * 40}), 'a' * 40)
    self.assertEqual(self.m.inventory_commit({'staging_sha': 'c' * 40}), 'c' * 40)
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cd ../HyperForge && python3 -B -m unittest discover -s .github/scripts -p 'test_release_inventory.py' -k preparation -k todays -k package_run_check -k staging_commit 2>&1 | tail -5`
Expected: FAIL / ERROR with `AttributeError: ... has no attribute 'CI_WORKFLOW'` and `KeyError: 'package_workflow'`.

- [ ] **Step 3: Implement**

In `release_inventory.py`: add the two constants beside `FORGE`; `IDENTITY = (..., 'forge_sha', 'package_workflow')`; `normalize_identity` as the Interfaces block says, called first by `validate_identity` and `build_inventory` (both work on the normalized copy); `validate_identity` accepts `package_workflow` in `(CI_WORKFLOW, RELEASE_WORKFLOW)` and requires `staging_sha` to be a 40-hex only when `package_workflow == RELEASE_WORKFLOW`. `build_inventory`: when `package_workflow == CI_WORKFLOW`, the inventory has `package_workflow` and no `staging_sha`, and every entry's `source_sha` is `identity['source_sha']`; otherwise today's serialized shape, byte for byte (no `package_workflow` key). `validate_inventory`: when `'package_workflow' in saved`, rebuild with `{**identity, 'forge_sha': saved['forge_sha'], 'package_run_id': saved['package_run_id'], 'package_workflow': saved['package_workflow']}`. `validate_package_run`: `workflow = identity.get('package_workflow') or RELEASE_WORKFLOW`; expected `path=workflow`, `head_sha = identity['source_sha'] if workflow == CI_WORKFLOW else identity['staging_sha']`; for `CI_WORKFLOW` also `head_branch='master'` and `event == 'workflow_dispatch'`. `staging_commit` and `inventory_commit` as the Interfaces block says; `verify_release_attestation` uses `staging_commit(inventory)`; the nuget receipt comparison and its `--source-digest` use `staging_commit(inventory)`; the `nuget-receipt` command writes `staging_sha=staging_commit(saved)`. In `main`: `identity = {key: os.environ.get(key.upper(), '') for key in IDENTITY}` then `identity['package_workflow'] = identity['package_workflow'] or RELEASE_WORKFLOW`; the `verify` branch sets `identity['package_workflow'] = inventory.get('package_workflow', RELEASE_WORKFLOW)` and, for a preparation inventory, `identity['forge_sha'] = inventory['forge_sha']` before `validate_inventory`, and verifies `inventory.json`'s attestation with `--source-digest inventory_commit(inventory)`. Add a `prepared` command: everything `verify` does except `verify_release_attestation` (the tag gate runs it before the attestation exists); it honours `--inventory-only` like `verify`, and the gate calls it without that flag.

- [ ] **Step 4: Run the inventory tests, then the Forge suite**

Run: `cd ../HyperForge && find .github -name __pycache__ -exec rm -rf {} + ; LLVM_BIN=/opt/homebrew/opt/llvm/bin python3 -B -m unittest discover -s .github/scripts -p 'test_*.py' 2>&1 | tail -3`
Expected: `OK (skipped=3)` with 375 tests; every existing test (the old shape) still passes.

- [ ] **Step 5: Commit**

```bash
cd ../HyperForge && git add .github/scripts/release_inventory.py .github/scripts/test_release_inventory.py
git commit -m "release_inventory: a preparation identity beside the publication identity

An inventory built in ci.yml at A carries package_workflow and no staging_sha; it verifies
against its own forge_sha and run, its attestation against A, and the staging commit comes
from the running identity. Today's shape takes every path it took."
```

### Task 2: `release-identity` learns the package run and workflow

**Files:**
- Modify: `../HyperForge/.github/actions/release-identity/action.yml`
- Test: `../HyperForge/.github/scripts/test_forge_actions.py` (class `ActionFileTests`)

**Interfaces:**
- Produces: optional inputs `package_run_id` (default `${{ github.run_id }}`) and `package_workflow` (default `.github/workflows/release.yml`); exports `PACKAGE_RUN_ID` and `PACKAGE_WORKFLOW` to `GITHUB_ENV`.

- [ ] **Step 1: Write the failing test**

```python
def test_release_identity_defaults_to_the_running_release_workflow(self):
    text = (ACTIONS / 'release-identity' / 'action.yml').read_text()
    action = yaml_lite.load(text)
    self.assertEqual(action['inputs']['package_run_id']['default'], '${{ github.run_id }}')
    self.assertEqual(action['inputs']['package_workflow']['default'], '.github/workflows/release.yml')
    self.assertFalse(action['inputs']['package_run_id'].get('required', False))
    self.assertIn('echo "PACKAGE_RUN_ID=$PACKAGE_RUN_ID"', text)
    self.assertIn('echo "PACKAGE_WORKFLOW=$PACKAGE_WORKFLOW"', text)
    self.assertNotIn('PACKAGE_RUN_ID=$GITHUB_RUN_ID', text)
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd ../HyperForge && python3 -B -m unittest discover -s .github/scripts -p 'test_forge_actions.py' -k running_release_workflow 2>&1 | tail -3`
Expected: FAIL with `KeyError: 'package_run_id'`.

- [ ] **Step 3: Implement**: the two inputs with those defaults and descriptions ("The run that packed; the release run itself unless the inventory was built in ci.yml at A" and "The workflow that packed: release.yml at B, or ci.yml at A"), passed into the step's `env` and echoed in place of `PACKAGE_RUN_ID=$GITHUB_RUN_ID`. The description's first paragraph gains PACKAGE_WORKFLOW.

- [ ] **Step 4: Run the Forge suite** (the Global Constraints command). Expected: `OK (skipped=3)`, 376 tests.

- [ ] **Step 5: Commit** `git add .github/actions/release-identity/action.yml .github/scripts/test_forge_actions.py && git commit -m "release-identity: optional package_run_id and package_workflow, defaulting to the running release.yml"`.

### Task 3: Settle the release headings in the prepare workflow

**Files:**
- Create: `../HyperForge/.github/scripts/release_headings.py`
- Modify: `../HyperForge/.github/workflows/hyper-prepare-release.yml` (after "Bump every committed manifest to the version"; the push step's `git add` list)
- Test: `../HyperForge/.github/scripts/test_release_headings.py` (new), `../HyperForge/.github/scripts/test_release_guard.py` (class `DispatchTests`)

**Interfaces:**
- Produces: `rename_unreleased(text: str, version: str, date: str) -> str` (turns the first `## [Unreleased]` heading line into `## [{version}] — {date}`; returns `text` unchanged when `## [{version}]` already heads a section; raises `ValueError` when neither heading exists), `rename_news(text: str, crate: str, version: str) -> str` (the first line matching `^# {crate} \S+$` becomes `# {crate} {version}`; unchanged when it already reads so; `ValueError` otherwise), `settle(root: Path, crate: str, version: str, date: str) -> list[str]` (rewrites `CHANGELOG.md` and, when present, `r/NEWS.md`; then runs `release_guard.changelog_section` and the `⟨` placeholder check on the result, raising `ValueError` as `require_changelog` does; returns the paths it changed). CLI: `python3 release_headings.py --version X.Y.Z [--date YYYY-MM-DD]` from the consumer checkout, reading the crate from `hyper_config`.

- [ ] **Step 1: Write the failing tests** (`test_release_headings.py`, loading the module as `test_release_guard.py` loads `release_guard`):

```python
def test_unreleased_becomes_the_dated_version_heading_once(self):
    text = '# Changelog\n\n## [Unreleased]\n\n### Added\n\n- A thing.\n\n## [1.0.0] — 2026-10-20\n'
    once = headings.rename_unreleased(text, '1.1.0', '2026-11-01')
    self.assertIn('\n## [1.1.0] — 2026-11-01\n', once)
    self.assertNotIn('Unreleased', once)
    self.assertEqual(headings.rename_unreleased(once, '1.1.0', '2026-11-02'), once)
    with self.assertRaisesRegex(ValueError, 'no \\[Unreleased\\] or \\[1.2.0\\] section'):
        headings.rename_unreleased(once, '1.2.0', '2026-11-02')

def test_news_heading_takes_the_version(self):
    self.assertEqual(headings.rename_news('# hyperuuid (development version)\n\n* A thing.\n', 'hyperuuid', '1.1.0'),
                     '# hyperuuid 1.1.0\n\n* A thing.\n')
    self.assertEqual(headings.rename_news('# hyperuuid 1.1.0\n', 'hyperuuid', '1.1.0'), '# hyperuuid 1.1.0\n')
    with self.assertRaises(ValueError):
        headings.rename_news('# Something else\n', 'hyperuuid', '1.1.0')

def test_settle_rewrites_both_files_and_refuses_a_placeholder(self):
    # a temp root with hyper.json (test_hyper_config.write_config), CHANGELOG.md with [Unreleased]
    # holding '- ⟨n⟩ KB', r/NEWS.md with '# hyperuuid (development version)'
    with self.assertRaisesRegex(ValueError, 'placeholder'):
        headings.settle(root, 'hyperuuid', '1.1.0', '2026-11-01')
    # replace the placeholder, settle again: returns ['CHANGELOG.md', 'r/NEWS.md'], both headings renamed;
    # a third call returns [].
```

And in `test_release_guard.py`'s `DispatchTests`:

```python
def test_prepare_release_settles_the_headings_before_the_push(self):
    workflow = SCRIPTS.parent / 'workflows/hyper-prepare-release.yml'
    text = workflow.read_text()
    bump, settle, push = (text.index(f'      - name: {n}\n') for n in
                          ('Bump every committed manifest to the version', 'Settle the release headings', 'Commit and push if anything actually changed'))
    self.assertEqual([bump, settle, push], sorted([bump, settle, push]))
    self.assertIn('release_headings.py" --version "$VERSION"', step(workflow, 'Settle the release headings'))
    self.assertIn('git add CHANGELOG.md', step(workflow, 'Commit and push if anything actually changed'))
    self.assertIn('if [ -f r/NEWS.md ]; then git add r/NEWS.md; fi', step(workflow, 'Commit and push if anything actually changed'))
```

- [ ] **Step 2: Run them to verify they fail**. Run: `cd ../HyperForge && python3 -B -m unittest discover -s .github/scripts -p 'test_release_headings.py' 2>&1 | tail -3; python3 -B -m unittest discover -s .github/scripts -p 'test_release_guard.py' -k settles 2>&1 | tail -3`. Expected: `ModuleNotFoundError`/`FileNotFoundError` for the script; `ValueError: substring not found` for the workflow step.

- [ ] **Step 3: Implement** the script (date defaults to `datetime.date.today().isoformat()`; the em dash is `—`; `settle` prints each changed path) and the workflow step:

```yaml
      # The heading that was [Unreleased] becomes this version's, dated today, and r/NEWS.md's
      # heading matches; both are no-ops when the owner renamed them by hand. The section must
      # exist and hold no ⟨placeholder⟩: the finalizer publishes it verbatim, so the dispatch
      # fails here, with nothing built and nothing pushed, rather than at the tag.
      - name: Settle the release headings
        env:
          VERSION: ${{ inputs.version }}
          FORGE_SCRIPTS: .hyperforge/.github/scripts
        run: python3 "$FORGE_SCRIPTS/release_headings.py" --version "$VERSION"
```

The push step adds `git add CHANGELOG.md` and `if [ -f r/NEWS.md ]; then git add r/NEWS.md; fi` before the `git diff --cached --quiet` check. The header comment gains one sentence on the headings.

- [ ] **Step 4: Run the Forge suite**. Expected: `OK (skipped=3)`, 380 tests.

- [ ] **Step 5: Commit** `git add .github/scripts/release_headings.py .github/scripts/test_release_headings.py .github/workflows/hyper-prepare-release.yml .github/scripts/test_release_guard.py && git commit -m "prepare-release: settle the CHANGELOG and r/NEWS.md headings at the version before the push"`.

### Task 4: `swift_release.py unpack`

**Files:**
- Modify: `../HyperForge/.github/scripts/swift_release.py` (after `stage`; `main`'s subcommands)
- Test: `../HyperForge/.github/scripts/test_swift_release.py`

**Interfaces:**
- Produces: `unpack(cfg, root, assets) -> list[Path]`: extracts each of `cfg.swift_assets` from `assets/` into `root / Path(tree).parent` (so `swift/HyperUuidCore.artifactbundle/...` lands under `swift/`), refusing a zip whose member names do not all start with `Path(tree).name + '/'` or that contain `..`, and refusing an existing tree; afterwards the extracted files must equal `expected_files(cfg)[asset]` exactly. CLI: `swift_release.py unpack --assets <dir>` from the consumer checkout.

- [ ] **Step 1: Write the failing test** beside the existing zip tests (reuse their fixture that builds a complete bundle and XCFramework tree, zips them with `build_zips`, then deletes the trees):

```python
def test_unpack_restores_the_release_trees_exactly(self):
    built = swift_release.build_zips(cfg, root, out)
    shutil.rmtree(root / cfg.bundle); shutil.rmtree(root / cfg.xcframework)
    swift_release.unpack(cfg, root, out)
    present = {p.relative_to(root).as_posix() for tree in (cfg.bundle, cfg.xcframework) for p in (root / tree).rglob('*') if p.is_file()}
    self.assertEqual(present, {f for _, files in swift_release.expected_files(cfg).values() for f in files})
    with self.assertRaisesRegex(ValueError, 'already'):
        swift_release.unpack(cfg, root, out)
    # a zip with a member outside its tree is refused before anything is written
```

- [ ] **Step 2: Run it to verify it fails**. Run: `cd ../HyperForge && python3 -B -m unittest discover -s .github/scripts -p 'test_swift_release.py' -k unpack 2>&1 | tail -3`. Expected: `AttributeError: module has no attribute 'unpack'`.

- [ ] **Step 3: Implement** `unpack` and the `unpack` subcommand (`--assets`, required).

- [ ] **Step 4: Run the Forge suite**. Expected: `OK (skipped=3)`, 381 tests.

- [ ] **Step 5: Commit** `git add .github/scripts/swift_release.py .github/scripts/test_swift_release.py && git commit -m "swift_release: unpack the release zips into the checkout, for a CodeQL build without a committed core"`.

### Task 5: `release-prepare`, the inventory at A

**Files:**
- Create: `../HyperForge/.github/actions/release-prepare/action.yml`
- Test: `../HyperForge/.github/scripts/test_forge_actions.py` (`RELEASE_ACTIONS` gains `'release-prepare'`; a new test in `ReleaseGateTests`)

**Interfaces:**
- Consumes: Task 1's `create` with `PACKAGE_WORKFLOW=.github/workflows/ci.yml`; Task 2's inputs.
- Produces: composite inputs `version`, `source_sha`, `ci_run_id`, `forge_sha`, `token` (required), `npm_packages`, `cran_packages` (default `"false"`); artifact `release-packages` (90 days) in the calling run.

- [ ] **Step 1: Write the failing test**

```python
def test_release_prepare_builds_attests_and_preserves_without_the_release_attestation(self):
    text = (ACTIONS / 'release-prepare' / 'action.yml').read_text()
    steps = yaml_lite.load(text)['runs']['steps']
    identity = next(s for s in steps if s.get('uses', '').endswith('actions/release-identity'))
    self.assertEqual(identity['with']['package_workflow'], '.github/workflows/ci.yml')
    self.assertEqual(identity['with']['package_run_id'], '${{ inputs.ci_run_id }}')
    names = [s.get('name') or s.get('uses') for s in steps]
    self.assertIn('Verify every final package and create the source/run inventory', names)
    self.assertIn('Attest the complete package inventory', names)
    self.assertNotIn('actions/attest@', text)                      # the release attestation is the tag's
    self.assertNotIn('prepared_run_id', text)                      # nothing to recover here
    for name in ('nuget-org-package', 'ruby-packages', 'pypi-packages', 'crate-package', 'maven-central-package',
                 'npm-packages', 'cran-packages', 'swift-release-assets', 'c-packages'):
        self.assertIn(f'name: {name}\n', text)
    self.assertIn('pattern: php-ext-*', text)
    self.assertNotIn('run-id:', text)                               # every artifact is this run's
    upload = next(s for s in steps if s.get('uses', '').startswith('actions/upload-artifact'))
    self.assertEqual(upload['with']['name'], 'release-packages')
    self.assertEqual(str(upload['with']['retention-days']), '90')
    self.assertIn('release_inventory.py" create', text)
    self.assertIn('release_inventory.py" checksums --output release-checksums.txt', text)
```

- [ ] **Step 2: Run it to verify it fails**. Run: `cd ../HyperForge && python3 -B -m unittest discover -s .github/scripts -p 'test_forge_actions.py' -k release_prepare 2>&1 | tail -3`. Expected: `FileNotFoundError`.

- [ ] **Step 3: Implement** the composite: the first half of `release-gate` (lines 1–131 of its file, every download without `run-id`/`github-token` and without the `prepared_run_id` gates, the `config` step, the create and checksums step, the inventory attestation), then a summary step (`### Release v$VERSION prepared at $SOURCE_SHA (ci.yml run $CI_RUN_ID, Forge $FORGE_SHA)` plus the checksums block and the CRAN note), then the `release-packages` upload. Description: what it does and that the release attestation is the tag's. The existing `StepHandoffTests` run over it automatically.

- [ ] **Step 4: Run the Forge suite**. Expected: `OK (skipped=3)`, 382 tests.

- [ ] **Step 5: Commit** `git add .github/actions/release-prepare .github/scripts/test_forge_actions.py && git commit -m "release-prepare: the inventory, its attestation and release-packages, built in ci.yml at A"`.

### Task 6: `stage-release`, B from the run's own zips

**Files:**
- Create: `../HyperForge/.github/actions/stage-release/action.yml`
- Test: `../HyperForge/.github/scripts/test_forge_actions.py`

**Interfaces:**
- Produces: inputs `source_sha`, `ci_run_id`, `forge_sha`, `token` (all required); commits B on master with `Release-Source-SHA`, `Release-CI-Run-ID`, `Release-Forge-SHA` trailers. Runs in a checkout of `master` with `.hyperforge` beside it.

- [ ] **Step 1: Write the failing test**

```python
def test_stage_release_commits_package_swift_alone_with_the_three_trailers(self):
    text = (ACTIONS / 'stage-release' / 'action.yml').read_text()
    for needle in ('release_guard.py" master', 'name: swift-release-assets', '--signer-repo SkunkWerkx/.github',
                   'swift_release.py" stage --assets', 'git add Package.swift', 'release_guard.py" index',
                   'Release-Source-SHA: $SOURCE_SHA', 'Release-CI-Run-ID: $RUN_ID', 'Release-Forge-SHA: $FORGE_SHA',
                   'git push origin HEAD:master', 'SWIFT_REMOTE" = true'):
        with self.subTest(needle=needle):
            self.assertIn(needle, text)
    self.assertNotIn('STATIC_TREES', text)     # consumers that commit a tree stay on hyper-stage-natives.yml
    self.assertNotIn('run-id:', text)
    self.assertNotIn('RUN_WAIT_SECONDS', text)  # the run is this one; nothing to wait for
```

- [ ] **Step 2: Run it to verify it fails**. Expected: `FileNotFoundError`.

- [ ] **Step 3: Implement**: `require_master` (`git fetch origin master` then `release_guard.py master` with `SOURCE_SHA`), refuse when `SWIFT_REMOTE` is not `true` (`::error::stage-release serves swift_remote consumers; others stage through hyper-stage-natives.yml`), download `swift-release-assets` (this run), verify each zip (`--repo "$GITHUB_REPOSITORY" --signer-repo SkunkWerkx/.github --source-digest "$SOURCE_SHA"`), `swift_release.py stage --assets`, then the commit step lifted from `hyper-stage-natives.yml`'s last step for the `SWIFT_REMOTE` branch, with `FORGE_SHA` from the input and the digest listing of the two zips in the message body. `git config` the bot identity as the stager does.

- [ ] **Step 4: Run the Forge suite**. Expected: `OK (skipped=3)`, 383 tests.

- [ ] **Step 5: Commit** `git add .github/actions/stage-release .github/scripts/test_forge_actions.py && git commit -m "stage-release: B from the run's own attested zips, in the same run"`.

### Task 7: `release_guard.py tag` and the Forge ancestry check

**Files:**
- Modify: `../HyperForge/.github/scripts/release_guard.py` (after `release_record`; `main`'s modes)
- Test: `../HyperForge/.github/scripts/test_release_guard.py`

**Interfaces:**
- Produces: `forge_trailer(message: str) -> str` (exactly one `Release-Forge-SHA`, 40 hex); `require_prepared_release(repo, run_id) -> dict` (the `release-packages` artifact of `actions/runs/{run_id}/artifacts?name=release-packages`, exactly one, `expired` false; `ValueError` naming the run otherwise); `require_forge_ancestry(packed: str, running: str)` (`api('SkunkWerkx/.github', f'compare/{packed}...{running}')['status']` in `('identical', 'ahead')`, else `ValueError(f'the Forge that packed, {packed}, is not an ancestor of the Forge this run uses, {running}')`); `check_swift_assets(root, cfg, manifest, sha=None, repo=None, assets=None)` (the existing function with an optional assets directory in place of `swift_assets_dir(root)`); mode `tag`: `release_record` with the tag required, then `require_prepared_release`, writing `source_sha`, `run_id` and `packing_forge_sha` to `GITHUB_OUTPUT`; mode `forge-ancestry`: reads `PACKED_FORGE_SHA` (B's trailer), `FORGE_SHA` (this run's) and `INVENTORY` (default `release-packages/inventory.json`), requires the inventory's `forge_sha` to equal the trailer's (`ValueError('B names Forge {trailer} but the preserved inventory was packed by {inventory}')`), then the ancestry; mode `swift-assets`: `check_swift_assets` on `Package.swift` at the checkout (B) with `SOURCE_SHA`, `GITHUB_REPOSITORY` and the directory `SWIFT_ASSETS` names, so B's checksums are held to the preserved zips and each zip to its provenance at A.

- [ ] **Step 1: Write the failing tests** in `ReleaseGuardTests` (the class already builds A and B in a temp repository with trailers; add `Release-Forge-SHA` to its staging message fixture where the existing tests allow it):

```python
def test_tag_mode_requires_the_preserved_packages_and_reports_the_packing_forge(self):
    # B's message carries Release-Forge-SHA: <f*40>; api returns a green release-mode run for A and one
    # unexpired release-packages artifact
    ... run guard.main() with GITHUB_REF=refs/tags/v1.0.0, GITHUB_SHA=B, mode 'tag', GITHUB_OUTPUT=<tmp>
    self.assertEqual(outputs, {'source_sha': A, 'run_id': '42', 'packing_forge_sha': 'f' * 40})

def test_tag_mode_refuses_a_run_without_release_packages(self):
    # the artifacts endpoint returns [] or an expired artifact
    with self.assertRaisesRegex(ValueError, 'release-packages'):
        ...

def test_tag_mode_refuses_a_tag_off_b(self):
    # GITHUB_SHA=A (no trailers) and GITHUB_SHA=a commit after B whose parent is B
    with self.assertRaisesRegex(ValueError, 'trailer|exactly one parent'):
        ...

def test_forge_ancestry(self):
    for status, ok in (('identical', True), ('ahead', True), ('behind', False), ('diverged', False)):
        with self.subTest(status=status), patch.object(guard, 'api', return_value={'status': status}):
            if ok:
                guard.require_forge_ancestry('a' * 40, 'b' * 40)
            else:
                with self.assertRaisesRegex(ValueError, 'not an ancestor'):
                    guard.require_forge_ancestry('a' * 40, 'b' * 40)

def test_forge_ancestry_mode_requires_the_trailer_to_name_the_inventory_s_forge(self):
    # INVENTORY points at a json with forge_sha 'a'*40; PACKED_FORGE_SHA 'a'*40 and api 'ahead' passes;
    # PACKED_FORGE_SHA 'c'*40 fails with 'packed by' before api is called
    ...

def test_swift_assets_mode_holds_b_s_manifest_to_the_preserved_zips(self):
    # a checkout whose Package.swift names the two zips' checksums (swift_release.write), SWIFT_ASSETS a
    # directory holding them, verify_attestation patched: passes; a manifest with one checksum changed fails
    # with swift_release.check's error; a missing zip fails
    ...
```

- [ ] **Step 2: Run them to verify they fail**. Run: `cd ../HyperForge && python3 -B -m unittest discover -s .github/scripts -p 'test_release_guard.py' -k tag_mode -k forge_ancestry 2>&1 | tail -3`. Expected: `argparse` rejects mode `tag` (SystemExit) and `AttributeError` for `require_forge_ancestry`.

- [ ] **Step 3: Implement** the three functions, the `assets` parameter and the three modes. `release_record` is unchanged; `tag` calls it with `tag` from `GITHUB_REF` and no override, then `forge_trailer(git(root, 'show', '-s', '--format=%B', commit))`.

- [ ] **Step 4: Run the Forge suite**. Expected: `OK (skipped=3)`, 389 tests.

- [ ] **Step 5: Commit** `git add .github/scripts/release_guard.py .github/scripts/test_release_guard.py && git commit -m "release_guard: the tag mode (B's trailers, the preserved packages, the packing Forge), the Forge agreement and ancestry check, the Swift assets mode"`.

### Task 8: `tag-guard` and `tag-gate`

**Files:**
- Create: `../HyperForge/.github/actions/tag-guard/action.yml`, `../HyperForge/.github/actions/tag-gate/action.yml`
- Test: `../HyperForge/.github/scripts/test_forge_actions.py` (`RELEASE_ACTIONS` gains both)

**Interfaces:**
- Consumes: Task 7's `tag` and `forge-ancestry` modes; Task 1's `prepared` and `verify`; Task 2's inputs.
- Produces: `tag-guard` inputs `forge_sha`, `token`; outputs `run_id`, `source_sha`, `version`, `packing_forge_sha`. `tag-gate` inputs `version`, `source_sha`, `ci_run_id`, `forge_sha`, `packing_forge_sha`, `token` (required), `attest` (`"true"` makes the release attestation, `"false"` verifies it; default `"false"`), `npm_packages`, `cran_packages`; uploads `release-packages`, `verified-crate-package`, `verified-maven-central-package` into the running run.

- [ ] **Step 1: Write the failing tests**

```python
def test_tag_guard_takes_no_overrides_and_binds_the_ci_run_as_the_package_run(self):
    action = yaml_lite.load((ACTIONS / 'tag-guard' / 'action.yml').read_text())
    self.assertEqual(set(action['inputs']), {'forge_sha', 'token'})
    self.assertEqual(set(action['outputs']), {'run_id', 'source_sha', 'version', 'packing_forge_sha'})
    text = (ACTIONS / 'tag-guard' / 'action.yml').read_text()
    self.assertIn('release_guard.py" tag', text)
    self.assertIn('package_workflow: .github/workflows/ci.yml', text)
    self.assertIn('release_inventory.py" run', text)
    self.assertNotIn('download-artifact', text)   # the guard reads git and the API; the gate downloads

def test_tag_gate_verifies_the_preserved_run_before_attesting_from_the_tag_and_never_packs(self):
    text = (ACTIONS / 'tag-gate' / 'action.yml').read_text()
    steps = yaml_lite.load(text)['runs']['steps']
    order = [text.index(n) for n in ('name: release-packages', 'release_inventory.py" prepared --workspace .', 'release_guard.py" swift-assets',
                                     'release_guard.py" forge-ancestry', 'checksums --output release-checksums.txt',
                                     'uses: actions/attest@', 'release_inventory.py" verify --inventory-only')]
    self.assertEqual(order, sorted(order))        # every per-file provenance, B's manifest and the Forge agreement before signing
    self.assertNotIn('prepared --inventory-only', text)
    self.assertIn('SWIFT_ASSETS: release-packages/github', text)
    self.assertIn('PACKED_FORGE_SHA: ${{ inputs.packing_forge_sha }}', text)
    download = next(s for s in steps if s.get('with', {}).get('name') == 'release-packages' and 'download' in s['uses'])
    self.assertEqual(download['with']['run-id'], '${{ inputs.ci_run_id }}')
    attest = next(s for s in steps if s.get('uses', '').startswith('actions/attest@'))
    self.assertEqual(attest['if'], "inputs.attest == 'true'")
    self.assertEqual(attest['with']['predicate-type'], PREDICATE)
    self.assertEqual(attest['with']['predicate-path'], 'release-packages/inventory.json')
    self.assertNotIn('attest-build-provenance', text)       # the inventory was attested at A
    self.assertNotIn('release_inventory.py" create', text)
    uploads = text[text.index('upload-artifact'):]
    for name in ('release-packages', 'verified-crate-package', 'verified-maven-central-package'):
        self.assertIn(f'name: {name}\n', uploads)
```

- [ ] **Step 2: Run them to verify they fail**. Expected: `FileNotFoundError` for both.

- [ ] **Step 3: Implement**. `tag-guard`: the tag-ref step (from `release-guard`'s first step), then `release_guard.py tag` (`GH_TOKEN`, `FORGE_SCRIPTS`), then `release-identity` with `package_run_id: ${{ steps.find.outputs.run_id }}` and `package_workflow: .github/workflows/ci.yml`, then `release_inventory.py run` with `PREPARED_RUN_ID: ${{ steps.find.outputs.run_id }}`. `tag-gate`: `release-identity` (same two inputs from `ci_run_id`), download `release-packages` from `ci_run_id`, `prepared --workspace .` (the full per-file build-provenance pass: `--inventory-only` would skip `attest_files`, and the GitHub Release assets have no publisher to recheck them), `swift-assets` with `SWIFT_ASSETS: release-packages/github` and `SOURCE_SHA` (B's `Package.swift` against the preserved zips), `forge-ancestry` with `PACKED_FORGE_SHA: ${{ inputs.packing_forge_sha }}`, `FORGE_SHA: ${{ inputs.forge_sha }}` and `INVENTORY: release-packages/inventory.json`, `checksums --output release-checksums.txt`, the `actions/attest` step gated on `attest`, `verify --inventory-only --workspace .` (the release attestation, now present on both paths), the summary (as `release-gate`'s, with "packed by Forge $PACKED_FORGE_SHA, published by Forge $FORGE_SHA"), the three uploads (90 days, `overwrite: true`). Permissions are the caller's.

- [ ] **Step 4: Run the Forge suite**. Expected: `OK (skipped=3)`, 389 tests.

- [ ] **Step 5: Commit** `git add .github/actions/tag-guard .github/actions/tag-gate .github/scripts/test_forge_actions.py && git commit -m "tag-guard and tag-gate: act two's guard and gate, beside release-guard and release-gate"`.

### Task 8b: The crate and Maven publishers verify at the preparation source

**Files:**
- Modify: `../HyperForge/.github/workflows/hyper-publish-crate.yml` (inputs; the "verify" step's `--source-digest`), `../HyperForge/.github/workflows/hyper-publish-maven.yml` (same)
- Test: `../HyperForge/.github/scripts/test_release_provenance.py`

**Interfaces:**
- Produces: optional input `source_sha` on both reusable workflows, `default: ${{ github.sha }}`, description "The commit the prepared package's build provenance names: B today, A on the release train"; the per-file `gh attestation verify ... --source-digest "${{ inputs.source_sha }}"` in place of `"$GITHUB_SHA"`. The publication receipts stay bound to the running commit (B).

- [ ] **Step 1: Write the failing test**

```python
def test_crate_and_maven_publishers_verify_at_the_preparation_source_and_receipt_at_b(self):
    for name, count in (('hyper-publish-crate.yml', 3), ('hyper-publish-maven.yml', 4)):
        text = (WORKFLOWS / name).read_text()
        with self.subTest(workflow=name):
            self.assertRegex(text, r'\n      source_sha:\n(?:        .*\n)*        default: \$\{\{ github\.sha \}\}\n')
            self.assertIn('--source-digest "${{ inputs.source_sha }}"', text)
            self.assertNotIn('--source-digest "$GITHUB_SHA"', text)
            self.assertIn('$GITHUB_SHA', text.split('Locate the latest preserved publication receipt')[1])  # receipts stay at B
```

- [ ] **Step 2: Run it to verify it fails**. Run: `cd ../HyperForge && python3 -B -m unittest discover -s .github/scripts -p 'test_release_provenance.py' -k preparation_source 2>&1 | tail -3`. Expected: FAIL at the `source_sha` regex.

- [ ] **Step 3: Implement** the input and the one substitution in each file; the step comment says why the digest is an input.

- [ ] **Step 4: Run the Forge suite**. Expected: `OK (skipped=3)`, one more test than Task 8 left.

- [ ] **Step 5: Commit** `git add .github/workflows/hyper-publish-crate.yml .github/workflows/hyper-publish-maven.yml .github/scripts/test_release_provenance.py && git commit -m "crate and Maven publishers: verify the prepared package at the preparation source it was attested at"`.

### Task 9: The Forge runbook section

**Files:**
- Modify: `../HyperForge/docs/release-runbook.md` (a new `## The release train` section after `## Normal release`, before `## After publication`; the head paragraph's "Hold Forge merges while a release is in flight anywhere" gains "on the old flow; on the train, hold them during preparation only")

- [ ] **Step 1: Write the section**: the two acts, the six hand steps and the retry ladder from the spec's "The hand steps", the composites each act uses, the inventory's two identities in three sentences, and which consumers run it (HyperUuid; the siblings stay on the flow above until they thaw).

- [ ] **Step 2: Verify**: `cd ../HyperForge && grep -c '^## ' docs/release-runbook.md` prints 7; the Forge suite still `OK (skipped=3)` (no test reads the runbook, so this is the negative control's absence: a doc edit).

- [ ] **Step 3: Commit** `git add docs/release-runbook.md && git commit -m "Runbook: the release train beside the old flow"`.

Part A is complete when the Forge suite is green and `actionlint` passes on `hyper-prepare-release.yml`. The owner opens and merges the Forge PR; Part B's contract tests read the Forge checkout at that state.

## Part B: this repository

### Task 10: `pr_board.py` learns the tail

**Files:**
- Modify: `.github/scripts/pr_board.py` (`JOBS`, `gates`, `main`)
- Test: `.github/scripts/tests/test_pr_board.py`

**Interfaces:**
- Produces: `TAIL = ['forge', 'nuget-org-pack', 'crate-pack', 'maven-pack', 'ruby-pack', 'npm-pack', 'r-pack', 'c-pack', 'pypi-pack', 'release-prepare', 'codeql', 'stage-release']`, appended to `JOBS` in that order, with `request-staging` nowhere; `gates(outputs, event, ref='')` gates every tail job on `event == 'workflow_dispatch' and ref == 'refs/heads/master'`; `main` reads `GITHUB_REF`.

- [ ] **Step 1: Write the failing tests** (the module's `outputs()` and `results()` helpers exist):

```python
def test_the_tail_is_gated_on_a_master_dispatch_only(self):
    on = pr_board.gates(outputs(core=True), 'workflow_dispatch', 'refs/heads/master')
    self.assertTrue(all(on[name] for name in pr_board.TAIL))
    for event, ref in (('pull_request', 'refs/pull/1/merge'), ('schedule', 'refs/heads/master'), ('workflow_dispatch', 'refs/heads/topic')):
        with self.subTest(event=event, ref=ref):
            self.assertFalse(any(pr_board.gates(outputs(core=True), event, ref)[name] for name in pr_board.TAIL))

def test_the_tail_is_gated_off_on_a_schedule(self):
    self.assertTrue(pr_board.verdict(results(**{name: 'skipped' for name in pr_board.TAIL}),
                                     pr_board.gates(outputs(core=True), 'schedule', 'refs/heads/master'))[0])

def test_request_staging_is_gone(self):
    self.assertNotIn('request-staging', pr_board.JOBS)
    self.assertEqual(pr_board.JOBS[-len(pr_board.TAIL):], pr_board.TAIL)
```

Update `test_gate_names_are_the_boards_jobs`, `test_release_only_jobs_are_gated_off_on_pull_requests` and `test_gates_mirror_ci_yml` to pass `ref` (read how the last one derives gates from `ci.yml`'s `if:` lines and add the tail's gate `github.event_name == 'workflow_dispatch' && github.ref == 'refs/heads/master'` to its mirror table).

- [ ] **Step 2: Run them to verify they fail**. Run: `python3 -B -m unittest discover -s .github/scripts/tests -p 'test_pr_board.py' 2>&1 | tail -3`. Expected: `AttributeError: ... 'TAIL'` and `TypeError: gates() takes 2 positional arguments`.

- [ ] **Step 3: Implement** in `pr_board.py`; `main` passes `os.environ.get('GITHUB_REF', '')`.

- [ ] **Step 4: Run the tests**. Run: `python3 -B -m unittest discover -s .github/scripts/tests -p 'test_pr_board.py' -k tail -k request_staging -k gate_names -k release_only 2>&1 | tail -3`. Expected: `OK`. `test_gates_mirror_ci_yml` reads `ci.yml`, which Task 11 edits; it is expected red until then and is run there (unittest's `-k` takes substrings only, with no negation).

- [ ] **Step 5: Commit** `git add .github/scripts/pr_board.py .github/scripts/tests/test_pr_board.py && git commit -m "pr_board: the release tail, gated on a master dispatch"`.

### Task 11: `ci.yml`'s release-mode tail

**Files:**
- Modify: `.github/workflows/ci.yml` (replace `request-staging` lines 1281–1302; `pr-board`'s `needs`; the header comment's `request-staging` sentence)
- Modify: `.github/scripts/tests/test_workflow_contracts.py` (`CiTests`; constants `ATTESTING`, `NO_MODE`)
- Delete: `.github/workflows/stage-native-binaries.yml`; delete `test_stage_natives_keeps_its_triggers_and_gate`, `test_release_mode_ci_on_master_requests_staging_of_its_own_run`, `test_only_request_staging_may_start_a_workflow`, `test_request_staging_needs_every_job_but_pr_board`

**Interfaces:**
- Consumes: Forge composites `release-prepare`, `stage-release`; reusable `hyper-pack-nuget.yml`, `hyper-pack-crate.yml`, `hyper-pack-maven.yml`; `codeql.yml`'s `ci_run_id` input (Task 13 adds it; the dispatch here passes it).
- Produces: jobs `forge` (output `revision`), `nuget-org-pack`, `crate-pack`, `maven-pack`, `ruby-pack`, `npm-pack`, `r-pack`, `c-pack`, `pypi-pack`, `release-prepare`, `codeql`, `stage-release`; `BOARD` in the test file = every job but the tail and `pr-board`.

- [ ] **Step 1: Write the failing tests** in `CiTests`:

```python
TAIL = ['forge', 'nuget-org-pack', 'crate-pack', 'maven-pack', 'ruby-pack', 'npm-pack', 'r-pack', 'c-pack', 'pypi-pack',
        'release-prepare', 'codeql', 'stage-release']
TAIL_GATE = "github.event_name == 'workflow_dispatch' && github.ref == 'refs/heads/master'"

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
        self.assertIn('run_id: ${{ github.run_id }}', job); self.assertIn('source_sha: ${{ github.sha }}', job)
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
    grants = lambda name: set(re.findall(r'^      ([a-z-]+): write$', header(self.jobs[name]), re.M))
    for name in TAIL[1:10]:
        self.assertEqual(grants(name), {'id-token', 'attestations'}, name)
    self.assertEqual(grants('forge'), set())
    self.assertEqual(grants('codeql'), {'actions'})
    self.assertEqual(grants('stage-release'), {'contents'})

def test_codeql_is_dispatched_on_this_run_and_must_leave_no_open_alert(self):
    step = self.jobs['codeql']
    for needle in ('actions/workflows/codeql.yml/dispatches', 'ref=master', 'inputs[ci_run_id]=${{ github.run_id }}',
                   'X-GitHub-Api-Version: 2026-03-10', '.workflow_run_id', 'gh run watch', '--exit-status',
                   'code-scanning/alerts?state=open&ref=refs/heads/master', 'security-events: read'):
        with self.subTest(needle=needle):
            self.assertIn(needle, step)

def test_pr_board_needs_every_job_but_itself(self):
    board = self.jobs['pr-board']
    self.assertEqual(self.needs(board), set(self.jobs) - {'pr-board'})
    self.assertEqual(self.gate(board), 'always()')
    self.assertIn('    permissions:\n      contents: read\n', header(board))
    self.assertEqual(list(self.jobs)[-1], 'pr-board')
```

`test_attesting_callers_hold_the_write_scopes`'s `elif name != 'request-staging'` becomes `elif name not in TAIL`; `test_pr_board_wiring`'s needs assertion becomes `set(self.jobs) - {'pr-board'}`; `ATTESTING` gains `'hyper-pack-nuget.yml', 'hyper-pack-crate.yml', 'hyper-pack-maven.yml'`; `NO_MODE` gains the same three. Replace `test_only_request_staging_may_start_a_workflow` with `test_only_codeql_may_start_a_workflow` (allowed grants: `{'actions'}` for `codeql`, `{'contents'}` for `stage-release`, `{'id-token', 'attestations'}` otherwise).

- [ ] **Step 2: Run them to verify they fail**. Run: `python3 -B -m unittest discover -s .github/scripts/tests -p 'test_workflow_contracts.py' -k tail -k codeql_is -k pack_jobs -k pr_board_needs 2>&1 | tail -3`. Expected: FAIL with `KeyError: 'forge'`.

- [ ] **Step 3: Implement** in `ci.yml`. `forge`: `needs` = every board job (the list `request-staging` has today), the gate, `runs-on: ubuntu-26.04`, `permissions: contents: read`, outputs `revision` and `version` (the second from `rust/Cargo.toml` at the checkout: `sed -n 's/^version = "\(.*\)"/\1/p' rust/Cargo.toml`), one checkout with `persist-credentials: false`, the "Resolve the Forge commit this run uses" step copied verbatim from `release.yml`. The eight pack jobs lifted from `release.yml` with `needs: [changes, forge]`, the gate, `.hyperforge` at `${{ needs.forge.outputs.revision }}`, and the inputs the test names. `release-prepare`: needs the eight, `runs-on: ubuntu-26.04`, `permissions: contents: read, actions: read, id-token: write, attestations: write`, the two checkouts, the composite with the test's inputs. `codeql`: `needs: [changes, forge]`, `permissions: actions: write, security-events: read, contents: read`, one step:

```yaml
      - name: Scan this commit and require a clean code scanning dashboard
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          set -euo pipefail
          run_id=$(gh api --method POST -H "X-GitHub-Api-Version: 2026-03-10" \
            "repos/$GITHUB_REPOSITORY/actions/workflows/codeql.yml/dispatches" \
            -f ref=master -f "inputs[ci_run_id]=${{ github.run_id }}" --jq '.workflow_run_id')
          [[ "$run_id" =~ ^[1-9][0-9]*$ ]] || { echo "::error::codeql.yml dispatch returned no run id"; exit 1; }
          gh run watch "$run_id" --repo "$GITHUB_REPOSITORY" --exit-status --interval 60
          open=$(gh api --paginate "repos/$GITHUB_REPOSITORY/code-scanning/alerts?state=open&ref=refs/heads/master&per_page=100" --jq length | paste -sd+ - | bc)
          [ "${open:-0}" = 0 ] || { echo "::error::$open open code scanning alerts on master; fix them and prepare again"; exit 1; }
```

`stage-release`: `needs: [changes, forge, release-prepare, codeql]`, `permissions: contents: write, attestations: read, actions: read`, checkout `master` (`ref: master`) and `.hyperforge`, the composite with `source_sha: ${{ github.sha }}`, `ci_run_id: ${{ github.run_id }}`, `forge_sha: ${{ needs.forge.outputs.revision }}`, `token`. `pr-board` needs every job above it. The header comment: the tail replaces the `request-staging` sentence. Delete `stage-native-binaries.yml` with `rm`.

- [ ] **Step 4: Run the tests and actionlint**

Run: `python3 -B -m unittest discover -s .github/scripts/tests -p 'test_*.py' 2>&1 | tail -3 && actionlint .github/workflows/ci.yml`
Expected: `OK`; record the count the run prints (127 before Part B, minus the four deleted tests, plus the new ones); actionlint prints nothing.

- [ ] **Step 5: Commit** `git add -A .github/workflows .github/scripts/tests/test_workflow_contracts.py && git commit -m "ci.yml: the release tail packs, prepares, scans and stages in the preparation run"` (the file was removed with `rm` in Step 3; `git add -A` on the directory records the deletion).

### Task 12: `release.yml` on the tag

**Files:**
- Modify: `.github/workflows/release.yml` (rewrite), `.github/scripts/tests/test_workflow_contracts.py` (`ReleaseFlowTests`, constants `PACKERS`, `ACTION_JOBS`, `PUBLISH_GATE`)

**Interfaces:**
- Consumes: Task 8's `tag-guard` and `tag-gate`; Task 8b's `source_sha` input on the crate and Maven publishers.
- Produces: `release.yml` with `workflow_dispatch` inputs `publication_run_id` and `maven_deployment_id` only; jobs `find-ci-run`, `publication-gate`, the six publishers, `finalize-release`, `pie-acceptance`. The npm upload stays in the Forge's `publish-npm` composite (Global Constraints).

- [ ] **Step 1: Rewrite the tests**: `PACKERS = ()`; `ACTION_JOBS` maps `find-ci-run` to `tag-guard` and `publication-gate` to `tag-gate`, no packers; delete `PUBLISH_GATE`. Replace `test_tag_push_prepares_and_only_a_dispatch_publishes` with:

```python
def test_the_tag_push_publishes_and_a_dispatch_retries(self):
    self.assertIn('    tags: ["v*"]', self.text)
    self.assertEqual(re.findall(r'^      ([a-z_]+):\n        description', self.text, re.M), ['publication_run_id', 'maven_deployment_id'])
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
```

`test_pypi_upload_is_a_step_of_this_workflow`: the pypa step's `if` becomes `steps.pypi.outputs.pending == 'true'` and the verify step has no `if`. `test_every_registry_publisher_requires_the_global_gate`: the gate's `needs` is `{'find-ci-run'}`, no packer clauses. `test_pie_acceptance_runs_last_against_the_published_release`: drop the `workflow_dispatch` assertion. `test_forge_commit_is_resolved_once_and_checked_out_before_every_action`: the guard line is `tag-guard`; `forge_sha: ${{ steps.forge.outputs.revision }}` count stays 2. Add `test_pack_crates_and_maven_take_the_gate_s_verified_packages_at_a`: `prepared_run_id: ${{ github.run_id }}`, `publish: true` and `source_sha: ${{ needs.find-ci-run.outputs.source_sha }}` in both (the preserved crate and bundle were attested at A, and both reusable publishers verify at `inputs.source_sha`).

- [ ] **Step 2: Run them to verify they fail**. Run: `python3 -B -m unittest discover -s .github/scripts/tests -p 'test_workflow_contracts.py' -k ReleaseFlow 2>&1 | tail -3`. Expected: FAIL (`prepared_run_id` present, `-pack` jobs present).

- [ ] **Step 3: Rewrite `release.yml`**: the header comment describes the two acts and the retry dispatch (the spec's "Act two" in prose); `on: push: tags: ["v*"]` and the two inputs; `find-ci-run` as today with `tag-guard` (`forge_sha`, `token`) and a fourth output `packing_forge_sha`; `publication-gate` needs `find-ci-run`, `tag-gate` with `version`, `source_sha`, `ci_run_id`, `forge_sha`, `packing_forge_sha`, `attest: ${{ github.event_name == 'push' }}`, `token`, `npm_packages: true`, `cran_packages: true`; the six publishers as today with `publish: true` and `publication_run_id: ${{ inputs.publication_run_id }}` (`pack-crates` and `maven-publish` add `source_sha: ${{ needs.find-ci-run.outputs.source_sha }}`; `maven-publish` keeps `deployment_id: ${{ inputs.maven_deployment_id }}`); `pypi-publish`'s pypa step `if: steps.pypi.outputs.pending == 'true'` and the verify step unconditional; `finalize-release` and `pie-acceptance` without the event clause. Nothing else changes.

- [ ] **Step 4: Run the suite and actionlint**. Run: `python3 -B -m unittest discover -s .github/scripts/tests -p 'test_*.py' 2>&1 | tail -3 && actionlint .github/workflows/release.yml`. Expected: `OK`; nothing from actionlint. `test_calls_match_declared_inputs` proves every `with:` key against the Forge checkout: it needs Part A merged or checked out beside this repository.

- [ ] **Step 5: Commit** `git add .github/workflows/release.yml .github/scripts/tests/test_workflow_contracts.py && git commit -m "release.yml: the tag push verifies the preserved packages, attests from the tag and publishes; a dispatch retries"`.

### Task 13: CodeQL builds Swift against the preparation run's core

**Files:**
- Modify: `.github/workflows/codeql.yml` (`workflow_dispatch` input `ci_run_id`; `permissions` gains `attestations: read`; three steps before "Build the Swift library"; the header's "run on B" sentence)
- Test: `.github/scripts/tests/test_workflow_contracts.py` (class `PreparationFlowTests`)

**Interfaces:**
- Consumes: Task 4's `swift_release.py unpack`.
- Produces: `codeql.yml`'s `ci_run_id` input (optional, default `""`).

- [ ] **Step 1: Write the failing test**

```python
def test_codeql_swift_leg_stages_the_preparation_runs_core(self):
    text = (WORKFLOWS / 'codeql.yml').read_text()
    self.assertRegex(text, r'\n      ci_run_id:\n(?:        .*\n)*        required: false\n')
    self.assertIn('attestations: read', text)
    job = jobs(WORKFLOWS / 'codeql.yml')['analyze']
    names = re.findall(r'^      - name: (.*)$', job, re.M)
    self.assertEqual([n for n in names if n in ('Resolve the release-mode run whose core to build against',
                                                 'Stage that run\'s attested Swift core', 'Build the Swift library')],
                     ['Resolve the release-mode run whose core to build against', 'Stage that run\'s attested Swift core', 'Build the Swift library'])
    stage = job.split("      - name: Stage that run's attested Swift core\n")[1].split('\n\n')[0]
    for needle in ("if: matrix.build-mode == 'manual'", 'name: swift-release-assets', 'run-id: ${{ steps.run.outputs.id }}',
                   '--signer-repo SkunkWerkx/.github', '--source-digest', 'swift_release.py" unpack --assets'):
        with self.subTest(needle=needle):
            self.assertIn(needle, job)
    resolve = job.split('      - name: Resolve the release-mode run whose core to build against\n')[1].split('\n\n')[0]
    self.assertIn('inputs.ci_run_id', resolve)
    self.assertIn('--workflow ci.yml', resolve); self.assertIn('--branch master', resolve); self.assertIn('--status success', resolve)
    self.assertIn('"workflow_dispatch" or .event == "schedule"', resolve)
    self.assertIn('repository: SkunkWerkx/.github', job)
```

- [ ] **Step 2: Run it to verify it fails**. Expected: FAIL at the `ci_run_id` regex.

- [ ] **Step 3: Implement**: the input; the Forge checkout at master (`path: .hyperforge`, `persist-credentials: false`, `if: matrix.build-mode == 'manual'`); the resolve step (`id: run`, outputs `id` and `sha`: the input when given, else `gh run list --repo "$GITHUB_REPOSITORY" --workflow ci.yml --branch master --status success --limit 20 --json databaseId,event,headSha --jq 'map(select(.event == "workflow_dispatch" or .event == "schedule"))[0] | "\(.databaseId) \(.headSha)"'`, failing with `::error::No green release-mode ci.yml run on master to take a core from` when empty); the stage step: `actions/download-artifact` (`name: swift-release-assets`, `path: ${{ runner.temp }}/swift-release-assets`, `run-id`, `github-token: ${{ github.token }}`), then `gh attestation verify` each zip with `--repo "$GITHUB_REPOSITORY" --signer-repo SkunkWerkx/.github --source-digest "${{ steps.run.outputs.sha }}"`, then `python3 .hyperforge/.github/scripts/swift_release.py unpack --assets "$RUNNER_TEMP/swift-release-assets"`. The header: "run on A by the preparation run's `codeql` job, which passes its run id; the weekly scan takes the newest green release-mode run's core". `.github/RELEASING.md`'s step 3 goes in Task 14.

- [ ] **Step 4: Run the suite and actionlint**. Expected: `OK`; nothing from `actionlint .github/workflows/codeql.yml`.

- [ ] **Step 5: Commit** `git add .github/workflows/codeql.yml .github/scripts/tests/test_workflow_contracts.py && git commit -m "codeql: build Swift against the preparation run's attested core; the weekly scan takes the newest green run's"`.

### Task 14: The runbook, the CHANGELOG and the docs

**Files:**
- Modify: `.github/RELEASING.md` (rewrite to the six steps and the ladder), `CHANGELOG.md` (under `## [1.1.0]`, `### Changed`, a repository-only entry), `CLAUDE.md` ("Forge coupling and releases": the composite list, the two acts, the drift rule; "Never hand-edit": `Package.swift` is written by `ci.yml`'s `stage-release` job; "Local loop" and any `stage-native-binaries` mention), `docs/provenance.md` (the inventory and the packages are signed at A in the preparation run; the release attestation alone at the tag), `python/README.md` (the provenance paragraph: 1.1.0's publisher is `release.yml` on the tag push), `docs/LESSONS_LEARNED.md` (one paragraph under the open debts: the 1.0.0 PyPI provenance change and what the train does about it)
- Verify with: `grep -rn 'stage-native-binaries\|prepared_run_id\|request-staging' --include='*.md' --include='*.yml' --include='*.py' . | grep -v docs/superpowers`

- [ ] **Step 1: Write `.github/RELEASING.md`** as the spec's "The hand steps": the six steps, "Before 1.1.0 only", the retry ladder (rerun failed jobs; dispatch at the tag with `publication_run_id` and, for Central, `maven_deployment_id`; a patch release), the replay-proof dispatch, the 90-day window, the no-commit-between-B-and-the-tag rule, the PIE paragraph kept.

- [ ] **Step 2: The CHANGELOG entry** under `## [1.1.0]`'s `### Changed`, after the PR board entry:

```markdown
- **One dispatch prepares and attests, one tag publishes.** *(repository only)* `prepare-release.yml`
  bumps, dates the CHANGELOG heading, runs the full board, packs every registry input, attests each
  package and the inventory, preserves them for 90 days and commits the staging commit; pushing
  `vX.Y.Z` at it verifies the preserved bytes, signs the release attestation from the tag and
  publishes to the six registries. A retry is the same workflow dispatched at the tag, so the PyPI
  and npm publisher is `release.yml` from 1.1.0 on and never changes again (1.0.0's wheels went up
  from a one-off workflow after the tag run's upload failed, which PyPI shows as a change of
  provenance). `stage-native-binaries.yml` is gone; CodeQL runs on the preparation commit inside
  that run and must leave no open alert before the staging commit exists.
```

- [ ] **Step 3: The other docs**, each edit verified by reading the surrounding paragraph; then the grep above prints nothing outside `docs/superpowers/` and `CHANGELOG.md`'s history.

- [ ] **Step 4: Run the contract tests** (`test_text_files.py` and the changelog check): `python3 -B -m unittest discover -s .github/scripts/tests -p 'test_*.py' 2>&1 | tail -3` and `cd ../HyperForge && python3 -c "import sys; sys.path.insert(0,'.github/scripts'); import release_guard; t=open('../HyperUuid/CHANGELOG.md').read(); s=release_guard.changelog_section(t,'1.1.0'); assert s and '⟨' not in s; print(len(s.splitlines()),'lines')"`. Expected: `OK`; a line count.

- [ ] **Step 5: Commit** `git add .github/RELEASING.md CHANGELOG.md CLAUDE.md docs/provenance.md python/README.md docs/LESSONS_LEARNED.md && git commit -m "Docs: the release train's six hand steps, the provenance move to A, the changelog entry"`.

### Task 15: Prove the branch locally

**Files:** none new.

- [ ] **Step 1: Both suites and every workflow's lint**

Run: `python3 -B -m unittest discover -s .github/scripts/tests -p 'test_*.py' 2>&1 | tail -3; actionlint .github/workflows/*.yml; (cd ../HyperForge && find .github -name __pycache__ -exec rm -rf {} + ; LLVM_BIN=/opt/homebrew/opt/llvm/bin python3 -B -m unittest discover -s .github/scripts -p 'test_*.py' 2>&1 | tail -3)`
Expected: `OK` twice; actionlint silent.

- [ ] **Step 2: The classifier and the board verdict on this branch's diff**

Run: `git diff --name-only master... | awk '{print $0"\t"}' | python3 .github/scripts/pr_changes.py` then `NEEDS='{}' CHANGES='{"core":"false","ci":"true","docs_only":"false"}' GITHUB_EVENT_NAME=pull_request GITHUB_REF=refs/pull/1/merge python3 .github/scripts/pr_board.py`
Expected: `ci=true` among the lines; the board prints `ok` with every tail job `gate off`.

- [ ] **Step 3: Negative control**: temporarily set `TAIL_GATE` in `ci.yml`'s `stage-release` to `github.event_name == 'workflow_dispatch'`, run `test_the_tail_replaces_request_staging` and `test_gates_mirror_ci_yml`, see both fail, revert.

- [ ] **Step 4: Report** to the owner: what each suite printed, the negative control, what only CI can prove (the tail's artifact handoffs, the CodeQL dispatch, the Swift unpack on a runner, act two itself), and the landing order from the spec. No push.

---

## Self-review notes

- Spec coverage: act one's bump and headings (Task 3), the tail (Tasks 5, 6, 11), CodeQL (Tasks 4, 13), act two's guard, gate, publishers, finalizer, PIE (Tasks 7, 8, 12), the identity split (Tasks 1, 2), the runbook and docs (Tasks 9, 14), what the tests pin (each task's Step 1), the landing order (header). Not in any task: the npm placeholders and trusted publishers (the owner's hand step, in progress) and the Central Portal usage check (a glance).
- Names used across tasks: `CI_WORKFLOW`, `RELEASE_WORKFLOW`, `staging_commit`, `inventory_commit`, `prepared` (Task 1 → Task 8); `package_run_id`, `package_workflow` (Task 2 → Tasks 5, 8); `release_guard.py tag` / `forge-ancestry` and `packing_forge_sha` (Task 7 → Tasks 8, 12); `TAIL` (Task 10's Python and Task 11's test constant hold the same twelve names); `unpack` (Task 4 → Task 13).
- Proportion: 15 tasks over a 333-line spec; code appears only where the spec fixes exact copy (the codeql step, the changelog entry) or a test's assertions.
