//! The parts of the 1.0 surface that ext-php-rs's builders can't declare, written into the
//! registered class entries once every module's startup has run (see
//! `uuid::allow_serialization` for when that is, and the `dl()` fallback):
//!
//! - `Uuid` accepts `unserialize` (`uuid::clear_not_serializable`);
//! - `UuidLayout` and `UuidVariant` are `final`, as every PHP enum is: ext-php-rs registers an
//!   enum through `zend_register_internal_enum` without the flag the compiler gives a declared
//!   one;
//! - `newV7`, `newV7Batch` and `newV7BatchBytes` declare `$unixMillis` as 1.0 did,
//!   `DateTimeInterface|int|null`. ext-php-rs's argument info carries one type, so the
//!   functions are registered with `mixed`; this rewrites the type that reflection, a debug
//!   PHP's internal argument check and the doors' own argument check (`args.rs`) read;
//! - `Uuid`'s defaulted `$throwOnMismatch` and `$layout` parameters are not nullable, as 1.0's
//!   were not. Their wrapper types tell ext-php-rs they are (`args::Flag`), so that an explicit
//!   `null` reaches the argument check and is refused in 1.0's words; this clears the bit.
//!
//! `tests/SurfaceTest.php` holds the result to 1.0's surface.

use std::ptr;

use ext_php_rs::class::RegisteredClass;
use ext_php_rs::ffi::{
    _ZEND_TYPE_NAME_BIT, IS_LONG, IS_NULL, MAY_BE_ANY, ZEND_ACC_FINAL, zend_class_entry,
    zend_function,
};

use super::enums::{UuidLayout, UuidVariant};
use super::generate::HyperUuid;
use super::uuid::{Uuid, clear_not_serializable};

/// `_ZEND_TYPE_MASK` (`zend_types.h`): the type bits of a `zend_type`'s mask, below the extra
/// flags (send mode, variadic) that share the word.
const TYPE_MASK: u32 = (1 << 25) - 1;

/// The methods whose parameter (by position) is 1.0's `DateTimeInterface|int|null $unixMillis`.
const UNIX_MILLIS_PARAMS: [(&str, usize); 3] =
    [("newv7", 0), ("newv7batch", 1), ("newv7batchbytes", 1)];

/// `Uuid`'s defaulted parameters (method, position) that 1.0 declared non-nullable.
const NOT_NULL_PARAMS: [(&str, usize); 5] = [
    ("version", 0),
    ("isrfc", 1),
    ("unixmillis", 0),
    ("timestamp", 0),
    ("timestamp", 1),
];

/// Finishes registration. Called once, after every module's startup, or for a `dl()` load in
/// its first RINIT; each step checks the entry's current state, so a second call changes
/// nothing.
pub(crate) fn finish() {
    clear_not_serializable();
    for ce in [own_entry::<UuidLayout>(), own_entry::<UuidVariant>()]
        .into_iter()
        .flatten()
    {
        // SAFETY: a live, registered class entry, written before any request on the startup
        // thread (or in a `dl()` load's own RINIT; NTS only).
        unsafe { (*ce).ce_flags |= ZEND_ACC_FINAL };
    }
    widen_unix_millis();
    refuse_null();
}

/// The class entry ext-php-rs registered for `T`, found in the compiler's class table
/// (`CG(class_table)`, complete once every MINIT has run), or `None` if a namesake took it.
pub(crate) fn own_entry<T: RegisteredClass>() -> Option<*mut zend_class_entry> {
    let found = class_table_entry(&T::CLASS_NAME.to_ascii_lowercase())?;
    ptr::eq(found.cast_const(), T::get_metadata().ce()).then_some(found)
}

/// A class entry in `CG(class_table)` by its lowercase name.
fn class_table_entry(lower: &str) -> Option<*mut zend_class_entry> {
    // SAFETY: the compiler globals live for the process; the class table is complete once
    // every MINIT has run, and nothing else writes it during startup or RINIT.
    let table = unsafe { ext_php_rs::ffi::ext_php_rs_compiler_globals().as_ref() }
        .and_then(|globals| unsafe { globals.class_table.as_ref() })?;
    // SAFETY: the class table's values are `IS_PTR` zvals holding class entries.
    table
        .get(lower)
        .and_then(|zv| unsafe { zv.ptr::<zend_class_entry>() })
}

/// Rewrites each `$unixMillis` parameter's type from `mixed` to `DateTimeInterface|int|null`:
/// the class name, as a type holds one, is `DateTimeInterface`'s own interned name, so the
/// engine's release of the type at shutdown (a no-op for an interned string) frees nothing.
fn widen_unix_millis() {
    let Some(class) = own_entry::<HyperUuid>() else {
        return;
    };
    let Some(date_time) = class_table_entry("datetimeinterface") else {
        return;
    };
    // SAFETY: ext/date's class entry, registered before this extension's MINIT.
    let name = unsafe { (*date_time).name };
    if name.is_null() {
        return;
    }
    // SAFETY: the live class entry; its function table maps lowercase names to `IS_PTR`
    // zvals holding the methods.
    let functions = unsafe { &(*class).function_table };
    for (method, position) in UNIX_MILLIS_PARAMS {
        let Some(function) = functions
            .get(method)
            .and_then(|zv| unsafe { zv.ptr::<zend_function>() })
        else {
            continue;
        };
        // SAFETY: an internal method of this class: `arg_info` points at `num_args` entries
        // (the engine's copy, made at registration for a function with typed parameters),
        // and the union's `common` view shares the internal function's layout.
        unsafe {
            let common = &(*function).common;
            if position >= common.num_args as usize || common.arg_info.is_null() {
                continue;
            }
            let ty = &mut (*common.arg_info.add(position)).type_;
            // Only the `mixed` ext-php-rs registered, never a type already rewritten.
            if ty.type_mask & TYPE_MASK != MAY_BE_ANY || !ty.ptr.is_null() {
                continue;
            }
            ty.ptr = name.cast();
            ty.type_mask =
                (ty.type_mask & !TYPE_MASK) | _ZEND_TYPE_NAME_BIT | (1 << IS_LONG) | (1 << IS_NULL);
        }
    }
}

/// Clears the nullable bit of each of `NOT_NULL_PARAMS`, leaving the rest of the type alone.
fn refuse_null() {
    let Some(class) = own_entry::<Uuid>() else {
        return;
    };
    // SAFETY: as in `widen_unix_millis`.
    let functions = unsafe { &(*class).function_table };
    for (method, position) in NOT_NULL_PARAMS {
        let Some(function) = functions
            .get(method)
            .and_then(|zv| unsafe { zv.ptr::<zend_function>() })
        else {
            continue;
        };
        // SAFETY: as in `widen_unix_millis`.
        unsafe {
            let common = &(*function).common;
            if position >= common.num_args as usize || common.arg_info.is_null() {
                continue;
            }
            let ty = &mut (*common.arg_info.add(position)).type_;
            // Only a type that names something besides null.
            if ty.type_mask & TYPE_MASK & !(1 << IS_NULL) != 0 {
                ty.type_mask &= !(1 << IS_NULL);
            }
        }
    }
}
