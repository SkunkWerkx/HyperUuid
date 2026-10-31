# Go: one backend, the core translated to Go by wasm2go (design, 2026-10-31)

ROADMAP Phase 3, items 6 and 7, and the Go step of the owner's v1.1 sequence (after PHP, before
the TypeScript wasm backend). One PR, CI green before the next step. The spike that decided it,
with its harness and raw results, is `docs/superpowers/spikes/2026-10-30-go-cgo-free/`. Two
external review rounds are folded in (2026-10-31); their findings are marked where they changed
the design.

## Goal

`go get github.com/SkunkWerkx/HyperUuid/go` is the whole install on every GOOS and GOARCH Go
has, with no C compiler anywhere and no binary in git. The Go binding is one backend: the Rust
core built for `wasm32-wasip1`, translated to one Go source file by
[wasm2go](https://github.com/ncruces/wasm2go) and committed, regenerated and diffed in CI the way
`include/hyperuuid.h` is. The public API and the 194 corpus vectors are unchanged; one error
value is added. The whole Go suite runs under stock Go on every CI leg, under TinyGo, and under
stock Go's own WebAssembly targets.

## Decisions (owner, 2026-10-30 and 2026-10-31)

- **One backend, the translated core.** cgo, the thirteen committed archives under
  `go/staticlib/`, `backend_static.go`, `backend_tinygo.go`, `unsupported.go`, the core freshness
  check, the `hyperuuid_local` tag and the release-only-commit plan all go. The batch loss is
  accepted for reach and for one way of binding.
- **Randomness: a hardened AES-256-CTR stream where Go's own AES is accelerated, the OS source
  everywhere else** (review round 2, finding 1; the owner's terms, 2026-10-31). The stream is
  selected only when `golang.org/x/sys/cpu` reports the features Go's assembly requires for that
  architecture (Go 1.27, `crypto/internal/fips140/aes/aes_asm.go` and `aes_s390x.go`):
  amd64 `X86.HasAES && X86.HasSSE41 && X86.HasSSSE3`, arm64 `ARM64.HasAES`, s390x
  `S390X.HasAES && S390X.HasAESCBC`. ppc64 and ppc64le take the OS source: Go assumes POWER8
  there with no runtime detection and only an unregistered `GODEBUG` knob to turn it off, so
  acceleration cannot be established from outside. The stream is never selected under the
  `purego` tag, under `tinygo`, or when `crypto/fips140.Enabled()` is true. `x/sys/cpu` processes
  the `GODEBUG` cpu options itself, so `GODEBUG=cpu.aes=off` selects the OS source. The feature
  rule is pinned beside Go's source file and line in a comment and re-checked at every Go
  upgrade (a "lists that move together" entry). The OS source is one checked `io.ReadFull` of
  `crypto/rand.Reader` per `random_get`; a failure or a short read returns an errno, never
  success. ChaCha8 and ChaCha20 are not used. `golang.org/x/sys` is the module's second
  dependency, accepted for the measured batch gain.
- **Contention: a pool of instances for stateless calls, a 64 KiB guest stack** (2026-10-31).
  The v7 counter lives in the instance's linear memory, so `uuid_new_v7` and `uuid_new_v7_batch`
  run on one primary instance behind its mutex. v4, v5 and every inspection call carry no state
  and run on the primary when its lock is free, else on a pooled instance, created lazily. The
  evidence is the gap, not the fix: on Linux arm64 cgo's v4 scales across 12 threads to 4.9 ns a
  call aggregate, where one mutexed instance holds at about 130 ns. That the pool closes it is a
  hypothesis until the shipped binding's benchmarks say so (review round 2, finding 5). The
  wasip1 build is linked with a 64 KiB shadow stack instead of rust-lld's 1 MiB default so an
  instance is small enough to pool.
- **Batches live in guest memory.** A batch of n needs 16n bytes in the primary's scratch region,
  which grows to the largest batch asked for and never shrinks (WebAssembly memory cannot), at
  most 1 GiB at `MaxV7Batch`. Chunking was rejected: a chunk that hits `CounterExhausted` would
  leave earlier chunks written and the stream advanced, where the contract says a failed batch
  reserves nothing.
- **v5 names have a stated limit and an error** (review rounds 2 and 3). A name is copied into
  guest memory and read there through `slice::from_raw_parts`, whose length may not exceed
  `isize::MAX` of the guest, 2^31 − 1 on wasm32, whatever the host's word size; so the limit is
  the smaller of that and what the guest and host can address, not `MaxUint32`. `NewV5` returns
  the new `ErrNameTooLong` past it, and an error from the core call is no longer discarded.
- **The regeneration toolchain is pinned.** The wasm's bytes follow rustc's codegen, so the
  translation is made with one named stable rustc (1.99.1 today), installed by CI for the check
  and by release preparation for the bump. A floating stable would turn every push red on each
  Rust release day until someone regenerated. The pin is commented as a reproducibility
  constraint, which is what CLAUDE.md asks of any pin, and a bump is a deliberate
  regenerate-and-commit, as a cbindgen bump is.
- **Release preparation regenerates the core** (review round 2, finding 3). The core's version
  is a literal in the generated file, so the bump commit must carry a regenerated core or fail
  its own version test.
- **wasm2go is pinned in a tool module of its own**, `go/internal/wasm2go/` with its own `go.mod`
  and `go.sum`, so it never enters a consumer's module graph. The Go README says what the pin
  means: wasm2go is pre-1.0 with one maintainer, and because the generated Go is committed, a
  dead wasm2go costs regeneration, not the binding.
- **The Forge drops its Go-archive path outright.** HyperCast and HyperTabular are frozen until
  HyperUuid 1.1 and get no shims; they take this design when they thaw.
- **Scope beyond item 6:** item 7 (the whole suite under TinyGo in CI) and stock Go's
  `GOOS=js` and `GOOS=wasip1` as supported, tested targets. Not in scope: ChaCha variants, the
  core's pool in the guest, a streaming v5 interface in the C ABI, Android packaging for Java,
  the TypeScript wasm backend, the siblings, anything about release timing.

## Facts this rests on (measured 2026-10-30 and 2026-10-31)

Apple M6 (Mac18,5), Go 1.27.3, rustc 1.99.1, `-count 6`, benchstat medians; the harness in the
spike, this session's results under `results/macos-arm64-m6-*` and
`results/linux-arm64-docker-m6-*`. Every row is the spike's wrapper: one instance, one mutex,
no pool, an unbuffered AES stream with the budget checked once per call. The shipped binding is
benchmarked again before any of these numbers appears in a README.

| ns/op, macOS arm64 | `version` | `new_v4` | `new_v7` | batch ×1000 | v7 ×12 threads | v4 ×12 threads |
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
pool there) and single calls are 1.2-1.4x behind it; the 27x gap in parallel v4 is the one the
pool is meant to close. The x64 rows are the spike's XPS run (ROADMAP item 6): AES-CTR's batch
1.2x behind cgo on Linux amd64 and 1.6x on Windows, single calls ahead on Windows and 1.1-1.3x
behind on Linux; the parallel v4 benchmark is new and waits for the XPS re-run.

- **The WebAssembly batch walk** (`rust/src/v7.rs`, `fill_batch_words`, on this branch): the
  translated batch −29% with AES and −36% with no entropy on macOS arm64, −28% and −35% on Linux
  arm64, native unchanged (interleaved A/B, `walk-ab.sh`). Same on x64 (−30%/−33% WSL2,
  −26%/−34% Windows).
- **Under node (`GOOS=js`, software AES):** v4 200 ns, v7 304 ns, batch 40.7 µs, against
  `crypto/rand`'s 515 / 524 ns / 11.6 µs. So wasm takes the OS source, as the rule says.
- **Go's AES is chosen at run time**, not by GOARCH: `supportsAES` is a CPU-feature check, the
  assembly is excluded by `purego`, and the generic fallback is variable-time (Go 1.27 source).
- **`aes.NewCipher` plus `cipher.NewCTR` allocate twice per rekey** (review round 2's probe),
  amortized over 1 MiB of output; between rekeys and growth events the paths allocate nothing.
- **Stock Go's `crypto/rand.Read` never returns an error** since Go 1.24 (it crashes
  irrecoverably instead), but TinyGo 0.44's returns `io.ReadFull(Reader, b)` and can fail or
  fall short (review round 3, finding 2). A Go program cannot fork without exec, so the stream
  needs no fork guard. `crypto/fips140.Enabled()` exists (Go 1.24+); the module's floor is 1.26.
- **The core chunks WASI `random_get` at 64 KiB** (`rust/src/entropy.rs`, `fill_with` for
  `target_os = "wasi"`): a batch of 1000 is one call of 6,004 bytes, a maximum batch is about
  6,150 calls, and the core's `getrandom` maps a nonzero errno to an error that reaches the
  caller as the random-source failure code.
- **The generated memory** starts at the stack plus the data segment rounded to pages, grows by
  `append` in whole pages, and refuses to grow past 65,536 pages or past `math.MaxInt >> 16`
  pages on a 32-bit host (`memory_grow` in the generated file), returning −1 only for those
  limits; a host allocation failure inside `append` is a runtime fatal, as any `make` is. Growth
  follows Go's slice policy, approximately 1.25x for a small incremental grow of a large slice
  (review round 3's probe: 16 MiB grown by one page retained 20 MiB, 24.6% over the logical
  length). `Xhyperuuid_version` returns a literal compiled from `CARGO_PKG_VERSION`.
- **The corpus is byte-identical through the translated core** natively on arm64 and amd64 and
  under emulation on 386, riscv64, ppc64le and s390x; a corrupted vector fails (spike).
- **The wasm is byte-identical** across macOS, Linux arm64 and amd64 and Windows at one rustc,
  and wasm2go's output is deterministic, so the committed file can be diffed.
- **The sandbox:** wasm2go's generated code bounds-checks every memory access and keeps the call
  stack in Go, so a core bug, a stack overflow included, is a Go panic and never a corrupted
  heap. The generated core confirms rustc links the stack first (the stack pointer starts at the
  stack size and the data segment sits above it), so an overflow runs the pointer below zero and
  traps on the next access rather than overwriting data.
- **`check_exports.py` reads the two backend files** being deleted (its two Go sites), and
  `hyper-prepare-release.yml` regenerates the napi wrappers after the bump and nothing for Go.

## The module (`go/`)

**Layout.**

| Path | What |
| --- | --- |
| `internal/core/core.go` | wasm2go's output, package `core`: `// Code generated by wasm2go vX. DO NOT EDIT.`, no imports beyond the standard library, the WASI import as an interface the binding implements. About 45 KB. |
| `core.go` | The instance type, the primary, the pool, the scratch region, the WASI `random_get` host, and the unexported calls `uuidgen.go` and `inspect.go` already make (`newV4`, `newV5`, `newV7`, `newV7Batch`, `v7ToSqlOrder`, `version`, `getTimestamp` and the rest), with their signatures unchanged except `newV5`, which returns an error. `Available`, `LoadError` and `NativeVersion` move here from `load.go`; the first two keep returning `true` and `nil` for parity with every binding's load probe. |
| `random.go` | The `source` interface, the shared checked reader (`io.ReadFull` on a field defaulting to `crypto/rand.Reader`, `EIO` on failure) and the OS source, always compiled: one checked read per `random_get`. |
| `random_aes.go` | `//go:build (amd64 \|\| arm64 \|\| s390x) && !purego && !tinygo`: the buffered AES-256-CTR stream and `accelerated()`, the per-architecture feature match above. Its inverse-tagged twin's `accelerated()` returns false. Selection, at instance creation: the stream when `accelerated() && !fips140.Enabled()`, else the OS source. |
| `errors.go` | Gains `ErrNameTooLong`. |
| `internal/wasm2go/` | The tool module: `go.mod` with `tool github.com/ncruces/wasm2go`, `rust-toolchain.toml` naming the pinned rustc, and `main.go`, the generator (below). Not part of the published module. |
| `generate.go` | `//go:generate go -C internal/wasm2go run .` |

**Gone:** `backend_static.go`, `backend_tinygo.go`, `unsupported.go`, `load.go`, `staticlib/`
with its README, `core_freshness_test.go`, `internal/corefreshness/`,
`internal/exhaustiontest/freshness_test.go` and the build tag on that package, every
`hyperuuid_local` and cgo build constraint, `.github/scripts/check_go_archives.sh`, the Go halves
of `.github/scripts/local-core.sh` and `android_build_suite.sh`, and the `go/staticlib` lines in
`.gitignore`.

**The instance.** One `core.Module`, its random source, and a scratch region: one 64 KiB page
grown past the core's own memory at creation, addressed by its byte offset, with its page count
tracked. Arguments are copied into the scratch, the core is called, results are copied out.
`getTimestamp`'s 8-byte out parameter sits in the scratch after the UUID. One function,
`scratchPagesFor(bytes)`, does the page arithmetic and refuses a size the host cannot address;
it is unit-tested at the page boundaries and at the caps without allocating.

**Growth, in three numbers** (review rounds 2 and 3). *Logical* guest length is the pages the
core and the scratch occupy: 64 KiB stack plus the data segment, 2 pages, plus the scratch, 3
pages (192 KiB) for a fresh instance, before the Go objects around it and the random source's
state. *Retained* capacity is what `append` kept: approximately 1.25x the logical length for a
small incremental grow of a large slice (Go's growth formula has an additive term and the
allocator rounds, so this is a description, not a bound) and about the required length for a
large one; the spec asserts no figure and the plan measures both shapes. *Peak* during a
grow is the old backing array plus the new one, both live during the copy. A maximum batch is
therefore on the order of 1 GiB retained in the primary, more than that at the moment it grows,
and the caller's own 1 GiB slice besides; the README states the three as separate, observed
numbers from the plan's `runtime.MemStats` measurements of a fresh maximum batch and of
incremental growth, never as a fixed estimate. A grow refused by the generated `memory_grow`
(the page limits) is an error to the caller; a grow the host cannot allocate is a runtime fatal,
exactly as the caller's own `make([]uuid.UUID, n)` would be, and the README says which is which.

**Primary and pool.** `primary` is created on first use (`sync.Once`), so an import costs
nothing. `newV7` and `newV7Batch` take `primary.mu.Lock()`. Every other call does
`primary.mu.TryLock()` and, when that fails, takes an instance from a `sync.Pool` whose `New`
creates one; each pooled instance has its own random source, keyed on creation. `sync.Pool`
bounds nothing: it is per-P with a victim cache, cleared across two GCs, and a goroutine
preempted inside a call holds its instance meanwhile, so the live count tracks the goroutines
concurrently inside stateless calls, approximately GOMAXPROCS and not a limit. An instance is
returned to the pool only if its scratch is still the base page; one that grew is dropped.
Instances in the pool never run a v7 call, so their counters are never used. If the shipped
benchmarks show churn (re-keying and 192 KiB allocations after every GC under load), a bounded
free list of GOMAXPROCS instances replaces `sync.Pool`; the spec commits to the measurement, not
to the container. Under TinyGo the scheduler is single-threaded; if its `sync` lacks `TryLock`
the plan adds a `//go:build tinygo` file that routes everything through the primary.

**v5.** The namespace, the output and the name sit in the scratch at offsets 0, 16 and 32. A
name that fits the base page runs on the primary or a pooled instance like any stateless call.
A longer one runs on a one-off instance sized for it by `scratchPagesFor(len(name) + 32)` and
discarded afterwards, so neither the primary nor the pool retains it; its random source is never
called and is the OS source. `MaxV5Name` is the smaller of two bounds, computed in `uint64`
before any conversion to `int`: the guest's slice limit, 2^31 − 1 bytes (`isize::MAX` on
wasm32, which `slice::from_raw_parts` in `rust/src/ffi.rs` must not exceed, on any host), and
the guest's address space less the instance's two base pages and the 32 bytes before the name,
itself capped by the host's page limit on a 32-bit platform. On a 64-bit host the slice limit
wins and `MaxV5Name` is 2,147,483,647; on a 32-bit host the address bound is a little under it
and the README gives the formula. `NewV5` returns `ErrNameTooLong` when `len(name) >
MaxV5Name` or when `scratchPagesFor` refuses, both decided by arithmetic before anything is
allocated, and the message names the limit; a host that cannot allocate the pages is a runtime
fatal, as the caller's own allocation of such a name was. The former `MaxUint32` message goes. A
streaming v5 through the C ABI, which would restore the old range, is out of scope and noted in
the ROADMAP.

