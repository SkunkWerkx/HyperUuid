# The release train: one dispatch prepares and attests, one tag ships

Written 2026-11-01 from the owner's release conversation of 2026-10-31 and 2026-11-01, after the
PR board (`2026-10-31-pr-board-design.md`) ran green on its first pull request. **Draft for
owner review.** Spans this repo's `prepare-release.yml`, `ci.yml` (its release-mode tail) and
`release.yml`, and the Forge (`SkunkWerkx/.github`), which gains pieces beside the ones it has.
HyperCast and HyperTabular take the consumer side when they thaw (the freeze stands).
`docs/superpowers/plans/` will hold the implementation plan.

## Goal

A release is two acts. Act one is a dispatch with the version: it bumps, builds and tests
everything on every host, packs every registry input, attests every package and the
inventory, preserves the bytes, and commits the staging commit B. Green means done; nothing
has left GitHub. Act two is the tag at B: it verifies the preserved bytes against act one's
attestations, signs the release attestation at the tag, publishes to the six registries
idempotently, creates `go/vX.Y.Z` and the GitHub Release, and has PIE install the result. A
retry is the same workflow dispatched at the tag, so the publisher PyPI and npm record never
changes again. The runbook is six hand steps, three of them reading.

## Decisions (owner, 2026-10-31 and 2026-11-01)

- **Attest without publishing, publish from the attested bytes.** "The provenance step needs
  to push the artifact and attest to it without ever publishing to the 3rd party providers so
  that the re-try mechanism can be patched in after the tag and not break the provenance and
  require another temporary stopgap trusted publisher like we did during the 1.0 re-org."
  1.0.0's PyPI upload failed inside a Forge composite (actions/runner#2473), the tag's
  `release.yml` could not change, a one-off `pypi-recovery.yml` uploaded the wheels, and PyPI
  recorded a change of publisher. 1.1.0 flips it back to `release.yml`, the last change.
- **The shape.** "We prep the release by manual dispatch of the intended tag v1.1.0 which
  sets the version number in all the packages and triggers the behemoth build, test, attest
  thing everywhere then when that goes green it's done until we create the v1.1.0 tag which
  then in turn triggers the artifact load from the manual dispatch run and ships it to the
  right destination."
- **Two runs, the work moved, not one callable run.** "I'm hoping after the 1.1 releases to
  be done with them for a good long while": a release every few months does not pay for
  making `ci.yml` callable, and every piece kept (the stager, the packers, the publishers,
  the finalizer, PIE acceptance) was proven at 1.0.0.
- **The runbook locked down.** "I also want the runbook locked down so that it becomes easy
  to reason about and managed."
- **The changelog is settled before the tag.** "I want it to be settled before the tag lands
  so that it is correct when people look at the tag but I also don't want updating it to
  screw up any provenance/attestations." Every prose edit lands in or before A; nothing
  prose-related touches master between A and the tag.
- **CRAN stays a hand step**, scripted up to the click: there is no API, the form post can be
  made from R, the email confirmation and the reviewer thread are the maintainer's, and
  CRAN asks for a release at most every month or two.
- **Maven Central's monthly cap** (enforced from 2026-11-01) changes nothing in the design:
  one deployment per release, found again on a retry.
- **npm roots are scoped** (`@skunkwerkx/hyperuuid`; decided the same night, landed in the
  Forge as 811f9d0): an unscoped name can already be anyone's, and `hypercast` is.

## Act one: `prepare-release.yml`, dispatched with the bare version

