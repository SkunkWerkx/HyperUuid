//! The PHP binding: this crate linked straight into a Zend extension through `ext-php-rs`,
//! the same native-extension pattern as the Python (PyO3) and Ruby (Magnus) bindings. The
//! whole `HyperUuid\` API is defined here; the Composer package ships no PHP classes.

use std::panic::{AssertUnwindSafe, catch_unwind};

use ext_php_rs::exception::{PhpException, PhpResult};
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;

mod args;
mod enums;
mod errors;
mod generate;
mod registration;
mod uuid;

/// Runs a PHP-callable body and turns a panic into a `\RuntimeException`, so unwinding never
/// crosses the Zend engine's C frames. Every `#[php_function]` and `#[php_impl]` body in this
/// module is `guard(|| ...)` as its first expression.
pub(crate) fn guard<T>(f: impl FnOnce() -> PhpResult<T>) -> PhpResult<T> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(payload) => {
            let detail = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic payload".to_owned());
            Err(PhpException::new(
                format!("hyperuuid: internal panic: {detail}"),
                0,
                errors::runtime_exception(),
            ))
        }
    }
}

/// How PHP 8.3+ names a value in a type error (`zend_zval_value_name`): `true` and `false`
/// for booleans, and the class name for an object, up to its first NUL byte, as the engine's
/// `%s` prints it. (An anonymous class's name is `class@anonymous`, a NUL, then where it was
/// declared; a message holding the NUL can't be thrown at all.)
pub(crate) fn value_name(zv: &Zval) -> String {
    let name = if zv.is_null() {
        "null"
    } else if zv.is_true() {
        "true"
    } else if zv.is_false() {
        "false"
    } else if zv.is_long() {
        "int"
    } else if zv.is_double() {
        "float"
    } else if zv.is_string() {
        "string"
    } else if zv.is_array() {
        "array"
    } else if zv.is_resource() {
        "resource"
    } else if let Some(object) = zv.object() {
        let class = object
            .get_class_name()
            .unwrap_or_else(|_| "object".to_owned());
        return class.split('\0').next().unwrap_or_default().to_owned();
    } else {
        "mixed"
    };
    name.to_owned()
}

/// RSHUTDOWN: stops caching, then releases the request's cached `Uuid::nil()`, `Uuid::max()`
/// and `Namespaces` values while the engine can still free them. A call after this (from
/// another extension's request shutdown) gets a fresh, uncached value.
extern "C" fn request_shutdown(_type: i32, _module_number: i32) -> i32 {
    // A panic must not unwind into the engine; dropping zvals has nothing to report.
    let _ = catch_unwind(uuid::release_constants);
    0
}

/// Post-RSHUTDOWN, which `php_request_shutdown` runs for every request, after every module's
/// RSHUTDOWN (or in place of them all, when a module's RINIT failed) and after the executor has
/// freed the request's objects: ends the request for the cache and forgets, without releasing,
/// anything still cached (see `uuid::end_request`).
extern "C" fn post_deactivate() -> i32 {
    let _ = catch_unwind(uuid::end_request);
    0
}

/// RINIT: forgets, without releasing, any cached `Uuid::nil()`, `Uuid::max()` or `Namespaces`
/// value a request that never reached post-RSHUTDOWN left behind (see
/// `uuid::forget_stale_constants`), and, for a `dl()` load only, lets `Uuid` be unserialized
/// (see `uuid::allow_serialization`). Caching starts only after that.
extern "C" fn request_startup(_type: i32, _module_number: i32) -> i32 {
    let _ = catch_unwind(uuid::forget_stale_constants);
    uuid::allow_serialization_late();
    uuid::begin_request();
    0
}

/// MINIT, run before ext-php-rs registers the classes: registers the exception classes as
/// plain `\RuntimeException` subclasses (see `errors::Thrown`), and schedules `Uuid`'s
/// serialization switch for after every module's MINIT (see `uuid::allow_serialization`).
/// A failed registration fails MINIT, so the extension doesn't load without its exceptions.
extern "C" fn module_startup(module_type: i32, _module_number: i32) -> i32 {
    if !matches!(catch_unwind(errors::register_exceptions), Ok(Ok(()))) {
        return -1;
    }
    let _ = catch_unwind(|| uuid::allow_serialization(module_type));
    0
}

#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
        .version(env!("CARGO_PKG_VERSION"))
        .startup_function(module_startup)
        .request_startup_function(request_startup)
        .request_shutdown_function(request_shutdown)
        .post_deactivate_function(post_deactivate)
        .enumeration::<enums::UuidLayout>()
        .enumeration::<enums::UuidVariant>()
        .class::<uuid::Uuid>()
        .class::<generate::HyperUuid>()
        .class::<generate::Namespaces>()
}