**The random source.** Both sources read the OS through one `io.Reader` field that defaults to
`crypto/rand.Reader`, with `io.ReadFull`, and check the result (review round 3, finding 2):
stock Go's `Read` never fails, TinyGo's can fail or fall short, and in either case `random_get`
returns WASI's `EIO` and never 0 over bytes it did not write; the core turns that into the
random-source failure code and the caller gets `ErrRandomSource`, which is therefore reachable
and documented as such. A short `Read` with a nil error is not a failure: `io.ReadFull` retries
it, and a reader that delivers the bytes in several short successful reads succeeds; one that
ends early with `EOF` or an error is the failure, and a test of each sees the UUID in the first
case and the error, never a UUID, in the second.

The OS source is one `ReadFull` into guest memory per `random_get`. The AES stream: a 32-byte
key and zero IV from the reader, `cipher.NewCTR` over `aes.NewCipher`, and a 512-byte keystream
buffer, defined so that every byte served, in order, equals one contiguous AES-256-CTR keystream
of the current key (review round 3, finding 4):

- *Entry:* every request first checks the clock. If one second has passed since keying, the
  buffer's remainder is discarded and the stream is rekeyed before any byte is served (review
  round 4): buffered keystream never outlives its key's second.
- *Refill:* `XORKeyStream` over a zero source into the buffer, 512 bytes at a time, at most once
  per request; the buffer's served offset returns to 0.
