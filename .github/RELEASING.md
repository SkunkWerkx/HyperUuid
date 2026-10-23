# HyperUuid release flow

Follow the [shared runbook](https://github.com/SkunkWerkx/.github/blob/master/docs/release-runbook.md).
The workflows call the Forge at master. Each release run resolves that to one commit and
records it in the package inventory, so recovering a release needs the Forge's master
unchanged since the run that packaged it: hold Forge merges until a release is out.

1. Rename `## [Unreleased]` in `CHANGELOG.md` to `## [X.Y.Z] — <date>` with a bullet for
   every package the diff touched; the tag run refuses a CHANGELOG without that section,
   and the GitHub Release notes are generated from it.
2. Dispatch `prepare-release.yml` with the version. It bumps every manifest, pushes A and
   runs `ci.yml` on it; `stage-native-binaries.yml` then commits the verified natives as B.
3. CodeQL runs weekly and on demand, not on pull requests: run it on B
   (`gh workflow run codeql.yml`, with master at B) and go on only once it is green and
   code scanning shows no open alerts.
4. Push the root tag `vX.Y.Z` at B (lightweight, unsigned). The release run verifies B
   against its preparation run, packs every registry input, attests each package and the
   release as a whole, preserves `release-packages` and plans every upload. Nothing is
   published. Read the run summary: the inventory, both attestations, every registry plan.
5. Publish from the preserved bytes:

   ```sh
   gh workflow run release.yml --repo SkunkWerkx/HyperUuid --ref vX.Y.Z -f prepared_run_id=<tag run id>
   ```

   This is the recovery path, so every release exercises it. After all five publishers
   succeed it creates `go/vX.Y.Z` and the GitHub Release.
6. Verify one file of each kind from the tag alone, exactly as a consumer would:

   ```sh
   gh attestation verify <file> --repo SkunkWerkx/HyperUuid --source-ref refs/tags/vX.Y.Z \
     --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
   ```

To recover, dispatch again at the same tag with `prepared_run_id`, plus `publication_run_id`
when registry receipts exist. Missing or conflicting bytes fail; no ancestor lookup, CI
override or native rebuild is allowed. Keep the package artifact (90-day retention, bounded
by policy). NuGet symbols and ambiguous Central uploads require the receipt/deployment
reconciliation described in the shared runbook. Registry versions can never be replaced:
never retag, never rebuild.
