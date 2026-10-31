# Spike: Go without cgo (2026-10-30)

The R&D behind ROADMAP Phase 3, item 6, kept so that a session on another machine can pick it
up. The ROADMAP entry holds the findings and tables; this folder holds the harness that
produced them, the raw results, and what is left to do.

## Decision (owner, 2026-10-30)

The Go binding moves to **one backend**: the Rust core built for `wasm32-wasip1`, translated
to Go source by [wasm2go](https://github.com/ncruces/wasm2go), and committed as generated code
with a regenerate-and-diff check (as `include/hyperuuid.h` is). No cgo, no archives, no
TinyGo-specific archive, no release-only commits. The batch loss is accepted in exchange for
reach (no C toolchain anywhere) and one way of binding. It ships in 1.1.

Direction (owner, 2026-10-30): close the batch gap inside the translated path, with no native
regression: AES-256-CTR for `random_get` and the core's WebAssembly batch walk
(`rust/src/v7.rs`, `fill_batch_words`, which only WebAssembly builds compile). Measured on x64
(the XPS, in WSL2 and on Windows) and on arm64 (the M6 natively and Linux arm64 in Docker on
it), below.

## Closed (2026-10-31)

The one choice left open was where `random_get` gets its bytes, which decides generation
speed. The candidates: Go-side AES-256-CTR (`crypto/aes`, `cipher.NewCTR`, keyed from
`crypto/rand`; on the CPU's AES instructions it is a CTR-DRBG like Windows' `ProcessPrng` and
Apple's `CCRandomGenerateBytes`, and where there is none, riscv64, 386, the browser, Go's AES is
constant-time software); Go-side ChaCha20 (`golang.org/x/crypto/chacha20`, the native pool's
construction and rule, no amd64 assembly); Go-side ChaCha8 (`math/rand/v2`, fastest, but 8
rounds); the core's own ChaCha20 pool inside the guest (`entropy-in-guest.patch`, scalar, the
slowest batch); and `crypto/rand` per call (about 2x behind cgo on Linux arm64, where
`vgetrandom` and its locking are ~45% of a v7). Decided (owner, 2026-10-31): AES-256-CTR where
Go's own AES assembly runs (amd64, arm64, s390x, by the per-architecture feature rule Go's
`crypto/aes` uses, read through `golang.org/x/sys/cpu`; never under `purego`, TinyGo or FIPS
140 mode), rekeyed after 1 MiB or one second with the buffered keystream discarded, and the OS
source everywhere else, the browser included. Contention: a pool of instances for the stateless
calls behind one primary for v7, with the guest stack cut to 64 KiB so an instance is cheap.

The tables below are the spike's wrapper, not the shipped binding: one instance, one mutex,
no pool, an unbuffered AES stream with the budget checked once per call. Apple M6 (Mac18,5),
Go 1.27.3, rustc 1.99.1, `-count 6`, benchstat medians, from `results/macos-arm64-m6-*` and
`results/linux-arm64-docker-m6-*`. The shipped binding's own numbers are in `go/README.md` and
`docs/benchmarks.md`.

| ns/op, macOS arm64 | `version` | `new_v4` | `new_v7` | batch ×1000 | v7 ×12 goroutines | v4 ×12 goroutines |
| --- | --- | --- | --- | --- | --- | --- |
| cgo (1.0) | 12.4 | 184 | 72 | 1,835 | 207 | 459 |
| wasm2go, AES-256-CTR | 3.2 | 30 | 35 | 2,058 | 135 | 121 |
| wasm2go, ChaCha8 | 3.2 | 24 | 24 | 3,631 | 113 | |
| wasm2go, `crypto/rand` per call | 3.1 | 164 | 52 | 2,646 | 151 | |
| wasm2go, no entropy (the walk alone) | | | | 1,492 | | |

| ns/op, Linux arm64 (Docker on the M6) | `version` | `new_v4` | `new_v7` | batch ×1000 | v7 ×12 | v4 ×12 |
| --- | --- | --- | --- | --- | --- | --- |
| cgo (1.0) | 21 | 41 | 40 | 3,464 | 186 | 4.9 |
| wasm2go, AES-256-CTR | 3.3 | 47 | 55 | 2,145 | 145 | 132 |
| wasm2go, ChaCha8 | 3.3 | 39 | 40 | 3,718 | 120 | |
| wasm2go, `crypto/rand` per call | 4.7 | 75 | 65 | 8,470 | 150 | |
| wasm2go, no entropy (the walk alone) | | | | 1,516 | | |