- *Serve:* a request then drains whatever the buffer still holds, in order. If more is needed and
  the remainder is at least the buffer's size, it is generated directly into guest memory, which
  is zeroed first so the stored bytes are keystream, not keystream XOR stale memory; a smaller
  remainder comes from a refill. So the sequence small, large, small reads offsets 0-15, 16-N,
  N+1 onward of one stream, which the deterministic test pins with an injected key.
- *Byte budget:* 1 MiB of keystream generated under a key (refills and direct generation counted
  alike), checked before every refill and every direct generation and enforced inside a request:
  a generation that would cross it is cut at the boundary, the stream is rekeyed, and the rest is
  generated under the new key. The core's one `random_get` call is served whole, and a batch's
  single reservation is untouched; the core itself already chunks WASI `random_get` at 64 KiB,
  so no request exceeds that.
- *Rekey:* discards whatever the buffer still holds, draws a fresh key through the same checked
  reader, and starts the counter at zero. A rekey that fails leaves the source unusable, with no
  key, until a later request's rekey completes: nothing is served from the old buffer or under
  the old key meanwhile, and the request that hit the failure returns `EIO`. The key and IV live
  in reusable storage on the source, so a rekey's allocations are the cipher's and the CTR's at
  most; the exact count is a measured result of the shipped code, not a number this spec asserts
  (review round 4: a probe with local key arrays and the read inside the closure counted four),
  and the README reports what `testing.AllocsPerRun` measures over the complete rekey path.
  Between rekeys and growth events nothing allocates, and the allocation tests pin that.

