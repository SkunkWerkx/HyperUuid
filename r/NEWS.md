# hyperuuid 1.0.0

## Added

* First release of the R binding to the HyperUuid core, built from source with the
  vendored Rust crates (no network access at install).
* Generation: `uuid_v4()`, `uuid_v5()`, `uuid_v7()` (one id per element of a `time` vector,
  or a strictly increasing batch for a single time), and the raw forms `uuid_v4_raw()` and
  `uuid_v7_raw()`.
* SQL Server ordering: `uuid_to_sql_order()` and `uuid_from_sql_order()`, the version 8
  layout of a version 7 id that sorts by creation time in a `uniqueidentifier` index.
* Inspection: `uuid_version()`, `uuid_variant()`, `uuid_is_rfc()`, `uuid_timestamp()` and
  `uuid_v7_unix_millis()`, element-wise with `NA` for missing or malformed ids.
* Conversion: `uuid_to_raw()` and `uuid_from_raw()` between ids and a 16-row raw matrix.
* Constants: `uuid_nil`, `uuid_max`, `uuid_namespaces` and `uuid_v7_max_batch`.
* `hyperuuid_native_version()`: the version of the loaded Rust core.
