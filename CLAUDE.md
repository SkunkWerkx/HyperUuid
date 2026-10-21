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

- Ruby ABIs: `ABIS` in `ruby/Rakefile`, both `ABIS=(…)` in `release.yml`,
  `ruby_compat_version` in `ci.yml`, and the Forge's `ruby_wasm_abis` default. Magnus
  extensions are tied to one Ruby minor, so gems are fat across these ABIs and Fiddle
  covers everything outside them, including other platforms. Fiddle must never be
  restricted to old Rubies. Ruby 3.4 reaches EOL on 2028-03-31: then drop it from those
  lists and clear `ruby_compat_version`.
- Adding or removing a C export: every binding, C#'s three `LibraryImport` declarations
  (`"hyperuuid"`, `"*"`, `"__Internal"`), the `EmccExportedFunction` list in
  `csharp/HyperUuid/build/net11.0/HyperUuid.targets` (`uuid_variant` is excluded on
  purpose), the AOT and wasm smoke tests, and the export count in the READMEs.

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

- The Forge is pinned to one exact commit in `ci.yml`, `release.yml` and `RELEASING.md`.
  Repin all of them, and HyperCast and HyperTabular with them, as one reviewed change.
- Registry versions can never be replaced. Release and recovery: `.github/RELEASING.md`
  and the Forge's `docs/release-runbook.md`. Recover by re-dispatching from the preserved
  packages, never by retagging or rebuilding.

## Local loop

Build `cargo cdylib` in `rust/` before running a binding's tests. A `--features python` or
maturin build overwrites the same `target/release` library, and the other bindings then
fail on missing symbols until `cargo cdylib` runs again (`rust/README.md`, "Local dev
trap").

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

## Open follow-ups (as of 2026-10-21)

- Benchmark tables were measured on the WSL2 i9-11900H box. When the dev machine changes,
  re-measure every harness there before relabelling a table; never mix machines, and say
  which tables predate a core change. A benchmarking workflow is an idea, not started.
- The `EmccExportedFunction` list in
  `csharp/HyperUuid/build/net11.0/HyperUuid.targets` may be dead weight: on .NET 11 rc.1
  the wasm smoke test passed with every item removed (P/Invoke through `"*"` uses the
  runtime's static pinvoke table). Re-verify on the current SDK in all three repos, then
  drop the lists and correct the docs that call them load-bearing, together.
- Java does not reach Android: the binding uses FFM, which ART lacks, so it would need a
  JNI layer. Python, Ruby and PHP don't reach Android either.