**The generator** (`internal/wasm2go/main.go`): locates the repository root from its own source
file (`runtime.Caller`, three directories up from `go/internal/wasm2go/`), overridable with
`-root`, so it works from any working directory and under `go test`. It reads the toolchain from
its `rust-toolchain.toml`, runs `cargo +<pin> cdylib --target wasm32-wasip1 -- -C
link-arg=-zstack-size=65536` in `<root>/rust` (a missing toolchain fails with the `rustup
toolchain install <pin> --target wasm32-wasip1` line), then `go tool wasm2go -pkg core -o
<root>/go/internal/core/core.go <wasm>`, and takes `-o` to write elsewhere. The stack flag goes
on the command line, not in `rust/.cargo/config.toml`, so the crate's own wasip1 test suite under
wasmtime and Swift's WebAssembly archive are untouched.

**The 64 KiB stack has a measured margin, not a proof.** The plan's measurement task
regenerates the core at decreasing stack sizes and runs the whole suite (corpus, exhaustion, a
v5 over a name of `MaxV5Name` on a 64-bit host, a batch of `MaxV7Batch`) at each, records the
smallest size that passes and confirms that one size below it panics with an index out of range
(the overflow is a trap, not wrong bytes). The shipped 64 KiB must be at least 8x that smallest
size. The two numbers go in `core.go`'s comment and the README, worded as a margin over the
tested paths.

