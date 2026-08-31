# Changelog

All eight packages in this repository — the `hyperuuid` crate and the C#, Java, Go, Python, Ruby,
PHP and Swift bindings — share one coordinated version, so one changelog covers all of them. Each
entry marks which packages it actually affects.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] — 2026-08-31

### Added

- **The Rust core is genuinely `#![no_std]`** under a new default-on `std` feature, so the
  `no-std` category the crate has published since 0.1.0 is compiler-enforced rather than
  asserted. Default-on rather than unconditional because the crate also builds the `cdylib`
  every other binding dlopens, and a linked artifact needs a `#[panic_handler]` only std
  supplies. *(`hyperuuid` crate)*
- **The crate no longer links `alloc` either**, and now carries the `no-std::no-alloc` category
  alongside `no-std`. *(`hyperuuid` crate)*
- LICENSE and README are now bundled inside every package that can carry them, so the license
  text ships with the artifact rather than only being named in its metadata.
  *(NuGet, Maven, PyPI, RubyGems)*
- CI gained a `check-no-std` job. Every other cargo invocation in the pipeline compiles the
  *std* configuration, so nothing else would notice a `use std::` creeping back into the core.
- New tests: `tests/v7_counter_race.rs` (the monotonic counter under 8-thread contention,
  including its first concurrent calls while the seed is landing) and per-item entropy
  placement assertions for both batch functions. *(`hyperuuid` crate)*

### Changed

- `NewV6Error`/`NewV7Error` implement `core::error::Error` instead of `std::error::Error`. These
  are the same trait — `std::error::Error` is a re-export — so callers see no difference.
  *(`hyperuuid` crate)*
- `getrandom`'s std-only error impl now rides the `std` feature instead of being pinned on
  unconditionally, so it is no longer forced into every consumer's dependency graph.
  *(`hyperuuid` crate)*
- `v7::now_v7` is compiled out without the `std` feature, joining the `wasm32` gate it already
  had. It is the only API that reads a system clock. *(`hyperuuid` crate)*
- `v7`'s process-global monotonic counter replaced `std::sync::OnceLock` with a lock-free seed
  fold over `core::sync::atomic`. The seed is *added* rather than stored, so it commutes with
  concurrent increments instead of clobbering one; the RFC 9562 §6.2 ordering guarantee is
  unchanged. *(`hyperuuid` crate)*
- `new_v6_batch`/`new_v7_batch` no longer allocate. Their single `getrandom` call now draws into
  the caller's own output buffer, and each item's entropy is moved to its final octets as the
  batch is written backwards — no scratch buffer on the heap, and no fixed stack frame either.
  Benchmarked against the previous implementation on the same machine: v7 slightly faster, v6
  within noise; the published batch speedups still hold. *(`hyperuuid` crate)*
- **Behavior change on an error path.** The batch functions draw entropy before assembling any
  UUID, so on `NewV6Error::Random`/`NewV7Error::Random` no UUID has been written but the front of
  `out` may hold partial bytes from the failed draw. Previously `out` was left untouched. Treat
  the buffer as clobbered rather than intact when a batch call returns `Err`.
  *(`hyperuuid` crate)*
- Documentation corrected across the bindings where it still described the retired ctypes
  backend, and each binding's README now stays about that binding. Registry and CI badges added
  to all of them. *(docs only, all packages)*

### Upgrade note

For essentially everyone this is a drop-in patch release: default features are on and the public
API is unchanged. The one exception is a `Cargo.toml` that already said
`default-features = false` — in 0.1.0 the crate had no default features, so that line was inert
and you still got a full std build. It now means what it says, and `v7::now_v7` disappears.
Either drop the line, or take the no_std path deliberately, which additionally needs a
`getrandom` custom backend and your own timestamps.

The C ABI is untouched — all 12 exported symbols are identical — so the seven non-Rust bindings
carry no API or behavior change beyond the packaging and documentation entries above.

## [0.1.0] — 2026-08-30

First coordinated release — all eight packages published together from one tag, and the first
tag to go out through the repository's own release pipeline rather than by hand. Full notes:
[v0.1.0 release](https://github.com/SkunkWerkx/HyperUuid/releases/tag/v0.1.0).

### Added

- One RFC 9562 UUID engine written in Rust and called directly from C#, Java, Go, Swift, Ruby,
  PHP and Python — published to crates.io, NuGet, Maven Central, PyPI, RubyGems, Packagist, and
  git tags for Swift and Go.
- v4 (random), v5 (deterministic, namespace-based), v6 and v7 (time-sortable, with a real
  monotonic counter for v7) on every binding, plus batch generation, timestamp extraction, and
  SQL Server byte-ordering for clustered `uniqueidentifier` columns.
- Go's module is tagged separately as `go/v0.1.0`, a Go modules requirement for a subdirectory
  module, pushed alongside the bare tag.

### Notes

- v1 and v3 are deliberately not implemented; RFC 9562 treats them as superseded by v6 and v5.
- Every binding was verified by pulling it from its real live registry into a fresh scratch
  project and generating a UUID — not by a passing CI job alone. The same `(DNS, "example.com")`
  input produces a byte-identical v5 UUID on all eight.
- Go is a deliberate control group and is *slower* per call than `google/uuid`, which is pure
  compiled Go with no FFI boundary. Reported rather than omitted.
- Known gaps at this release: free-threaded CPython (3.13t/3.14t) unsupported; WebAssembly proven
  for Rust and C# only; PHP skips win-arm64, which PHP itself has never shipped a native build
  for.

[Unreleased]: https://github.com/SkunkWerkx/HyperUuid/compare/v.0.1.1...HEAD
[0.1.1]: https://github.com/SkunkWerkx/HyperUuid/compare/v0.1.0...v.0.1.1
[0.1.0]: https://github.com/SkunkWerkx/HyperUuid/releases/tag/v0.1.0
