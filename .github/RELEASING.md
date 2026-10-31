# HyperUuid release flow

Follow the [shared runbook](https://github.com/SkunkWerkx/.github/blob/master/docs/release-runbook.md).
The workflows call the Forge at master. Each release run resolves that to one commit and
records it in the package inventory, so recovering a release needs the Forge's master
unchanged since the run that packaged it: hold Forge merges until a release is out.

1. Rename `## [Unreleased]` in `CHANGELOG.md` to `## [X.Y.Z] — <date>` with a bullet for
   every package the diff touched; the tag run refuses a CHANGELOG without that section,
   and the GitHub Release notes are generated from it. Set `r/NEWS.md`'s heading to
   `# hyperuuid X.Y.Z` to match. Fill every `⟨…⟩` placeholder in the section (the Java
   bullet's `⟨n⟩ KB`, measured once the jar is built); the Forge refuses a section that
   still holds one.
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

   This is the recovery path, so every release exercises it. After all six publishers
   succeed it creates `go/vX.Y.Z` and the GitHub Release, with the 21 PHP extension zips
   attached (PIE installs from there; Packagist sees the tag at once, so `pie install` fails
   until this step has run). Its last job, `pie-acceptance`, then has PIE install
   `skunkwerkx/hyperuuid:vX.Y.Z` on every platform and PHP minor and checks it from a fresh
   `php`. An npm package that does not exist yet (the first release, or a new platform) has
   no trusted publisher to accept the upload, so before this dispatch the owner bootstraps it
   as the shared runbook says: a placeholder `0.0.1` under each new name, published by hand
   with a granular token, then `release.yml` registered as its trusted publisher and the token
   revoked. The release version itself is never published by hand, so its first real version
   is this run's Trusted Publishing upload and carries npm provenance.
6. Submit the R package to CRAN by hand: download `release-packages` from the tag run and
   upload its `cran/hyperuuid_X.Y.Z.tar.gz`, the attested bytes. No job uploads it.
7. Verify one file of each kind from the tag alone, exactly as a consumer would:

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

A red `pie-acceptance` after publication is fixed with a patch release, never in place: no
renamed asset (the inventory records its path), no repacked zip (the release attestation
names its digest) and no re-attest. Re-run the failed leg once if the failure was transient;
otherwise fix the cause and release X.Y.Z+1, noting in the CHANGELOG that X.Y.Z's PHP zips
don't install.