On Linux arm64 the AES batch beats cgo's (the native core draws its batch from the ChaCha20
pool there) and single calls are 1.2–1.4x behind it; the 27x gap in parallel v4 under one
mutex is what the shipped binding's pool is for. The x64 rows are the XPS run in ROADMAP item
6: AES-CTR's batch 1.2x behind cgo on Linux amd64 and 1.6x on Windows, single calls ahead on
Windows and 1.1–1.3x behind on Linux. The parallel v4 benchmark is newer than that run, so its
x64 column awaits the XPS.

The walk's A/B (`walk-ab.sh`, interleaved, base `bb390d5`): the translated batch −29% with AES
and −36% with no entropy on macOS arm64, −28% and −35% on Linux arm64, native unchanged; on
x64, −30% and −33% in WSL2, −26% and −34% on Windows.
The raw module, untranslated (`WALK_WASM=1`, the M6): in V8 (node 24) 2.65 → 2.11 µs (−20%),
under wasmtime 50 13.7 → 13.0 µs (−5%). The translated core compiled to WebAssembly a second
time by Go (`WALK_BROWSER=1`, `GOOS=js` in node): 36.0 → 41.5 µs with AES (+15%), 5.8 → 10.6 µs
with no entropy (+84%), the one place the word walk loses
(`macos-arm64-m6-walk-ab-{wasm,browser}.txt`).

Under node (`GOOS=js`, where Go's AES is software): v4 200 ns, v7 304 ns, batch 40.7 µs with
the AES stream against 515 ns, 524 ns and 11.6 µs with `crypto/rand`
(`macos-arm64-m6-js-node-w2gaes.txt` and `-js-node.txt`); so the browser takes the OS source,
as the rule says.

## How the spike was run

Each variant is one module instance behind a mutex. The v7 counter lives in its linear memory,
so a process can have only one instance; the parallel benchmarks (12 goroutines) showed the
mutex is not the bottleneck for v7, which every backend serializes on the counter, and is the
whole cost for v4 (the `v4 ×12` column above), which is why the shipped binding pools instances
for the stateless calls. The 16 bytes go through one scratch page grown past the core's
memory, and nothing allocates.

| Package | What it is |
| --- | --- |
| `harness/cgostatic` | The control: today's cgo backend shape (by-value shims, static archive from `CGO_LDFLAGS`) |
| `harness/w2g` | Translated core, `random_get` from `crypto/rand` |
| `harness/w2g8` | Translated core, `random_get` from a Go-side ChaCha8 stream |
| `harness/w2g20` | Translated core, `random_get` from a Go-side ChaCha20 stream |
| `harness/w2gaes` | Translated core, `random_get` from a Go-side AES-256-CTR stream |
| `harness/w2gc` | Translated core with the in-guest pool (`entropy-in-guest.patch`), `random_get` used only for keys, plus a `clock_time_get` import |
| `harness/w2gfloor` | Not a random source: `random_get` writes nothing, so its batch is the translated walk alone, the floor every source adds to. Batch benchmark only |
| `harness/walkwasm` | Not a Go package: `bench.mjs` times the raw `wasm32-wasip1` module's batch export in V8, untranslated, with `random_get` from `crypto.getRandomValues`. `walk-ab.sh`'s `WALK_WASM=1` leg |

`harness/cgostatic` and `harness/w2gaes` also carry the 12-goroutine benchmarks
(`BenchmarkParallelCgoNewV7` and `NewV4`, `BenchmarkParallelW2gNewV7` and `NewV4`), the
`×12` columns above. `run.sh` runs cgostatic's with the rest of that package's benchmarks
(`-bench .`) and w2gaes's only on request: from `harness/`, `go test -tags hyperuuid_spike
-run '^$' -bench Parallel -benchmem -count 6 ./w2gaes` into
`results/<label>-parallel-w2gaes.txt`, and the same for `./cgostatic` with `run.sh`'s
`CGO_ENABLED=1 CGO_LDFLAGS=<archive>` into `-parallel-cgo.txt`.

`walk-ab.sh` measures the batch walk on its own: the core at `WALK_BASE` (default `bb390d5`, the
last commit with one walk) against the checkout, natively (the core's Criterion
`batch_1000/v7_batch`, where the two should match) and translated (`w2gaes` and `w2gfloor`),
alternating sides each round. `WALK_WASM=1` adds the raw module, untranslated, two ways: the
same Criterion bench run by wasmtime, and `walkwasm/bench.mjs` in node (V8). `WALK_BROWSER=1`
adds the translated batch under `GOOS=js GOARCH=wasm` in node. Both need node on PATH, the
first wasmtime too.

Every translated package also runs the corpus (194 vectors: v5, SQL order both ways,
refusals, timestamps, inspection; `v7_layout.json` needs injected entropy and is not run).
Every Go file carries `//go:build hyperuuid_spike`, so untagged tooling (CodeQL's Go autobuild,
`go build ./...`) sees no packages here. `run.sh` regenerates the wasm and the Go on each
machine; nothing generated or binary is committed.