**The check** is `TestCoreIsCurrent` in `core_test.go`: it runs the generator with `-o` into a
temp dir and diffs against the committed file. Without cargo, the pinned toolchain or the wasip1
target it skips, except under `HYPERUUID_CHECK_CORE=1`, where the same conditions fail it; CI
sets the variable (the `HYPERUUID_CHECK_STUBS` pattern). Negative control: one changed byte in
`internal/core/core.go` fails it.

**TinyGo and wasm portability** (item 7): `inspect_test.go`'s `7 + 1<<32` case is added only when
`strconv.IntSize == 64`; the corpus is found through `HYPERUUID_CORPUS` first, then
`testdata/corpus`, then by walking up from the working directory; the crate version comes from
`HYPERUUID_CRATE_VERSION` as it already can. `internal/tinygosmoke` stays as the browser page,
without the archive.

## CI and the Forge

**HyperForge** (one PR, merged before HyperUuid's, since `ci.yml` calls master):

- `hyper_config.py`: `go_targets`, `GO_TARGETS` and every `go/staticlib` path go; `STATIC_TREES`
  holds Swift's trees only. `test_hyper_config.py`, `test_release_guard.py`,
  `test_release_inventory.py`, `test_release_notes.py` and `test_c_package.py` lose their Go
  fixtures, and the release notes say "committed for Swift" where they said "for Go and Swift".
- `static-libs/action.yml` and `build-static-libs.sh`: the `go/staticlib` destinations, the
  `wasm32-wasip1` Go destination and the Windows trim that existed only for MinGW go.
- `hyper-stage-natives.yml`: no staging into `go/`, no `git add -f go/staticlib`; its header
  describes Swift alone.
- `hyper-prepare-release.yml`: after the bump and the napi step, a Go step installs Go from
  `go/go.mod`, installs the rustc named in `go/internal/wasm2go/rust-toolchain.toml` with the
  wasip1 target, runs `go generate` in `go/` and adds `go/internal/core/core.go` to the bump
  commit, when the repository has `go/internal/wasm2go/`. The Forge's workflow tests gain a case:
  a bump, the regeneration, and `NativeVersion` reporting the new version through the core test.
- `hyper-build-native.yml`: the Go step runs `CGO_ENABLED=0 go test ./...` right after the Rust
  compile steps, ahead of Java's JNI path, in the desktop job and the musl suite list alike (the
  fail-fast rule). The llvm-mingw fetch on Windows ARM64 and the Alpine Go run go: the first
  served cgo, the second proved the archive against musl, and a pure-Go binary links no libc.