1. **The bump**, in the Forge's `hyper-prepare-release.yml` as today: every manifest, the Go
   core regenerated at the version, the lock files. New: the CHANGELOG heading
   `## [Unreleased]` becomes `## [X.Y.Z] — <today>` and `r/NEWS.md`'s heading becomes
   `# hyperuuid X.Y.Z`, each a no-op when it already reads so (1.1.0's was renamed by hand).
   `release_guard.py`'s changelog check runs on the result before the push: a missing section
   or a `⟨placeholder⟩` fails the dispatch with nothing built and nothing pushed. The bump is
   pushed as A and `ci.yml` is dispatched on it in release mode, as today.
2. **The board on A**, unchanged: every host, every binding, the simulators, Android, the
   wheels, the PHP zips, the C archives, the consumer tests. Off a pull request nothing is
   gated.
3. **The tail**, release mode only (`github.event_name == 'workflow_dispatch' && github.ref ==
   'refs/heads/master'`, the gate `request-staging` has today), replacing `request-staging`:
   - `forge`: resolves the one Forge commit this run uses from `referenced_workflows`, as
     `release.yml`'s `find-ci-run` does today; its output is `revision`.
   - the eight pack jobs, moved from `release.yml` with their inputs now the run's own:
     `nuget-org-pack`, `crate-pack`, `maven-pack` (the three Forge reusable workflows) and
     `ruby-pack`, `npm-pack`, `r-pack`, `c-pack`, `pypi-pack` (the five composites), each
     taking `version` from the bumped manifest, `source_sha` as `GITHUB_SHA`, `ci_run_id` as
     `GITHUB_RUN_ID` and `forge_sha` from `forge`. Each attests what it packs, so every
     caller job carries `id-token: write` and `attestations: write`.
   - `release-prepare`: a new Forge composite, the first half of today's `release-gate`:
     downloads every pack artifact, builds `inventory.json` (every shipped file, its digest,
     A, this run's id, the Forge commit), attests the inventory's build provenance, uploads
     `release-packages` (90 days). It makes no release attestation: that is the tag's.
   - `codeql`: dispatches `codeql.yml` at master with this run's id, waits for it, and
     requires zero open code scanning alerts on master; master must still be A (the stager's
     `require_master` check, run first). The hand dispatch between B and the tag goes.
     `codeql.yml`'s Swift leg builds through the root `Package.swift`, which links a core in
     `swift/` when one is there and otherwise downloads the release's zips; since the core
     left git (2026-10-30) no checkout has one, 1.0.0 attached no zips, and a second
     preparation at a version points at zips the finalizer has not published, so the leg
     cannot resolve on master as it stands (the weekly scan has not run since). The leg gains
     what the Swift suites have: it downloads the preparation run's `swift-release-assets`,
     verifies their attestations and unzips them into `swift/`, then builds. The run id is a
     dispatch input; the weekly scan looks up the newest green release-mode `ci.yml` run on
     master and takes its zips.
   - `stage-release`: a new Forge composite doing what `hyper-stage-natives.yml` does without
     the waiting: reads the Swift zips' checksums from this run's `swift-release-assets`,
     writes the root `Package.swift`, commits B with `Release-Source-SHA`, `Release-CI-Run-ID`
     and `Release-Forge-SHA` as trailers, pushes. Nothing but `Package.swift` changes in B.
   The run is green when B exists. `stage-native-binaries.yml` is deleted from this repo; the
   Forge keeps `hyper-stage-natives.yml` for the siblings.

A red leg is "rerun failed jobs" on that run. A fix that needs a commit lands on master
after A, and act one is dispatched again at the same version (unchanged version files still
dispatch the full board), making a new A and a new B.

## Act two: `release.yml` on the tag push at B

1. **The guard** (`find-ci-run`, the Forge's new `tag-guard`): the tag is at a B (a commit
   whose trailers name A, a run and a Forge commit; the tagged tree differs from A in
   `Package.swift` alone); that run is a green release-mode `ci.yml` run on this repo at A
   whose version is the tag's; its `release-packages` artifact exists. No input can override
   any of it: `prepared_run_id` and `ci_run_id` go. The guard's outputs are the run id, A,
   the version and this run's own Forge commit.
2. **The gate** (`publication-gate`, the Forge's new `tag-gate`): downloads
   `release-packages` from act one's run, verifies the inventory against the files, every
   file's digest, every build attestation (the Forge-signed ones with act one's Forge commit
   and A's digest, the consumer-signed ones with `--repo`), then, on the tag push only, signs
   the release attestation over every shipped file with the `release/v1` predicate from
   `refs/tags/vX.Y.Z`. A dispatch at the tag verifies that attestation and makes none.
3. **The six publishers**, in parallel after the gate, each as today: verify its own files'
   build provenance just before uploading, read what the registry serves, upload only what is
   missing, confirm the public bytes. Any conflict fails that publisher; nothing is ever
   overwritten. The PyPI upload stays a top-level step of `release.yml`, never inside a Forge
   action (actions/runner#2473), so its PEP 740 attestations name `release.yml` forever; npm's
   `npm publish` stays inside the Forge's `publish-npm` composite, since a composite keeps the
   caller's OIDC identity and npm's provenance therefore names `release.yml` too. The crate and
   Maven publishers verify the preserved package at A, where it was attested, and bind their
   receipts to B. Maven opens one deployment per release and finds it again on a retry. NuGet's
   symbol upload is verifiable only through the receipt of the run that pushed the package.
4. **The finalizer**, after all six: `go/vX.Y.Z` at B; the GitHub Release at the tag with the
   CHANGELOG's `## [X.Y.Z]` section as its body, a Verifying section, `inventory.json` and the
   release's own files attached (the Swift zips, the C archives, the PHP zips); each step
   skips what already exists. Its summary prints the CRAN submission command. The
   publishers' `publish` input, off on today's tag push (which only planned), is on for both
   events: the tag push publishes.
5. **PIE acceptance** last, as today; a red leg is a patch release, never a repair in place.

The dispatch form takes two inputs: `publication_run_id`, the earlier attempt holding
registry receipts, and `maven_deployment_id`, for a lost Central response. A tag push and a
dispatch at the tag run the same jobs; the dispatch differs only in those two inputs and in
verifying the release attestation instead of making it.

## Artifacts and attestations

| What | Made by | Where | Verifies with |
|---|---|---|---|
| natives, static libs, wheels, sdist, PHP zips, Swift zips, the nupkg, the crate, the jar and its native bundle | the Forge's reusable workflows, in act one's board and tail | master at A | `--signer-repo SkunkWerkx/.github --source-digest <A>` |
| gems, npm tarballs, R tarball, C archives, inventory | `ci.yml`'s tail (composites), act one | master at A | `--repo SkunkWerkx/HyperUuid` |
| the release attestation (`release/v1`, the inventory as predicate) over every shipped file | `release.yml`, the tag push, once | `refs/tags/vX.Y.Z` | the README's one command, unchanged |
| the served nupkg after NuGet's repository signature | `release.yml`, the run that pushed it | the tag | `--repo`, twice (provenance and `release/v1`) |
| PEP 740 publish attestations, npm provenance | the pypa action, a top-level step of `release.yml`, and `npm publish` in the `publish-npm` composite, under `release.yml`'s identity | the tag | PyPI's and npm's pages |

Nothing is ever remade, moved or replaced. The inventory records act one's Forge commit;
act two records its own in the run summary beside it.

## The inventory's two identities

Today's inventory carries one identity, bound to B: `staging_sha` is B, `package_run_id` is
the `release.yml` run at B that packed, every file not built at A is recorded at B, and the
validators hold it so (`validate_package_run` wants that run at B on `release.yml`;
`verify_release_attestation`, the publishers' receipts and the finalizer read B from the
inventory). An inventory built in act one exists before B does, so it cannot carry B, and the
Forge commit act two runs may differ from the one that packed.

The inventory therefore splits what it records:

- **Preparation identity**, in the inventory: `repository`, `crate`, `version`, `source_sha`
  (A), `ci_run_id` and `package_run_id` (both the `ci.yml` run at A), `forge_sha` (the Forge
  that packed) and a new `package_workflow` (`.github/workflows/ci.yml`). Every file's
  `source_sha` is A. No `staging_sha`.
- **Publication identity**, never in the inventory: B, the tag, the tag run's id and its
  Forge commit. They are what the release attestation's own provenance carries (signed at
  `refs/tags/vX.Y.Z`, whose commit is B) and what the run summary prints.

The readers follow, each change additive:

- `release-identity` keeps every existing input, default and step, exporting `STAGING_SHA` as
  the running commit, and gains two optional inputs, `package_run_id` and `package_workflow`,
  whose defaults are today's values (the run itself, `release.yml`). A sibling's call is
  unchanged in effect.
- **The inventory is verified against the preparation identity it carries, never against
  the act two run.** Today `verify` rebuilds the inventory from the environment's identity
  (the running commit as `staging_sha`, the running Forge as `forge_sha`) and compares, then
  verifies `inventory.json`'s attestation with `--source-digest` B. For an inventory with
  `package_workflow`, the rebuild takes `forge_sha`, `package_run_id` and `package_workflow`
  from the inventory itself, so the comparison holds across Forge drift, and its attestation
  is verified with `--repo` the consumer and `--source-digest` A. The environment still
  asserts what act two knows on its own, and the inventory must agree: `repository`,
  `crate`, `version`, `source_sha` (A from B's trailers) and `ci_run_id`; a difference in
  any of them fails. The inventory's `forge_sha` is additionally required to be an ancestor
  of the Forge act two runs, printed beside it in the summary.
- `validate_package_run` checks the package run against the inventory's `package_workflow`
  and, for `ci.yml`, at A (`head_sha` = `source_sha`, event `workflow_dispatch`).
- `verify_release_attestation` and `registry_plan`'s per-file attestation checks take the
  staging commit from the identity (`STAGING_SHA`, the running commit), falling back to the
  inventory's `staging_sha` when it has one. Per-file build provenance is verified at each
  entry's own `source_sha`, as today; every entry's is A.
- The receipts and `finalize` record the identity's `STAGING_SHA`, as they do.

An inventory of today's shape (`staging_sha`, no `package_workflow`) takes every path it
takes today: the Forge's tests keep one inventory of each shape through `verify`,
`validate_package_run` and `verify_release_attestation`, and the siblings' flow is untouched. `release-packages` lives 90 days: a
tag pushed later finds nothing and refuses, and the answer is act one again at a new A.

## Forge drift between the acts

Today: "hold Forge merges while a release is in flight anywhere". Now: the rule covers act
one alone, since act one's inventory names the Forge commit that packed and act two verifies
bytes against attestations, not code against a commit. A publisher fix merged to the Forge
between the acts is the intended path, which is what the owner asked for: the retry
mechanism can be patched after the tag. The consumer's `release.yml` is frozen at the tag and
must therefore stay thin: the guard, the gate, six publishers that call composites or
reusable workflows at master, the one top-level PyPI upload step, the finalizer, PIE.

## The hand steps

1. Write the bullets under `## [Unreleased]` (or the version's heading) and fill every
   placeholder. Nothing else about the release is prose.
2. Dispatch `prepare-release.yml` with the bare version.
3. Read act one's summary: B's trailers, the inventory, the digest listing, CodeQL clean.
4. Push `vX.Y.Z` at B, lightweight and unsigned.
5. Read the tag run's summary: six registries confirmed, the Release written, PIE green. Then
   dispatch `release.yml` at the tag once with that run's id as `publication_run_id`: every
   publisher must report nothing to do. That is the replay proof, so every release still
   exercises recovery, as a no-op instead of as the normal path.
6. CRAN: run the printed command on the preserved tarball, click the confirmation email.

Before 1.1.0 only: the thirty npm placeholders and their trusted publishers (memory
`npm-scoped-roots`), and a glance at the Central Portal's usage page.

The retry ladder: rerun failed jobs on the same run; then dispatch at the tag with the
receipts; then, for a wrong artifact, a patch release. Never a retag, a repack or a re-attest.

`.github/RELEASING.md` becomes exactly those six steps and the ladder. The Forge's
`docs/release-runbook.md` gains a section for this flow beside the existing one, since the
siblings run the old flow until they thaw.

## Forge pieces and consumer pieces

Forge, added beside what exists (nothing the siblings call changes shape):

- `actions/release-prepare`: the inventory, its attestation, the preservation.
- `actions/stage-release`: `Package.swift` from this run's zips, B with the trailers.
- `actions/tag-guard` and `actions/tag-gate`: the guard and the gate of act two. The
  existing `release-guard` and `release-gate` keep every input, default and step, and
  `release-identity` keeps its existing ones and adds two optional inputs with today's
  values as defaults: HyperCast and HyperTabular call all three at master with
  `prepared_run_id`, and their tag runs rely on the gate packing and preserving. `release_guard.py` gains the tag-run
  checks as new functions beside the old.
- `release_inventory.py` and `package_preparation.py`: the two identities above, and the
  tail's job names where artifacts are looked up by run and job.
- `hyper-prepare-release.yml`: the heading date and the R heading, the changelog check
  before the push.
- Tests for each, written to fail first against today's files; the runbook section.

This repo:

- `ci.yml`: `request-staging` replaced by `forge`, the eight pack jobs, `release-prepare`,
  `codeql`, `stage-release`, each `needs` every board job (the list `request-staging` has
  today) and gated as it is. `pr-board` unchanged; `pr_board.py` learns the new release-only
  job names.
- `release.yml`: `find-ci-run` without inputs; no pack jobs; the gate; the six publishers
  running on the tag push; the finalizer and PIE acceptance on both events; two inputs.
- `stage-native-binaries.yml` deleted. `prepare-release.yml` unchanged (one Forge call).
- `RELEASING.md`, the CHANGELOG entry under 1.1.0 (the release flow, repository only), the
  provenance docs where the signer of the inventory moves from the tag to A.

## What the tests pin

This repo's workflow contract tests (`.github/scripts/tests/test_workflow_contracts.py`):

- the tail: the six jobs exist, in dependency order, gated on dispatch at master, and each
  `needs` every board job; `request-staging` is gone; every attesting job has `id-token` and
  `attestations` write.
- `release.yml`: exactly two `workflow_dispatch` inputs; no job whose name ends in `-pack`;
  the PyPI upload is a top-level step of this file; every publisher needs the
  gate; the finalizer needs all six; PIE needs the finalizer; the tag push and the dispatch
  run the same publisher jobs; the file's name is `release.yml`.
- `pr_board.py`: a gate per new release-only job.

The Forge's tests: the four composites' inputs and steps, the guard's refusals (a tag not at
a B, a B whose run is red, a run at another version, a missing `release-packages`), the
gate's two modes, both inventory shapes through every validator, the prepare workflow's
heading rename as a no-op on an already-renamed section, and the existing actions' inputs
unchanged. Each written to fail first.

## Proof

- The branch's own PR board proves the PR-mode side of `ci.yml` and the contract tests.
- Act one is proven by 1.1.0's own preparation: dispatched on master, it makes A, runs the
  board and the tail, and commits B, before any tag exists. A red tail is fixed on master and
  dispatched again.
- Act two is proven once, at 1.1.0's tag; the replay dispatch proves the retry path on the
  same night. The PIE rehearsal against a scratch repository stays the one thing provable
  before.
- The siblings prove nothing until they thaw.

## Landing order

1. The Forge PR, tests first, additive only. No release in flight anywhere.
2. This repo's PR, with the npm scoped-root change already in its working tree, and one last
   PR board run on it.
3. Dispatch `prepare-release.yml` with 1.1.0 on master: act one for real.
4. The npm placeholders and trusted publishers, before any tag.
5. Push `v1.1.0` at B. Act two, the replay dispatch, CRAN.

Step 3 bumps master to 1.1.0 before step 4 is done; no other commit may land between B and
the tag, since the tag must point at B. If one must, prepare again.

## Hazards

- **A tag not at B.** The guard refuses: the tagged tree must be A plus `Package.swift`, and
  the trailers must name a green run at that version. No ancestor lookup.
- **Master moved past A during act one.** The stager's `require_master` refuses to commit B;
  prepare again.
- **CodeQL finds an alert on A.** The tail is red before B; fix on master, prepare again.
- **A publisher fix needs `release.yml` itself.** The file at the tag is frozen. That is why
  it stays thin: the fix belongs in the Forge, which act two reads at master. A fix that
  truly needs the consumer file is a patch release.
- **The 90-day window.** A tag after the artifact expired refuses; act one again.
- **Maven's monthly cap.** One deployment, reused on retry; the summary prints the usage.
- **npm's first publish.** The placeholders must exist with `release.yml` registered on all
  ten of this repo's names, or the npm publisher fails after the other five succeeded; the
  retry ladder covers it, and the finalizer waits.

## Open questions

None. The one carried from spec one, where package build provenance is made, is settled by
the shape: at A, in act one, with the release attestation alone made at the tag.

## Revision log

- 2026-11-01: first draft, from the conversation of 2026-10-31 and 2026-11-01.
- 2026-11-01, plan review: the crate and Maven publishers verified the preserved package at
  the running commit, so they take the preparation source as an input; npm's upload stays in
  the Forge composite, which keeps the caller's identity (the earlier sentence was wrong).
- 2026-11-01, review round 1. Verified against the Forge and the siblings and folded in: the
  siblings call `release-guard` and `release-gate` with `prepared_run_id`, so act two's guard
  and gate are new composites and the old ones keep their shape; the CodeQL Swift leg cannot
  resolve a core on master since the core left git, so it stages the preparation run's zips;
  the inventory's identity was bound to B, so it now carries the preparation identity only,
  with the publication identity in the release attestation's provenance, and both shapes
  validate.