## Running it on another machine

Needs: Rust stable with the `wasm32-wasip1` target (`rustup target add wasm32-wasip1`), Go 1.26
or later, a C compiler for the cgo control, and `benchstat`
(`go install golang.org/x/perf/cmd/benchstat@latest`). Optional for `SPIKE_BROWSER=1`: node,
TinyGo.

**macOS (the M6):**

```sh
cd docs/superpowers/spikes/2026-10-30-go-cgo-free/harness
SPIKE_LABEL=macos-arm64-m6 ./walk-ab.sh
SPIKE_LABEL=macos-arm64-m6 SPIKE_BROWSER=1 ./run.sh
```

**Linux arm64, in Docker on the M6** (virtualized, not emulated), from the repository root:

```sh
docker run --rm -v "$PWD:/src" -w /src/docs/superpowers/spikes/2026-10-30-go-cgo-free/harness \
  golang:1.27-bookworm bash -c '
    apt-get -qq update >/dev/null && apt-get -qq install -y patch >/dev/null
    curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --target wasm32-wasip1 >/dev/null
    . "$HOME/.cargo/env" && go install golang.org/x/perf/cmd/benchstat@latest
    git config --global --add safe.directory /src
    SPIKE_LABEL=linux-arm64-docker-m6 ./walk-ab.sh && SPIKE_LABEL=linux-arm64-docker-m6 ./run.sh'
```

**WSL2 (Linux amd64):**

```sh
cd docs/superpowers/spikes/2026-10-30-go-cgo-free/harness
SPIKE_LABEL=linux-amd64-wsl2-i9-11900H ./walk-ab.sh
SPIKE_LABEL=linux-amd64-wsl2-i9-11900H SPIKE_BROWSER=1 ./run.sh
```

