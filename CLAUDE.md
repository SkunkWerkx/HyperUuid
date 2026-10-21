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

Don't open pull requests or push unless asked.
