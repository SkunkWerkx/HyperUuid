# HyperUuid — notes for agents

One RFC 9562 UUID engine in Rust (`rust/`), reached over a C ABI or as a native extension
from seven other languages. `README.md` has the why, each binding's README has its own
mechanics, and all CI and release logic lives in the Forge (`SkunkWerkx/.github`). This
file holds what the code alone won't tell you.

## Bytes that are persisted

- The SQL Server layout (the v8 form of a v7) and v5 output sit in other people's clustered
  indexes and keys. Moving one bit silently reorders or re-derives every stored row, and
  every property test would still pass. `corpus/*.json` pins the exact bytes.
- Corpus vectors are add-only. Regenerate them only with `python3 corpus/oracle.py
  --write`, never by hand. The oracle deliberately shares no code with the core (it works
  from RFC field tables in bit strings); keep it that way. Rationale: `corpus/README.md`
  and README "The layout is persisted data".
- v7's counter semantics (width, wrap, batch roll-forward) are a documented contract:
  README "v7 ordering, precisely".

## Claims are measured

Every performance claim is a dated, machine-specific receipt, and losses are printed next
to wins. Go is the deliberate control group, slower per call and said so. Don't adjust
numbers without re-running the harness, and don't soften the Go section.

## Lists that move together

- Ruby ABIs: `ABIS` in `ruby/Rakefile`, `ruby_compat_version` in `ci.yml`, and in the Forge
  the `ruby.abis` default in `hyper_config.py` (an `.github/hyper.json` override if this repo
  ever differs) and the `ruby_wasm_abis` default. Magnus
  extensions are tied to one Ruby minor, so gems are fat across these ABIs and Fiddle
  covers everything outside them, including other platforms. Fiddle must never be
  restricted to old Rubies. Ruby 3.4 reaches EOL on 2028-03-31: then drop it from those
  lists and clear `ruby_compat_version`.
- Adding or removing a C export: every binding, C#'s three `LibraryImport` declarations
  (`"hyperuuid"`, `"*"`, `"__Internal"`), the sites in `.github/scripts/check_exports.py`,
  `cdylib_smoke.py`, the prefixes in `.github/hyper.json` (the Forge's symbol check reads
  them), the AOT and wasm smoke tests, and the export count in the READMEs. The Blazor
  archive needs no export list: the SDK's pinvoke table links every P/Invoke by symbol.

## Never hand-edit

- `go/staticlib/`, the Swift artifact bundle and xcframework, and `php/src/native/` are
  CI-built binaries committed by `stage-native-binaries.yml`. Its commit trailers are what
  `release.yml` uses to find the CI run.
- Versions: bump through `prepare-release.yml`. The libraries embed their version, so a
  hand bump ships binaries reporting the old one.

## Toolchains and floors

Each language's primary toolchain is its current release; check endoflife.date rather than
assuming. Exceptions must be commented where pinned: a real platform constraint, or an
older floor shipped beside the primary and dropped at its EOL. Floors match HyperCast and
HyperTabular and move together.

## Forge coupling and releases

