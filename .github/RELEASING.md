# HyperUuid release flow

Follow the [shared runbook](https://github.com/SkunkWerkx/.github/blob/master/docs/release-runbook.md).
The workflows call the Forge at master. Each release run resolves that to one commit and
records it in the package inventory, so recovering a release needs the Forge's master
unchanged since the run that packaged it: hold Forge merges until a release is out.

Prepare with the existing prepare-release.yml version input, wait for the full CI
on A and verified staging commit B, then create the root vX.Y.Z tag at B. The
release prepares and attests every registry input before publication and preserves
the complete release-packages artifact. After all five publishers succeed, it
creates the matching go/vX.Y.Z tag and GitHub Release automatically.

CodeQL runs weekly and on demand, not on pull requests. Before tagging, run it on
B (`gh workflow run codeql.yml`, with master at B) and tag only once it is green
and code scanning shows no open alerts.

To recover saved bytes, dispatch release.yml at the same root tag with
prepared_run_id naming the earlier release run, plus publication_run_id when
registry receipts exist. Missing or conflicting bytes fail; no ancestor lookup,
CI override or native rebuild is allowed. Keep the package artifact (90-day
retention, bounded by policy). NuGet symbols and ambiguous Central uploads require
the receipt/deployment reconciliation described in the shared runbook.

This applies to subsequent releases. Do not republish v0.9.0.
