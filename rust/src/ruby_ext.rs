//! The Ruby backend: this crate linked straight into a Ruby native extension via Magnus —
//! the PyO3 play (already run for this repo's Python binding), run for Ruby. Fiddle's
//! per-call marshalling floor is ~1.6 µs of interpreted argument packing; an extension
//! method is an ordinary C-function call, so generation drops to the cost of the core call
//! plus building one 16-byte Ruby String.
//!
//! On require (after `lib/hyperuuid.rb` has defined the pure-Fiddle module), this extension
//! redefines the `HyperUuid::Runtime` singleton methods **in place** — no delegation layer,
//! no second surface. Everything above Runtime (the `Uuid` class, the top-level `HyperUuid`
//! doors, batch slicing) is shared byte-for-byte between backends, which is exactly what
//! keeps them provably in agreement. Exceptions stay the package's own
//! `Runtime::RandomSourceError` / `Runtime::TimestampOutOfRangeError`.
//! `HYPERUUID_PURE=1` (checked Ruby-side) keeps Fiddle.

use std::sync::OnceLock;

use magnus::value::Opaque;
use magnus::{function, prelude::*, Error, ExceptionClass, RModule, RString, Ruby};

use crate as core;

/// Constant-referenced classes are anchored by Ruby constants and never collected, so
/// caching them is GC-safe.
struct Cached {
    random_source_error: Opaque<ExceptionClass>,
    timestamp_out_of_range_error: Opaque<ExceptionClass>,
}

static CACHED: OnceLock<Cached> = OnceLock::new();

fn cached() -> &'static Cached {
    CACHED.get().expect("hyperuuid_native used before init")
}

fn random_source_error(ruby: &Ruby, message: String) -> Error {
    Error::new(ruby.get_inner(cached().random_source_error), message)
}

fn timestamp_out_of_range(ruby: &Ruby, message: &'static str) -> Error {
    Error::new(ruby.get_inner(cached().timestamp_out_of_range_error), message)
}

/// Borrows the RString's bytes only long enough to copy/parse them — no Ruby calls happen
/// inside the borrow, so the slice cannot be invalidated mid-use.
fn uuid_arg(ruby: &Ruby, bytes: RString) -> Result<core::Uuid, Error> {
    let slice = unsafe { bytes.as_slice() };
    let array: [u8; 16] = slice.try_into().map_err(|_| {
        Error::new(ruby.exception_arg_error(), "bytes must be exactly 16 bytes")
    })?;
    Ok(core::Uuid::from_bytes(array))
}

fn uuid_string(ruby: &Ruby, uuid: core::Uuid) -> RString {
    ruby.str_from_slice(uuid.as_bytes())
}

fn new_v4(ruby: &Ruby) -> Result<RString, Error> {
    match core::v4::new_v4() {
        Ok(uuid) => Ok(uuid_string(ruby, uuid)),
        Err(e) => Err(random_source_error(ruby, format!("uuid_new_v4 failed: {e}"))),
    }
}

fn new_v5(ruby: &Ruby, namespace_bytes: RString, name_bytes: RString) -> Result<RString, Error> {
    let namespace = uuid_arg(ruby, namespace_bytes)?;
    let uuid = {
        let name = unsafe { name_bytes.as_slice() };
        core::v5::new_v5(namespace, name)
    };
    Ok(uuid_string(ruby, uuid))
}

fn new_v6(ruby: &Ruby, unix_millis: u64) -> Result<RString, Error> {
    match core::v6::new_v6(unix_millis) {
        Ok(uuid) => Ok(uuid_string(ruby, uuid)),
        Err(core::v6::NewV6Error::TimestampOutOfRange) => Err(timestamp_out_of_range(
            ruby,
            "unix_millis does not fit the 60-bit v6 timestamp field",
        )),
        Err(core::v6::NewV6Error::Random(e)) => {
            Err(random_source_error(ruby, format!("uuid_new_v6 failed: {e}")))
        }
    }
}

fn v6_unix_millis(ruby: &Ruby, bytes: RString) -> Result<u64, Error> {
    Ok(core::v6::unix_millis(&uuid_arg(ruby, bytes)?))
}

fn new_v6_batch(ruby: &Ruby, count: u32, unix_millis: u64) -> Result<RString, Error> {
    if count == 0 {
        return Ok(ruby.str_from_slice(&[]));
    }
    let mut out = vec![0u8; count as usize * 16];
    match core::v6::new_v6_batch(unix_millis, count, &mut out) {
        Ok(()) => Ok(ruby.str_from_slice(&out)),
        Err(core::v6::NewV6Error::TimestampOutOfRange) => Err(timestamp_out_of_range(
            ruby,
            "unix_millis does not fit the 60-bit v6 timestamp field",
        )),
        Err(core::v6::NewV6Error::Random(e)) => {
            Err(random_source_error(ruby, format!("uuid_new_v6_batch failed: {e}")))
        }
    }
}