- `hyper-build-wasm.yml`: `test-go-tinygo` becomes the Go wasm job, gated by a boolean input.
  No archive is built. It runs the whole suite three ways and the browser page once: TinyGo
  `-target=wasip1`, compiled with `-c` and run under wasmtime with the checkout mounted and
  `HYPERUUID_CORPUS` set (negative control: without the mount the corpus tests fail with "not
  found"); stock Go `GOOS=js GOARCH=wasm` under node through `go_js_wasm_exec`; stock Go
  `GOOS=wasip1 GOARCH=wasm` under wasmtime through `go_wasip1_wasm_exec`; and the TinyGo
  browser page in headless Chrome as today.
- `hyper-check-core.yml`'s `lint-go`: the archive-table step is replaced by three. A
  cross-compile sweep, `CGO_ENABLED=0 go vet ./...` for each pair the README lists (linux/{amd64,
  386, arm, arm64, riscv64, ppc64le, s390x, loong64, mips64le}, windows/{amd64, arm64, 386},
  darwin/{amd64, arm64}, freebsd, openbsd, netbsd, illumos, aix/ppc64, plan9, android/{amd64,
  arm64}, ios/arm64, js/wasm, wasip1/wasm) and once with `-tags purego`. The regeneration check:
  install the pinned rustc with the wasip1 target, then `HYPERUUID_CHECK_CORE=1 go test -run
  TestCoreIsCurrent`. And the suite under `-race`. revive and gofmt also cover
  `internal/wasm2go/`.

**HyperUuid:**

- `ci.yml` follows the input changes, and `test_workflow_contracts.py` pins the needles: the
  Go wasm job's three runtimes, `CGO_ENABLED=0` on the Go step, the regeneration check, the
  prepare-release Go step.
- `.github/scripts/check_exports.py` (review rounds 2 and 3): the two Go sites become
  `go/internal/core/core.go` with the pattern `func \(m \*Module\) X((?:uuid|hyperuuid)_\w+)\(`
  (every export is translated) and `go/core.go` with `\.X((?:uuid|hyperuuid)_\w+)\(` (every
  export is called), both rule `all`. The prefix is load-bearing: wasm2go also emits `Xmemory`,
  which an unprefixed `X(\w+)` reports as a stale export. `test_check_exports.py` gains a
  positive control (the intact generated core and binding pass) beside the missing-export and
  stale-export negative controls on the new sites.
- `test-apple-mobile` keeps the Go suite in the iOS simulator as the iOS proof and drops the
  archive checks. An iOS executable still needs Xcode's external linker under any Go backend;
  that is Go's rule, and the README says so rather than claiming iOS without a toolchain.
- `test-android` keeps the Go suite in the emulator, built with `CGO_ENABLED=0`; the plan
  verifies that the internally linked binary carries the 16 KB page alignment the emulator image
  needs, and only if it does not keeps the NDK link for the test binary, saying why.
- `.github/dependabot.yml` gains `go/internal/wasm2go` as a gomod directory (the published
  module's directory is already there, and now carries `golang.org/x/sys`). A wasm2go bump that
  changes the output turns the regeneration check red on that PR; the fix is to regenerate and
  commit, and `CLAUDE.md` says so.
- `stage-native-binaries.yml`'s comments stop mentioning Go.

## Testing

- The existing suite, unchanged in what it asserts: unit, corpus (194 vectors), allocations
  (zero on the warmed paths), the SQL-order corpus both ways, the exhaustion package in its own
  process, the examples.
- `core_test.go`: `TestCoreIsCurrent`; `scratchPagesFor` at the page boundaries and both caps;
  a batch larger than the scratch grows it and a second one reuses it; a v5 past the base page
  runs on a one-off instance and leaves the primary's page count unchanged; `NewV5` returns
  `ErrNameTooLong` at `MaxV5Name + 1` through a `scratchPagesFor` call, not an allocation;
  stateless calls run on a pooled instance while the primary is held (the primary locked from
  the test, a v4 still returns); a grown instance is not returned to the pool; v7 under 12
  goroutines stays strictly increasing; mixed v4, v5, v7 and inspection traffic from many
  goroutines under `-race`.
- `random_test.go`: a reader that delivers its bytes in several short successful reads yields a
  UUID; a reader that fails, and one that ends early with `EOF`, each make `random_get` return
  `EIO` and `NewV4` return `ErrRandomSource`, with no bytes reported as written; a batch of 1000
  is one `random_get` of 6,004 bytes and a batch of 20,000 arrives in 64 KiB chunks (a counting
  host); a batch whose reader fails on a later chunk returns `ErrRandomSource` and leaves the v7
  stream where it was (the next single v7 shows no reservation was consumed).
- `random_aes_test.go`: deterministic byte streams with an injected key and clock: the bytes
  served by the sequence 16, 2,000, 16 equal offsets 0-15, 16-2,015 and 2,016-2,031 of one
  AES-256-CTR keystream of zeros under that key, which pins drain-then-direct and the zeroed
  destination; partial-buffer consumption across a size-triggered and a time-triggered rekey,
  with the discarded remainder never served (16 bytes drawn, the clock moved two seconds, the
  next 16 come from a new key); a successful draw, then expiry, then a key read that fails
  partway, then a retry that succeeds, with nothing served from the old buffer or under the old
  key in between and the counter not restarted under it; a generation cut at the 1 MiB boundary
  comes from two keys and the budget counts refills and direct generation alike; the complete
  rekey path's allocations measured with `testing.AllocsPerRun` and zero between rekeys.
  Selection: a subprocess with
  `GODEBUG=cpu.aes=off` and a build with `-tags purego` both select the OS source,
  `GODEBUG=fips140=on` selects it, and on this machine the default selects the stream (the
  negative controls the owner required).
- Benchmarks in `uuidgen_bench_test.go` gain `BenchmarkParallelNewV4`, `BenchmarkParallelNewV7`
  and a mixed v4/v5/v7 parallel benchmark reporting allocations, with `runtime.MemStats` retained
  bytes and GC count printed by the harness. The README's contention and batch numbers come from
  these, on the shipped binding, never from the spike.
- Negative controls, each run once and recorded in the plan: a changed byte in the generated
  core fails the check; a corrupted corpus vector fails; the suite under TinyGo without the mount
  fails on the corpus; the stack one size below the measured minimum panics; a removed export
  fails `check_exports.py` at each new site.
- What only CI proves: the cross-compile sweep, the three wasm runtimes, iOS and Android, the
  Forge's release guard against a tree with no `go/staticlib`, the prepare-release Go step. The
  report says which.

## Docs and receipts

- `go/README.md`, rewritten: install (`go get`, no compiler, `CGO_ENABLED` irrelevant, two
  dependencies named); what the binding is (the translated core, the sandbox, what wasm2go's
  pre-1.0 status costs); randomness (the exact feature rule per architecture, the FIPS, purego
  and TinyGo exclusions, the budget, the rekey allocations, no fork guard, why not ChaCha); the
  primary and the pool with the measured parallel numbers; memory in the three numbers, the
  caller's buffer separately; `MaxV5Name` and `ErrNameTooLong`; platforms (every GOOS/GOARCH,
  iOS needing only Xcode's linker, Android without the NDK, the browser and WASI under stock Go
  and TinyGo with their numbers); building and regenerating (the pinned rustc, the stack size and
  its margin); benchmarks from the shipped binding on the M6 with cgo's 1.0 numbers kept beside
  them as the before, the batch gap and the browser's cost stated as losses; provenance, which
  now means the regeneration check and the attested wasm it is made from.
- Root `README.md`: the Go rows of 'Platform support' and the Go benchmark section, re-measured
  on the M6 and labelled; the "control group" framing is rewritten to what the numbers now say
  and the losses are printed next to the wins. `docs/platforms.md` (the Go cells, the WebAssembly
  row) and `docs/benchmarks.md` (the Go table, M6-labelled; x64 rows marked as awaiting the XPS
  run). `rust/README.md` and `rust/src/lib.rs` where they list cgo among the loaders.
- `CHANGELOG.md`: one entry under Unreleased for the Go rewrite (the removal of cgo and the
  archives, the random rule, the pool, `ErrNameTooLong` and `MaxV5Name`, the batch gap with
  numbers, the new targets, the second dependency) and the browser line of the batch-walk entry
  replaced by the raw-wasm measurement.
- `ROADMAP.md`: items 6 and 7 closed with this session's tables; item 4's "CRAN is done"
  corrected (CRAN is a 1.1 first); a note that a streaming v5 would restore the old name range.
- `CLAUDE.md`: "Never hand-edit" gains `go/internal/core/core.go` and the regenerate command;
  "Lists that move together" gains the rustc pin, the wasm2go pin, and the AES feature rule
  against Go's source; "Local loop" replaces the Go paragraph; "Open follow-ups" drops the Go
  items.
- `.github/RELEASING.md`: no Go binaries to attest; the `go/vX.Y.Z` tag stays, since the module
  lives under `go/`; the bump commit carries the regenerated core.
- The spike: `walk-ab.sh` made executable; a raw-wasm leg that times the core's own
  `wasm32-wasip1` module, base against tree, in V8 (node, `random_get` from
  `crypto.getRandomValues`) and in wasmtime (the crate's Criterion bench built for wasip1, or a
  tiny wasip1 binary in the harness if Criterion will not build there); `patch` added to the
  Docker recipe; the parallel v4 benchmarks kept; the README's decision closed with this session's
  tables, labelled as the spike wrapper's, and the x64 columns left for the owner's XPS run.

## Risks

- **wasm2go is pre-1.0.** Mitigated by committing its output; a dead wasm2go freezes regeneration
  until replaced, and the binding keeps working.
- **The rustc pin drifts by hand.** Dependabot cannot bump it; it moves when the owner regenerates,
  like `dtolnay/rust-toolchain`. The check makes a stale pin visible, never silent.
- **Go's AES feature rule changes under us.** The comment pins the Go version and source line;
  the Go floor bump is the trigger to re-read it. A rule that drifts fails safe only if it
  becomes stricter, so the re-check is a listed duty, not a hope.
- **The pool does not deliver.** Then the measurement says so, the free list is tried, and if
  neither closes the gap the README prints the parallel v4 number beside cgo's as a loss.
- **A stack overflow on an untested path** panics inside the sandbox. The 8x margin over the
  measured minimum and the maximum-size paths in the measurement keep it theoretical, and the
  wording claims no more.
- **`sync.Pool` churn** re-keys a stream and allocates 192 KiB after a GC under load. Bounded
  and measured in the parallel benchmarks; the free list is the fallback.
- **TinyGo gaps** (`TryLock`, `crypto/fips140`, `x/sys/cpu`) are closed by build-tagged files,
  found by the suite under TinyGo in CI rather than by a user.
- **Android internal linking** may not give 16 KB page alignment; the fallback keeps the NDK
  link for the test binary alone and the README says the consumer's `go build` is plain.
