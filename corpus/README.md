# Conformance corpus

These vectors are the contract every binding replays. They pin byte layouts and permutations that HyperUuid guarantees and that other systems persist, such as SQL Server clustered keys and v5 ids baked into migrations. A refactor can pass every property test, round-trips and sort order alike, and still move one byte. Fixed vectors catch that.

The vectors come from `oracle.py`, which shares no code with the core:
- v5 is SHA-1 from Python's `hashlib`.
- The v7 SQL Server permutation is a line-for-line port of Norse Svartálfheim's `SequentialGuidBytes`. That code works in .NET's mixed-endian `Guid` layout, so it is a different derivation from the core's.
- The v6 permutation and both layouts come from RFC 9562's prose.

The core matches the oracle byte for byte, and the RFC 9562 Appendix A vectors are included verbatim.

## Conventions

- Every UUID, and every other byte string, is 32 (or 12) lowercase hex digits of the bytes in memory order, with no dashes. For an `rfc` value that is RFC 9562 network order. For a `sql` value it is the bytes `to_sql_order` writes, which are SQL Server's `uniqueidentifier` wire bytes. A binding whose platform UUID type is mixed-endian (.NET `Guid`) reads `sql` hex exactly the way its own `V7FromSqlOrder(Guid)` reads a `Guid`, not as RFC text.
- `layout` is `"rfc9562"` or `"sql_server"`.
- `unix_millis` is milliseconds since the Unix epoch. `null` means "no timestamp".

## Files

| File | Vector | Replayed by |
|---|---|---|
| `v5.json` | `namespace` (`dns`/`url`/`oid`/`x500`), `name_hex` (the raw name bytes), `name` (the same bytes as text, present only when they are valid UTF-8), `expect` | every binding, through both the bytes and text doors where a binding has both |
| `v7_layout.json` | `unix_millis`, `counter` (26-bit), `entropy` (6 bytes), `expect` | Rust only (in-crate): generation is random, so this pins the deterministic half that the public API doesn't expose |
| `v6_layout.json` | `unix_millis`, `clock_seq` (14-bit), `node` (6 bytes, before the multicast bit is set), `expect` | Rust only (in-crate), for the same reason |
| `sql_order.json` | `version` (6 or 7), `rfc`, `sql` | every binding, in both directions |
| `timestamp.json` | `uuid`, `layout`, `version` (as `version_in(layout)` reads it), `unix_millis` | every binding, through its generic timestamp getter, its per-version millisecond getter, and its layout-aware doors |
| `inspect.json` | `uuid`, `layout`, `version`, `is_rfc` (`is_rfc_in(version, layout)`), and in RFC layout also `variant` (`ncs`/`rfc9562`/`microsoft`/`future`) | every binding, through its version, variant and IsRfc doors. Also check that `is_rfc_in` is false for every other version. `note` is for people reading the file |

In SQL Server layout the only versions that exist are 6 and 7. Bytes that don't form a SQL-ordered v6 or v7 read as version 0 with no timestamp. Whether a value really is in SQL order is the caller's knowledge, not the value's: RFC-order bytes can form a valid SQL-ordered v7, and a random v4 does one time in 16. That is why the `sql_server` rows for non-v6/v7 values use fixed vectors. `sql_order.json` and `inspect.json` include a v6 whose SQL octet 7 happens to read as a v7 version nibble, and it must still read as 6.

## Changing the corpus

`python3 corpus/oracle.py` re-derives every file and fails if any file differs from what the oracle computes. To add a vector, add it to the oracle and run `--write`. Vectors are only ever added. A changed `expect` means persisted data changed meaning, which is a breaking change no matter what the tests say.
