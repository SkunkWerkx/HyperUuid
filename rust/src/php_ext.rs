//! Benchmark spike: this crate linked straight into a Zend extension via `ext-php-rs`,
//! mirroring the Python (PyO3) / Ruby (Magnus) native-backend pattern, to measure it
//! against PHP's `ext-ffi` path (`../src/Runtime.php`). `Runtime.php` already measured the
//! raw `ext-ffi` crossing at ~105ns — "already extension-class" — unlike ctypes (~1µs) and
//! Fiddle (~1.6µs), which is *why* Python and Ruby got a native backend and PHP didn't.
//! This module exists to check that reasoning against real numbers rather than leave it
//! asserted.
//!
//! Deliberately not wired into the `hyperuuid/hyperuuid` Composer package: this is a
//! benchmark-only spike, not a second production backend. Functions mirror
//! `Runtime.php`'s raw-bytes shape (16-byte binary strings in/out) so the two paths are
//! compared at the same layer — no `Uuid` value-object construction on either side.

use ext_php_rs::binary::Binary;
use ext_php_rs::prelude::*;

use crate::{v4, v5, v6, v7, Uuid};

fn uuid_arg(bytes: &Binary<u8>) -> PhpResult<Uuid> {
    let array: [u8; 16] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| PhpException::default("bytes must be exactly 16 bytes".into()))?;
    Ok(Uuid::from_bytes(array))
}

#[php_function]
#[php(name = "hyperuuid_native_new_v4")]
pub fn hyperuuid_native_new_v4() -> PhpResult<Binary<u8>> {
    v4::new_v4()
        .map(|id| Binary::from(id.as_bytes().to_vec()))
        .map_err(|e| PhpException::default(format!("uuid_new_v4 failed: {e}")))
}

#[php_function]
#[php(name = "hyperuuid_native_new_v5")]
pub fn hyperuuid_native_new_v5(namespace: Binary<u8>, name: Binary<u8>) -> PhpResult<Binary<u8>> {
    let namespace = uuid_arg(&namespace)?;
    let id = v5::new_v5(namespace, name.as_slice());
    Ok(Binary::from(id.as_bytes().to_vec()))
}

#[php_function]
#[php(name = "hyperuuid_native_new_v6")]
pub fn hyperuuid_native_new_v6(unix_millis: u64) -> PhpResult<Binary<u8>> {
    match v6::new_v6(unix_millis) {
        Ok(id) => Ok(Binary::from(id.as_bytes().to_vec())),
        Err(v6::NewV6Error::TimestampOutOfRange) => Err(PhpException::default(
            "unix_millis does not fit the 60-bit v6 timestamp field".into(),
        )),
        Err(v6::NewV6Error::Random(e)) => {
            Err(PhpException::default(format!("uuid_new_v6 failed: {e}")))
        }
    }
}

#[php_function]
#[php(name = "hyperuuid_native_v6_unix_millis")]
pub fn hyperuuid_native_v6_unix_millis(bytes: Binary<u8>) -> PhpResult<u64> {
    Ok(v6::unix_millis(&uuid_arg(&bytes)?))
}

#[php_function]
#[php(name = "hyperuuid_native_new_v6_batch")]
pub fn hyperuuid_native_new_v6_batch(count: u32, unix_millis: u64) -> PhpResult<Binary<u8>> {
    if count == 0 {
        return Ok(Binary::from(Vec::new()));
    }
    let mut out = vec![0u8; count as usize * 16];
    match v6::new_v6_batch(unix_millis, count, &mut out) {
        Ok(()) => Ok(Binary::from(out)),
        Err(v6::NewV6Error::TimestampOutOfRange) => Err(PhpException::default(
            "unix_millis does not fit the 60-bit v6 timestamp field".into(),
        )),
        Err(v6::NewV6Error::Random(e)) => Err(PhpException::default(format!(
            "uuid_new_v6_batch failed: {e}"
        ))),
    }
}

#[php_function]
#[php(name = "hyperuuid_native_new_v7")]
pub fn hyperuuid_native_new_v7(unix_millis: u64) -> PhpResult<Binary<u8>> {
    match v7::new_v7(unix_millis) {
        Ok(id) => Ok(Binary::from(id.as_bytes().to_vec())),
        Err(v7::NewV7Error::TimestampOutOfRange) => Err(PhpException::default(
            "unix_millis must fit within the RFC 9562 48-bit field".into(),
        )),
        Err(v7::NewV7Error::Random(e)) => {
            Err(PhpException::default(format!("uuid_new_v7 failed: {e}")))
        }
    }
}

#[php_function]
#[php(name = "hyperuuid_native_v7_unix_millis")]
pub fn hyperuuid_native_v7_unix_millis(bytes: Binary<u8>) -> PhpResult<u64> {
    Ok(v7::unix_millis(&uuid_arg(&bytes)?))
}

#[php_function]
#[php(name = "hyperuuid_native_new_v7_batch")]
pub fn hyperuuid_native_new_v7_batch(count: u32, unix_millis: u64) -> PhpResult<Binary<u8>> {
    if count == 0 {
        return Ok(Binary::from(Vec::new()));
    }
    let mut out = vec![0u8; count as usize * 16];
    match v7::new_v7_batch(unix_millis, count, &mut out) {
        Ok(()) => Ok(Binary::from(out)),
        Err(v7::NewV7Error::TimestampOutOfRange) => Err(PhpException::default(
            "unix_millis must fit within the RFC 9562 48-bit field".into(),
        )),
        Err(v7::NewV7Error::Random(e)) => Err(PhpException::default(format!(
            "uuid_new_v7_batch failed: {e}"
        ))),
    }
}

#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(hyperuuid_native_new_v4))
        .function(wrap_function!(hyperuuid_native_new_v5))
        .function(wrap_function!(hyperuuid_native_new_v6))
        .function(wrap_function!(hyperuuid_native_v6_unix_millis))
        .function(wrap_function!(hyperuuid_native_new_v6_batch))
        .function(wrap_function!(hyperuuid_native_new_v7))
        .function(wrap_function!(hyperuuid_native_v7_unix_millis))
        .function(wrap_function!(hyperuuid_native_new_v7_batch))
}