fn new_v7(ruby: &Ruby, unix_millis: u64) -> Result<RString, Error> {
    match core::v7::new_v7(unix_millis) {
        Ok(uuid) => Ok(uuid_string(ruby, uuid)),
        Err(core::v7::NewV7Error::TimestampOutOfRange) => Err(timestamp_out_of_range(
            ruby,
            "unix_millis must fit within the RFC 9562 48-bit field",
        )),
        Err(core::v7::NewV7Error::Random(e)) => {
            Err(random_source_error(ruby, format!("uuid_new_v7 failed: {e}")))
        }
    }
}

fn v7_unix_millis(ruby: &Ruby, bytes: RString) -> Result<u64, Error> {
    Ok(core::v7::unix_millis(&uuid_arg(ruby, bytes)?))
}

fn new_v7_batch(ruby: &Ruby, count: u32, unix_millis: u64) -> Result<RString, Error> {
    if count == 0 {
        return Ok(ruby.str_from_slice(&[]));
    }
    let mut out = vec![0u8; count as usize * 16];
    match core::v7::new_v7_batch(unix_millis, count, &mut out) {
        Ok(()) => Ok(ruby.str_from_slice(&out)),
        Err(core::v7::NewV7Error::TimestampOutOfRange) => Err(timestamp_out_of_range(
            ruby,
            "unix_millis must fit within the RFC 9562 48-bit field",
        )),
        Err(core::v7::NewV7Error::Random(e)) => {
            Err(random_source_error(ruby, format!("uuid_new_v7_batch failed: {e}")))
        }
    }
}

fn v7_to_sql_order(ruby: &Ruby, bytes: RString) -> Result<RString, Error> {
    Ok(uuid_string(ruby, core::v7::to_sql_order(&uuid_arg(ruby, bytes)?)))
}

fn v7_to_rfc_order(ruby: &Ruby, bytes: RString) -> Result<RString, Error> {
    Ok(uuid_string(ruby, core::v7::to_rfc_order(&uuid_arg(ruby, bytes)?)))
}

fn v6_to_sql_order(ruby: &Ruby, bytes: RString) -> Result<RString, Error> {
    Ok(uuid_string(ruby, core::v6::to_sql_order(&uuid_arg(ruby, bytes)?)))
}

fn v6_to_rfc_order(ruby: &Ruby, bytes: RString) -> Result<RString, Error> {
    Ok(uuid_string(ruby, core::v6::to_rfc_order(&uuid_arg(ruby, bytes)?)))
}

#[magnus::init(name = "hyperuuid_native")]
fn init(ruby: &Ruby) -> Result<(), Error> {
    // rb_define_module returns the existing module — lib/hyperuuid.rb has already defined
    // the pure-Fiddle Runtime; these redefinitions replace its methods in place.
    let hyperuuid = ruby.define_module("HyperUuid")?;
    let runtime: RModule = hyperuuid.define_module("Runtime")?;
    let _ = CACHED.set(Cached {
        random_source_error: Opaque::from(
            runtime.const_get::<_, ExceptionClass>("RandomSourceError")?,
        ),
        timestamp_out_of_range_error: Opaque::from(
            runtime.const_get::<_, ExceptionClass>("TimestampOutOfRangeError")?,
        ),
    });
    runtime.define_singleton_method("new_v4", function!(new_v4, 0))?;
    runtime.define_singleton_method("new_v5", function!(new_v5, 2))?;
    runtime.define_singleton_method("new_v6", function!(new_v6, 1))?;
    runtime.define_singleton_method("v6_unix_millis", function!(v6_unix_millis, 1))?;
    runtime.define_singleton_method("new_v6_batch", function!(new_v6_batch, 2))?;
    runtime.define_singleton_method("new_v7", function!(new_v7, 1))?;
    runtime.define_singleton_method("v7_unix_millis", function!(v7_unix_millis, 1))?;
    runtime.define_singleton_method("new_v7_batch", function!(new_v7_batch, 2))?;
    runtime.define_singleton_method("v7_to_sql_order", function!(v7_to_sql_order, 1))?;
    runtime.define_singleton_method("v7_to_rfc_order", function!(v7_to_rfc_order, 1))?;
    runtime.define_singleton_method("v6_to_sql_order", function!(v6_to_sql_order, 1))?;
    runtime.define_singleton_method("v6_to_rfc_order", function!(v6_to_rfc_order, 1))?;
    Ok(())
}