- The Forge is called at master, deliberately unpinned: a Forge merge reaches every consumer
  at once, and breakage it causes is fixed forward. `release.yml` resolves master to one
  commit per run (`find-ci-run`'s `forge_sha`), checks the Forge out at it as `.hyperforge`
  and runs its composite actions from there (`release-guard`, `pack-gems`, `verify-wheels`,
  `release-gate`, `publish-nuget`, `publish-rubygems`, `publish-pypi`, `finalize-release`);
  the five reusable workflows (NuGet, crate and Maven packing; crate and Maven publishing)
  are called at master and resolved when the run is queued. The inventory records the
  commit and recovery requires it unchanged. Don't merge Forge changes while a release
  is in flight.
- `prepare-release.yml`, `stage-native-binaries.yml` and ci.yml's `check-core` are one
  `uses:` each of a Forge reusable workflow; this repo keeps triggers, permissions and gates.
  `.github/hyper.json` is what the Forge knows about this project (crate and project names,
  the export rule, the cdylib smoke script; the ABI probe is `.github/scripts/abi_probe.c`,
  the Forge's default path, and every other name is derived in the Forge's `hyper_config.py`). `.github/scripts/tests` holds the workflow shape contracts
  and the probe tests; the generic release-guard tests live in the Forge.
- The release is two acts at one tag: the tag push packs, attests (build provenance and the
  release attestation over every shipped file) and preserves without uploading; `gh workflow
  run release.yml --ref vX.Y.Z -f prepared_run_id=<tag run>` publishes from the preserved
  bytes. The finalizer writes the GitHub Release from the CHANGELOG's `## [X.Y.Z]` section,
  and the tag run refuses a CHANGELOG without it.
- Third-party actions are pinned to a commit with the version in a trailing comment, and
  `.github/dependabot.yml` moves them with every other ecosystem in one monthly pull request.
  It ignores the Forge and `dtolnay/rust-toolchain` (branch heads, refreshed by hand).
- Registry versions can never be replaced. Release and recovery: `.github/RELEASING.md`
  and the Forge's `docs/release-runbook.md`. Recover by re-dispatching from the preserved
  packages, never by retagging or rebuilding.

## Local loop

Build `cargo cdylib` in `rust/` before running a binding's tests. A raw `cargo rustc
--release --crate-type cdylib --features python` (or `ruby`, or `php`) in `rust/` overwrites
the same `target/release` library, and the other bindings then fail on missing symbols until
`cargo cdylib` runs again; maturin in `python/` and the `ruby-ext`/`php-ext` aliases build
elsewhere and don't (`rust/README.md`, "Local dev trap").

Go, Swift and PHP otherwise run against the committed binaries, which are refreshed only at
release and so lag every core change: build the checkout's own with
`.github/scripts/local-core.sh`, then `go test -tags hyperuuid_local ./...`,
`HYPERUUID_LOCAL_CORE=1 swift test`, and PHP with `HYPERUUID_NATIVE_LIBRARY`. CI builds each
leg's core before its suites, so it never sees the committed ones. Outside CI the three
suites refuse to run against a committed core older than `rust/` (the core freshness check),
and `local-core.sh` finds `../HyperForge` on its own. Go caches a pass from before a core
change; `-count=1` runs the check again.

## Working with the owner

- The owner drives git. Don't commit, push, tag, open pull requests or dispatch workflows
  unless asked for that step by name; a go on the edits is not a go on git. Leave the work
  in the working tree, verified, and report it.
- Agreeing that an approach is sound, or stating a preference, is a design decision, not
  an instruction to start. Reading to scope the work is fine; edits wait for the go. During
  a release the owner may be acting in parallel: `git fetch` and compare with origin before
  editing.
- Prove everything locally first, installing toolchains under `~` as needed. Every push to
  a PR branch runs the full matrix, so only what cannot run locally goes to CI, and the
  report says which parts those were.
- Fix stale docs, comments and links on sight; that permission is standing. READMEs state
  the current state only, history goes in the CHANGELOG, and README examples are verified
  by hand in the same pass. No CI step for docs.
- A change reaches all seven bindings and the sibling repos in the same pass, with parity
  in behaviour, API and wording. Where a binding can't follow, say why for that binding.
  Never leave a repo half-edited.
- Pre-1.0, a breaking change is a plain CHANGELOG entry under Unreleased; no semver
  callout, no upgrade hand-wringing.
- Install snippets keep `<version>` (Go: `@latest`); a concrete version there is the bug.
- Drafting release notes includes the pre-publish fixes: rename `[Unreleased]`, add the
  bullet for every package the diff touched, then draft from the fixed changelog.
- Never report commit, push or release state from memory; check with git or gh in the same
  turn, or leave it out.
- A verification counts only when its negative control fails. Copy the flags the repo's own
  alias or CI step uses before trusting a check.
- Formatters and their CI checks cover src, tests, benchmarks and smoke tests in every
  language.
- Tags are lightweight and unsigned (`git -c tag.gpgsign=false tag`); commits are always
  signed.
- Remove agent worktrees and their branches once the work is in the main checkout;
  `.claude/` is untracked and `git add` picks it up.

## Open follow-ups (as of 2026-10-23)

- Benchmark tables were measured on the WSL2 i9-11900H box. When the dev machine changes,
  re-measure every harness there before relabelling a table; never mix machines, and say
  which tables predate a core change. A benchmarking workflow is an idea, not started.
- 1.0.0 is the first release on the two-step flow. What only it can prove: the composite
  actions in a real job, PyPI Trusted Publishing, nuget.org's finalized bytes, the Maven
  confirmation wait, the finalizer. Afterwards the Forge drops its schema-1 inventory paths.
- Java does not reach Android: the binding uses FFM, which ART lacks, so it would need a
  JNI layer. Python, Ruby and PHP don't reach Android either.
