# hyperuuid

**This is the actual engine — not scaffolding for the other six bindings. `cargo add hyperuuid` pulls the exact same RFC 9562 core that C#, Java, Go, Swift, Ruby, PHP, and Python all call into via FFI, as a real Rust dependency with no FFI boundary at all.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation. `no_std`-friendly dependency set (`getrandom`, `sha1`, both `default-features = false`), zero unsafe in the public API, and empirically zero-allocation per call — not just claimed, asserted by a real counting-allocator test (`tests/allocation_free.rs`).

```rust
use hyperuuid::{v4, v5, v6, v7};

let id = v4::new_v4()?;
let id2 = v5::new_v5(v5::namespace::DNS, b"example.com");
let id3 = v6::new_v6(unix_millis)?;
let id4 = v7::now_v7()?;

let ts = v7::unix_millis(&id4); // recover the embedded timestamp
let sql_ordered = v7::to_sql_order(&id4); // byte order SQL Server's uniqueidentifier needs to sort by creation order

// One random-bytes fetch, one counter reservation for the whole batch:
let mut out = vec![0u8; 1000 * 16];
v7::new_v7_batch(unix_millis, 1000, &mut out)?;
```

`v5::namespace::{DNS, URL, OID, X500}` are RFC 9562 Section 6.6's well-known namespaces. `v6::unix_millis`/`v7::unix_millis` recover the embedded UTC timestamp from a version 6 or 7 UUID. `v7::to_sql_order`/`v7::to_rfc_order` convert a version 7 UUID to and from the byte order SQL Server's `uniqueidentifier` needs on the wire to sort by creation order — the same permutation this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid)/[Svartalfheim](https://github.com/NorseArchitecture/Svartalfheim) already use for C#, computed here once and exported over FFI so every binding in this repo gets it from one verified source. Verified against the real `System.Data.SqlTypes.SqlGuid` comparator in the C# binding's test suite; this crate's own test suite verifies the same sort behavior against a comparator replicating `SqlGuid`'s documented byte order. `new_v6_batch`/`new_v7_batch` generate `count` UUIDs into a caller-owned `&mut [u8]` sharing one timestamp capture and one counter reservation, instead of `count` of each — this is also the one deliberate exception to the zero-allocation claim: the scratch buffer itself allocates (or is caller-provided, as above), the generation loop inside it does not.

This crate is also what makes the rest of this repo possible: a single `cdylib` (`libhyperuuid.so`/`.dylib`/`.dll`) exports a plain C ABI, and every other language binding in this repo (`../csharp`, `../java`, `../go`, `../swift`, `../ruby`, `../php`, `../python`) calls straight into it — same address space, same generation logic, same test vectors, on every platform. See [the repo root README](../README.md) for the full picture.

## Why not the `uuid` crate?

[`uuid`](https://docs.rs/uuid) is the de facto standard and it's a good crate — v4/v5/v6/v7 are all real, all correct, all there under feature flags. This isn't a "the standard crate is bad" pitch. The real differences, measured head-to-head on the same machine in the same benchmark run (`cargo bench` — see below):

1. **No batch API.** `uuid` generates exactly one UUID per call, always. `new_v6_batch`/`new_v7_batch` here share one timestamp capture and one counter reservation across the whole batch — 2.5-3.6x faster than the equivalent loop of individual calls (real numbers below).
2. **v6/v7 generation is substantially faster here** — `uuid_crate_v6` measured 838 ns/op vs this crate's 54 ns/op (~15.6x); `uuid_crate_v7` measured 905 ns/op vs 68 ns/op (~13.2x). One caveat for fairness: `uuid`'s `Uuid::now_v6`/`now_v7` capture the current time themselves (an internal clock read) inside the timed region, while this benchmark calls this crate's `new_v6`/`new_v7` with a pre-supplied timestamp, since this crate has no `now_v6` of its own (only `v7::now_v7` exists as a convenience wrapper). A clock read alone doesn't explain a gap this size — `uuid`'s v6/v7 paths route through a shared, lock-guarded `ClockSequence`/context abstraction — but the comparison isn't perfectly apples-to-apples and is reported that way rather than smoothed over.
3. **v4/v5 are roughly a wash.** `uuid_crate_v4` (85.1 ns) vs this crate's `v4` (88.3 ns) — statistically close, no real win either way. v5: this crate at 81.1 ns vs `uuid`'s 135.3 ns (~1.7x) — a real but modest difference, most likely from `uuid`'s SHA-1 implementation and output-formatting path versus this crate's narrower one.
4. **Verified zero-allocation.** `tests/allocation_free.rs` wraps a counting `#[global_allocator]` around 1000 calls to each of v4/v5/v6/v7 and asserts zero heap allocations, then asserts the batch functions' scratch buffer *does* allocate — confirming that's the one deliberate exception, not an oversight. `uuid`'s docs don't make an allocation claim either way.
5. **Cross-language consistency**, if that matters to you specifically: this is the literal same code seven other language bindings in this repo call into via FFI — a Rust service and a Python/Go/C# service using this crate/its bindings agree byte-for-byte on v5 output for the same `(namespace, name)`, verified in CI. `uuid` has no reason to make that claim and doesn't try to.

The honest trade-off: this crate's public surface is much narrower than `uuid`'s — no `Builder`, no `fmt` customization, no `serde`/`arbitrary`/`zerocopy` integrations, no v1/v3/v8. If you need any of that, or you're not chasing v6/v7 throughput or batch generation specifically, `uuid` is the better default and is what most of the Rust ecosystem already expects to interoperate with.

## Benchmarks

`cargo bench` from this directory (Criterion; HTML reports land in `target/criterion/report/index.html`). All numbers below are linux-arm64, one run, reproducible on your own hardware with the same command.

### vs. the `uuid` crate (single-item)

| Version | This crate | `uuid` crate | Delta |
| --- | ---: | ---: | ---: |
| v4 | 88.3 ns | 85.1 ns | ~0.96x (uuid slightly faster) |
| v5 | 81.1 ns | 135.3 ns | **1.67x faster** |
| v6 | 53.8 ns | 838.2 ns | **15.6x faster**¹ |
| v7 | 68.4 ns | 904.5 ns | **13.2x faster**¹ |

¹ See the methodology caveat above — `uuid`'s v6/v7 capture the clock internally; this crate's benchmarked calls take a pre-supplied timestamp.

### Batch generation vs. an equivalent loop (this crate only — `uuid` has no batch API)

| Version | 1000 individual calls | `*_batch(1000)` | Speedup |
| --- | ---: | ---: | ---: |
| v6 | 52.5 µs | 20.7 µs | **2.5x** |
| v7 | 61.7 µs | 16.9 µs | **3.6x** |

Allocation-free claim: `cargo test --release --test allocation_free`.

## Install

```toml
[dependencies]
hyperuuid = "0.1"
```

Not yet published to crates.io — proven for now by CI building and testing this crate fresh on 6 real-hardware platform legs (`.github/workflows/build-packages.yml`) plus the full `cargo test`/`cargo bench` suite. See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.

## License

[MIT](../LICENSE)
