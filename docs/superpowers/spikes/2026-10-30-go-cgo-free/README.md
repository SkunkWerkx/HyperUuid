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

Still open:

1. **Where `random_get` gets its bytes**, the one choice that decides generation speed:
   - Go-side ChaCha20 (`golang.org/x/crypto/chacha20`), keyed from `crypto/rand` and rekeyed
     after 1 MiB or one second: the native pool's construction and rule. Recommended. It has
     no amd64 assembly, so on x64 its batch is 1.5x ChaCha8's.
   - Go-side ChaCha8 (`math/rand/v2`), with the same rule: fastest, standard library, but 8 rounds
     will be questioned.
   - The core's own ChaCha20 pool inside the guest (`entropy-in-guest.patch`): entropy policy
     stays in the core; scalar code, so the slowest batch.
   - `crypto/rand` per call: simplest; about 2x behind cgo on Linux arm64 (`vgetrandom` and its
     locking are ~45% of a v7), 1.0–1.2x on Linux amd64 and 1.3x on Windows, where its batch is
     the fastest translated one.

## How the spike was run

Each variant is one module instance behind a mutex. The v7 counter lives in its linear memory,
so a process can have only one instance; parallel benchmarks showed the mutex is not the
bottleneck (all backends serialize on the counter). The 16 bytes go through one scratch page
grown past the core's memory, and nothing allocates.

| Package | What it is |
| --- | --- |
| `harness/cgostatic` | The control: today's cgo backend shape (by-value shims, static archive from `CGO_LDFLAGS`) |
| `harness/w2g` | Translated core, `random_get` from `crypto/rand` |
| `harness/w2g8` | Translated core, `random_get` from a Go-side ChaCha8 stream |
| `harness/w2g20` | Translated core, `random_get` from a Go-side ChaCha20 stream |
| `harness/w2gc` | Translated core with the in-guest pool (`entropy-in-guest.patch`), `random_get` used only for keys, plus a `clock_time_get` import |

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

**WSL2 (Linux amd64):**

```sh
cd docs/superpowers/spikes/2026-10-30-go-cgo-free/harness
SPIKE_LABEL=linux-amd64-wsl2-i9-11900H ./run.sh
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

Results land in `results/<label>-{env,cgo,w2g,w2g8,w2g20,w2gc}.txt` (and `-js-node`,
`-tinygo-wasm` with `SPIKE_BROWSER=1`); benchstat medians go into ROADMAP item 6.

## Results so far (`results/`)

| Prefix | Machine |
| --- | --- |
| `macos-arm64-*` | Apple M6 (Mac18,5), Go 1.27.3, native |
| `linux-arm64-docker-*` | The same M6, `golang:1.27-bookworm` in Docker Desktop (virtualized, not emulated) |
| `linux-amd64-wsl2-i9-11900H-*` | Dell XPS 15 9510 (i9-11900H), Fedora 44 in WSL2, Go 1.27.3, gcc 16.2, node 24, TinyGo 0.44.0 |
| `windows-amd64-i9-11900H-*` | The same XPS, Windows 11 natively, Go 1.27.3, MSYS2 UCRT gcc 16.2; cgo control on the Forge-trimmed archive of the current core |

The x64 sets are `run.sh`'s own output (`-w2g` is `crypto/rand`, `-w2g8` ChaCha8, `-w2g20`
ChaCha20, `-w2gc` the guest pool); the arm64 sets predate it and spell those out.

Also there: the backends that were ruled out (purego v0.11.2, goffi v0.6.7, wazero v1.12.0)
and the wazero tuning runs (`+simd128`, `wasm-opt -O4`). Those came from an earlier scratch
harness that isn't kept. The macOS browser rows in the ROADMAP (node, TinyGo, wasip1 under
wasmtime) were printed, not saved; the Linux amd64 ones are `linux-amd64-wsl2-i9-11900H-js-node`
and `-tinygo-wasm`, and `SPIKE_BROWSER=1` reproduces them.

## What the implementation will cover (for the plan, not done)

- `go/`: the generated core (`go tool wasm2go`, pinned in `go/go.mod`), the binding over it with
  the chosen random source, one instance behind a mutex, and the scratch-page call shape from
  `harness/w2g20`. `backend_static.go`, `backend_tinygo.go`, `unsupported.go`, `staticlib/`,
  the core freshness check, `hyperuuid_local` and the cgo-specific tests go.
- A test that regenerates the core from `rust/` and diffs it, and the Go suite running under
  TinyGo (ROADMAP item 7) against the same backend.
- The Forge: `stage-native-binaries.yml` stops staging Go archives; the Go legs in
  `hyper-build-native.yml` lose their C toolchains; the release no longer has Go binaries to
  attest. Translated sources are reviewable text, but their provenance is the wasm build, so the
  regenerate-and-diff check runs in CI on every push.
- Docs: the Go README (install, the sandbox, randomness, the honest batch loss), CLAUDE.md's
  "Never hand-edit" and "Local loop" sections, `docs/platforms.md`, `RELEASING.md`, and the
  CHANGELOG.
- Optional: shrink the guest's 1 MiB default stack (`-z stack-size`) to cut each instance's
  1.06 MB of linear memory.