**Windows (amd64), from Git Bash:** run from a checkout with LF line endings
(`git -c core.autocrlf=false clone`), since `entropy-in-guest.patch` won't apply to CRLF
sources. The control needs MinGW gcc on `PATH` (MSYS2's `mingw-w64-ucrt-x86_64-gcc`) and an
archive MinGW can link. `cargo staticlib` on the MSVC host makes one that MinGW refuses
(`build-static-libs.sh` in the Forge says why), so build the one the Forge trims for Go from
this checkout. It needs python3, so run it from WSL2 or any Unix box (it cross-compiles):

```sh
bash ../HyperForge/.github/actions/static-libs/build-static-libs.sh hyperuuid HyperUuid . x86_64-pc-windows-msvc
# -> go/staticlib/windows_amd64/libhyperuuid.a: copy it out, then `git checkout go/staticlib`
```

The committed `go/staticlib/windows_amd64/libhyperuuid.a` also links, but it is the 1.0.0 core,
older than `rust/`. Cargo's build scripts link for the host, so the MSVC host needs the Windows
SDK as well as the VC tools; without it, run on the GNU host toolchain, which builds the same
wasm byte for byte:

```sh
RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-gnu \
SPIKE_LABEL=windows-amd64-i9-11900H HYPERUUID_CGO_ARCHIVE=<the trimmed archive> ./run.sh
```

`walk-ab.sh` takes the same `RUSTUP_TOOLCHAIN` there. Results land in
`results/<label>-{env,cgo,w2g,w2g8,w2g20,w2gaes,w2gc,w2gfloor}.txt` (and `-js-node`,
`-tinygo-wasm` with `SPIKE_BROWSER=1`) from `run.sh`,
`results/<label>-walk-ab-{env,native,translated}.txt` from `walk-ab.sh` (and `-walk-ab-wasm`,
`-walk-ab-browser` with `WALK_WASM=1`, `WALK_BROWSER=1`), and `results/<label>-parallel-{cgo,w2gaes}.txt`
from the parallel runs above; benchstat medians go into ROADMAP item 6 and the tables above.

## Results so far (`results/`)

| Prefix | Machine |
| --- | --- |
| `macos-arm64-*` | Apple M6 (Mac18,5), Go 1.27.3, native |
| `linux-arm64-docker-*` | The same M6, `golang:1.27-bookworm` in Docker Desktop (virtualized, not emulated) |
| `linux-amd64-wsl2-i9-11900H-*` | Dell XPS 15 9510 (i9-11900H), Fedora 44 in WSL2, Go 1.27.3, gcc 16.2, node 24, TinyGo 0.44.0 |
| `windows-amd64-i9-11900H-*` | The same XPS, Windows 11 natively, Go 1.27.3, MSYS2 UCRT gcc 16.2; cgo control on the Forge-trimmed archive of the current core |

The x64 sets are `run.sh`'s own output (`-w2g` is `crypto/rand`, `-w2g8` ChaCha8, `-w2g20`
ChaCha20, `-w2gaes` AES-256-CTR, `-w2gc` the guest pool, `-w2gfloor` no entropy) and
`walk-ab.sh`'s (`-walk-ab-*`), all on the core with the WebAssembly batch walk; the arm64 sets
predate both, and spell the variants out.

Also there: the backends that were ruled out (purego v0.11.2, goffi v0.6.7, wazero v1.12.0)
and the wazero tuning runs (`+simd128`, `wasm-opt -O4`). Those came from an earlier scratch
harness that isn't kept. The browser rows in the ROADMAP are `macos-arm64-m6-js-node` and
`-tinygo-wasm` (the AES stream under node is `-js-node-w2gaes`) and
`linux-amd64-wsl2-i9-11900H-js-node` and `-tinygo-wasm`; `SPIKE_BROWSER=1` reproduces them.

## What the implementation did (2026-10-31)

Design and plan: `docs/superpowers/specs/2026-10-31-go-wasm2go-design.md` and
`docs/superpowers/plans/2026-10-31-go-wasm2go.md`; the result is ROADMAP Phase 3, items 6 and 7.

- `go/`: the generated core in `go/internal/core/core.go`, written by the tool module
  `go/internal/wasm2go/` (wasm2go pinned there, never in a consumer's graph; rustc pinned in its
  `rust-toolchain.toml`; the guest stack set to 64 KiB by the generator), and the binding over it
  with the AES-256-CTR stream or the OS source, one primary instance for v7 and a pool for the
  stateless calls, and the scratch-region call shape from `harness/w2g20`. `backend_static.go`,
  `backend_tinygo.go`, `unsupported.go`, `staticlib/`, the core freshness check,
  `hyperuuid_local` and the cgo-specific tests went.
- Tests that regenerate the core from `rust/` and diff it (`TestCoreIsCurrent`,
  `TestRegenerationCarriesTheCrateVersion`, forced by `HYPERUUID_CHECK_CORE=1`), and the whole
  Go suite under TinyGo on wasip1 and under stock Go on `js/wasm` and `wasip1/wasm` in CI.
- The Forge: `stage-native-binaries.yml` stages no Go archives; the Go legs need no C
  toolchain; the release has no Go binaries to attest. The regenerate-and-diff check runs on
  every push.
- Docs: `go/README.md`, CLAUDE.md, `docs/platforms.md`, `docs/benchmarks.md`, `RELEASING.md`
  and the CHANGELOG.
