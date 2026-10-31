# Conformance corpus

These vectors are the contract every binding replays. They pin byte layouts that HyperUuid guarantees and that other systems persist, such as SQL Server clustered keys and v5 ids baked into migrations. A refactor can pass every property test, round-trips and sort order alike, and still move one byte. Fixed vectors catch that.

The vectors come from `oracle.py`, which shares no code with the core:
- v5 is SHA-1 from Python's `hashlib`.
- The SQL Server layout is derived in bit strings from RFC 9562's field tables: a v7's 122 free bits, in order, poured into `SqlGuid`'s compare slots around a version 8 nibble and the variant. The core shifts integers instead. The bytes are also checked against a line-for-line port of Norse Svartálfheim's `SequentialGuidBytes`, the permutation SequentialGuid uses. That code works in .NET's mixed-endian `Guid` layout, so it is a different derivation again, and it differs only in keeping a 7 where HyperUuid writes the 8.
- The v7 generation layout comes from RFC 9562's prose.

The core matches the oracle byte for byte, and the RFC 9562 Appendix A vectors are included verbatim.

## Conventions

- Every UUID, and every other byte string, is 32 (or 12) lowercase hex digits of the bytes in memory order, with no dashes. For an `rfc` value that is RFC 9562 network order. For a `sql` value it is the bytes `to_sql_order` writes, which are SQL Server's `uniqueidentifier` wire bytes. A binding whose platform UUID type is mixed-endian (.NET `Guid`) reads `sql` hex exactly the way its own `V7FromSqlOrder(Guid)` reads a `Guid`, not as RFC text. In C#, a `sql` value is `Convert.ToHexStringLower(guid.ToByteArray())` of the `Guid` that `V7ToSqlOrder` returns, not that `Guid`'s `ToString()` with the dashes removed.
- `layout` is `"rfc9562"` or `"sql_server"`.
- `unix_millis` is milliseconds since the Unix epoch. `null` means "no timestamp".

## Files

| File | Vector | Replayed by |
|---|---|---|
| `v5.json` | `namespace` (`dns`/`url`/`oid`/`x500`), `name_hex` (the raw name bytes), `name` (the same bytes as text, present only when they are valid UTF-8), `expect` | every binding, through both the bytes and text doors where a binding has both |
| `v7_layout.json` | `unix_millis`, `counter` (26-bit), `entropy` (6 bytes), `expect` | Rust only (in-crate): generation is random, so this pins the deterministic half that the public API doesn't expose |
| `sql_order.json` | `rfc` (a version 7), `sql` | every binding, in both directions |
| `sql_order_refused.json` | `direction` (`to_sql`/`from_sql`), `uuid`, `note` | every binding: that conversion of `uuid` fails with the binding's wrong-version error |
| `timestamp.json` | `uuid`, `layout`, `version` (as `version_in(layout)` reads it), `unix_millis`, the 48-bit field as the per-version millisecond getter reads it | every binding, through its generic timestamp getter, its per-version millisecond getter, and its layout-aware doors. A row whose `unix_millis` is past the generation cap, 253402300799999 (README "v7 ordering, precisely"), has no timestamp at the generic getter; the per-version getter reads the field |
| `inspect.json` | `uuid`, `layout`, `version`, `is_rfc` (`is_rfc_in(version, layout)`), and in RFC layout also `variant` (`ncs`/`rfc9562`/`microsoft`/`future`) | every binding, through its version, variant and IsRfc doors. Also check that `is_rfc_in` is false for every other version. `note` is for people reading the file |

The TypeScript and R suites replay the five "every binding" files in `typescript/tests/corpus.test.ts` and `r/tests/testthat/test-corpus.R`. Under `R CMD check` an R package's tests see only the package's own tree, so `r/tools/vendor.sh` stages a copy of those five under `r/tests/testthat/corpus/` (gitignored) for the tarball to carry; a checkout without the copy reads this directory.

Every `sql` value is an RFC 9562 version 8 read in `Guid` order. In SQL Server layout, `version` is the version the value was converted from, 7; bytes without the layout's 8 and variant read as version 0 with no timestamp. Whether a value really is in SQL order is the caller's knowledge, not the value's: RFC-order bytes can carry the 8 and the variant where the layout puts them, and a random v4 does one time in 16. A value 0.8.0 converted, which carries a 7 where the 8 is, reads as version 0 until its nibble is changed. That is why the `sql_server` rows for values that aren't SQL-ordered use fixed vectors, and why `inspect.json` pins RFC A.4's v5, which happens to read as a SQL-ordered v7.

## Changing the corpus

`python3 corpus/oracle.py` re-derives every file and fails if any file differs from what the oracle computes. To add a vector, add it to the oracle and run `--write`. Vectors are only ever added. A changed `expect` means persisted data changed meaning, which is a breaking change no matter what the tests say.
