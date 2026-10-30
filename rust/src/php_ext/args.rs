//! Arguments as the 1.0 userland classes took them.
//!
//! 1.0's methods were PHP functions, so the engine checked each argument against the declared
//! type the way it checks a userland function's: an exact match passes; a caller without
//! `declare(strict_types=1)` gets the standard weak-mode coercion of scalars (a numeric string to
//! `int`, an `int` to `string`, anything scalar to `bool`, with the engine's deprecations); and
//! anything else is a `TypeError` worded `Class::method(): Argument #n ($name) must be of type T,
//! U given, called in F on line L`. ext-php-rs's argument conversion accepts only the exact Rust
//! type, so the parameters here are wrapper types whose conversion repeats that check first:
//!
//! - the declared type is the running function's own argument info, the one reflection and a
//!   debug PHP's internal argument check read, so what is accepted and what is declared can't
//!   drift apart (`registration.rs` finishes the declarations ext-php-rs can't write);
//! - strictness is the caller's, as `ZEND_ARG_USES_STRICT_TYPES()` reads it: the calling
//!   frame's function carries `ZEND_ACC_STRICT_TYPES`;
//! - the coercion is the engine's own: `zend_verify_scalar_type_hint`, called as the engine calls
//!   it for a userland parameter (`is_internal_arg` false, so `null` is never coerced), which
//!   converts the argument in place in the call frame and raises the engine's deprecations;
//! - the refusal is the engine's userland wording (`zend_verify_arg_error`), built here because
//!   that function asserts it is given a userland function.
//!
//! A ZEND_DEBUG PHP checks internal arguments before the handler runs, with internal rules
//! (`null` accepted for a scalar) and without side effects; whatever it refuses is refused here
//! too, so debug and release builds behave alike.

use std::ffi::CStr;
use std::mem::size_of;

use ext_php_rs::convert::FromZvalMut;
use ext_php_rs::exception::PhpException;
use ext_php_rs::ffi::{
    _ZEND_TYPE_LITERAL_NAME_BIT, _ZEND_TYPE_NAME_BIT, IS_DOUBLE, IS_FALSE, IS_LONG, IS_NULL,
    IS_STRING, IS_TRUE, ZEND_ACC_STRICT_TYPES, ZEND_INTERNAL_FUNCTION, zend_arg_info,
    zend_function, zend_string, zval,
};
use ext_php_rs::flags::DataType;
use ext_php_rs::types::{ZendStr, Zval};
use ext_php_rs::zend::{ClassEntry, ExecuteData, ExecutorGlobals, ce};

use super::enums::UuidLayout;
use super::uuid::Uuid;
use super::value_name;

// The engine's weak-mode check for a scalar parameter (`zend_execute.h`, PHP 8.3 to 8.5).
#[cfg_attr(windows, link(name = "php8"))]
unsafe extern "C" {
    fn zend_verify_scalar_type_hint(
        type_mask: u32,
        arg: *mut zval,
        strict: bool,
        is_internal_arg: bool,
    ) -> bool;
}

/// `ZEND_MM_ALIGNED_SIZE`: sizes in the call frame round up to 8 bytes.
const fn aligned(size: usize) -> usize {
    (size + 7) & !7
}

/// `ZEND_CALL_FRAME_SLOT`: the arguments start this many zvals past the frame's start.
const FRAME_SLOT: usize = (aligned(size_of::<ExecuteData>()) + aligned(size_of::<Zval>()) - 1)
    / aligned(size_of::<Zval>());

/// `_ZEND_TYPE_MASK`: the type bits of a `zend_type`'s mask.
const TYPE_MASK: u32 = (1 << 25) - 1;

/// The argument being converted, located in the running call: the frame, its function, and the
/// argument's 0-based position, found from where `arg` sits among the frame's arguments
/// (ext-php-rs converts each argument in place in the frame).
struct Located<'a> {
    frame: &'a ExecuteData,
    function: &'a zend_function,
    position: usize,
}

fn locate(arg: &Zval) -> Option<Located<'static>> {
    // SAFETY: during a call, the executor's current frame is the running call's, live for it.
    let frame = unsafe { ExecutorGlobals::get().current_execute_data.as_ref()? };
    // SAFETY: a running frame's function is set and outlives the call.
    let function = unsafe { frame.func.as_ref()? };
    let first = std::ptr::from_ref(frame)
        .cast::<Zval>()
        .wrapping_add(FRAME_SLOT);
    let offset = (std::ptr::from_ref(arg) as usize).checked_sub(first as usize)?;
    let position = offset / size_of::<Zval>();
    // SAFETY: every member of the union starts with the common header.
    let num_args = unsafe { function.common.num_args } as usize;
    // SAFETY: `This.u2.num_args` is the call's argument count while it runs.
    let passed = unsafe { frame.This.u2.num_args } as usize;
    (offset % size_of::<Zval>() == 0 && position < num_args && position < passed).then_some(
        Located {
            frame,
            function,
            position,
        },
    )
}

/// The parameter's declaration in the function's argument info.
fn arg_info<'a>(at: &Located<'a>) -> Option<&'a zend_arg_info> {
    // SAFETY: `arg_info` points at `num_args` entries; `position` is below it. An internal
    // function's entries are `zend_internal_arg_info`, whose layout this shares (the name is
    // read through `parameter_name`, which knows which it is).
    unsafe { at.function.common.arg_info.add(at.position).as_ref() }
}

/// A `zend_string`'s bytes as text.
///
/// # Safety
///
/// `s` must be null or point at a live `zend_string`.
unsafe fn zend_text(s: *const zend_string) -> String {
    // SAFETY: the caller's contract; `ZendStr` is `zend_string`.
    unsafe { s.cast::<ZendStr>().as_ref() }
        .map(|s| String::from_utf8_lossy(s.as_bytes()).into_owned())
        .unwrap_or_default()
}

/// The class name a type holds, if it holds one, as bytes.
fn type_class_name(info: &zend_arg_info) -> Option<&[u8]> {
    let ty = &info.type_;
    if ty.ptr.is_null() {
        return None;
    }
    if ty.type_mask & _ZEND_TYPE_NAME_BIT != 0 {
        // SAFETY: with the name bit set, `ptr` is the class name's `zend_string`, interned and
        // live for the process.
        return unsafe { ty.ptr.cast::<ZendStr>().as_ref() }.map(ZendStr::as_bytes);
    }
    if ty.type_mask & _ZEND_TYPE_LITERAL_NAME_BIT != 0 {
        // SAFETY: with the literal-name bit set, `ptr` is a NUL-terminated C string that lives
        // as long as the argument info.
        return Some(unsafe { CStr::from_ptr(ty.ptr.cast()) }.to_bytes());
    }
    None
}

/// The type as `zend_type_to_string` writes it, for the types this API declares: the class
/// name, then the scalar types in the engine's order, `null` last.
fn type_text(info: &zend_arg_info) -> String {
    let mask = info.type_.type_mask & TYPE_MASK;
    let mut parts: Vec<String> = type_class_name(info)
        .map(|name| String::from_utf8_lossy(name).into_owned())
        .into_iter()
        .collect();
    let bool_bits = (1 << IS_FALSE) | (1 << IS_TRUE);
    for (bits, name) in [
        (1 << IS_STRING, "string"),
        (1 << IS_LONG, "int"),
        (1 << IS_DOUBLE, "float"),
        (bool_bits, "bool"),
    ] {
        if mask & bits == bits {
            parts.push(name.to_owned());
        }
    }
    if mask & (1 << IS_NULL) != 0 {
        if parts.len() == 1 {
            return format!("?{}", parts[0]);
        }
        parts.push("null".to_owned());
    }
    if parts.is_empty() {
        return "mixed".to_owned();
    }
    parts.join("|")
}

/// The parameter's name, from either kind of argument info.
fn parameter_name(at: &Located<'_>, info: &zend_arg_info) -> String {
    // SAFETY: the common header's type says which argument info the function carries.
    if u32::from(unsafe { at.function.common.type_ }) == ZEND_INTERNAL_FUNCTION {
        let internal = std::ptr::from_ref(info).cast::<ext_php_rs::ffi::zend_internal_arg_info>();
        // SAFETY: an internal function's entries are `zend_internal_arg_info`, whose name is a
        // NUL-terminated C string.
        unsafe { (*internal).name.as_ref() }
            .map(|name| {
                unsafe { CStr::from_ptr(name) }
                    .to_string_lossy()
                    .into_owned()
            })
            .unwrap_or_default()
    } else {
        // SAFETY: a userland function's entries name the parameter with a `zend_string`.
        unsafe { zend_text(info.name) }
    }
}

/// `Class::method`, as the engine names the running method in an argument error.
fn method_name(at: &Located<'_>) -> String {
    // SAFETY: the common header's name and scope are set for a method and live for the call.
    let (name, scope) = unsafe { (at.function.common.function_name, at.function.common.scope) };
    // SAFETY: the function's name is a live `zend_string`.
    let name = unsafe { zend_text(name) };
    // SAFETY: a scope, when set, is a live class entry with a name.
    match unsafe { scope.as_ref() } {
        Some(scope) => format!("{}::{name}", unsafe { zend_text(scope.name) }),
        None => name,
    }
}

/// `, called in F on line L` when the caller is userland code, as `zend_verify_arg_error` adds.
fn called_in(at: &Located<'_>) -> String {
    // SAFETY: a running frame's previous frame, its function and its current opline are live
    // while the call runs.
    unsafe {
        let Some(caller) = at.frame.prev_execute_data.as_ref() else {
            return String::new();
        };
        let Some(function) = caller.func.as_ref() else {
            return String::new();
        };
        // `ZEND_USER_CODE()`: user functions and eval'd code have an even type.
        if function.common.type_ & 1 != 0 {
            return String::new();
        }
        let Some(opline) = caller.opline.as_ref() else {
            return String::new();
        };
        format!(
            ", called in {} on line {}",
            zend_text(function.op_array.filename),
            opline.lineno
        )
    }
}

/// `ZEND_ARG_USES_STRICT_TYPES()`: whether the calling frame's code declared strict types.
fn caller_is_strict(at: &Located<'_>) -> bool {
    // SAFETY: as in `called_in`.
    unsafe {
        at.frame
            .prev_execute_data
            .as_ref()
            .and_then(|caller| caller.func.as_ref())
            .is_some_and(|function| function.common.fn_flags & ZEND_ACC_STRICT_TYPES != 0)
    }
}

/// Whether `arg`, an object, is an instance of the class `name`.
fn instance_of(arg: &Zval, name: &[u8]) -> bool {
    let Some(object) = arg.object() else {
        return false;
    };
    // The API's own classes are final, so their instances are exactly that class: compare the
    // name before looking anything up.
    // SAFETY: a live object's class entry, and that entry's name, live for the request.
    let class = unsafe { object.ce.as_ref() }
        .and_then(|class| unsafe { class.name.cast::<ZendStr>().as_ref() });
    if class.is_some_and(|class| class.as_bytes().eq_ignore_ascii_case(name)) {
        return true;
    }
    std::str::from_utf8(name)
        .ok()
        .and_then(ClassEntry::try_find)
        .is_some_and(|class| object.instance_of(class))
}

/// Checks `arg` against its parameter's declared type as the engine checks a userland
/// function's argument, coercing it in place in weak mode. On refusal the engine's `TypeError`
/// (or whatever the coercion threw, a deprecation promoted by an error handler, say) is pending,
/// and `false` is returned.
pub(crate) fn coerce(arg: &mut Zval) -> bool {
    let Some(at) = locate(arg) else {
        // Not an argument of the running call: nothing to check it against.
        PhpException::new(
            "hyperuuid: argument not found in the call frame".into(),
            0,
            ce::type_error(),
        )
        .throw();
        return false;
    };
    let Some(info) = arg_info(&at) else {
        return false;
    };
    let mask = info.type_.type_mask;
    // SAFETY: a zval's type byte.
    let code = unsafe { arg.u1.type_info } & 0xFF;
    if mask & TYPE_MASK & (1 << code) != 0 {
        return true;
    }
    if let Some(name) = type_class_name(info)
        && instance_of(arg, name)
    {
        return true;
    }
    if ExecutorGlobals::has_exception() {
        return false;
    }
    // SAFETY: `arg` is a live argument zval in the frame, which the engine may replace with its
    // coerced value, as it does for a userland parameter.
    if unsafe { zend_verify_scalar_type_hint(mask, arg, caller_is_strict(&at), false) } {
        return true;
    }
    PhpException::new(
        format!(
            "{}(): Argument #{} (${}) must be of type {}, {} given{}",
            method_name(&at),
            at.position + 1,
            parameter_name(&at, info),
            type_text(info),
            value_name(arg),
            called_in(&at),
        ),
        0,
        ce::type_error(),
    )
    .throw();
    false
}

/// An `int` parameter.
pub struct Int(pub i64);

impl FromZvalMut<'_> for Int {
    const TYPE: DataType = DataType::Long;

    fn from_zval_mut(zval: &mut Zval) -> Option<Self> {
        if !coerce(zval) {
            return None;
        }
        zval.long().map(Int)
    }
}

/// A `string` parameter, its bytes borrowed from the call frame (where a coerced value lives
/// until the call returns).
pub struct Str<'a>(pub &'a [u8]);

impl<'a> FromZvalMut<'a> for Str<'a> {
    const TYPE: DataType = DataType::String;

    fn from_zval_mut(zval: &'a mut Zval) -> Option<Self> {
        if !coerce(zval) {
            return None;
        }
        let zval: &'a Zval = zval;
        zval.zend_str().map(|s| Str(s.as_bytes()))
    }
}

/// A `bool` parameter with a default (`timestamp()`'s `$throwOnMismatch`).
///
/// Declared nullable to ext-php-rs only so that its generated handler hands an explicit `null`
/// to this conversion (which refuses it, as 1.0 did) instead of refusing it in its own words;
/// `registration.rs` declares the parameter non-nullable to the engine, and the stub generator
/// says so too.
pub struct Flag(pub bool);

impl FromZvalMut<'_> for Flag {
    const TYPE: DataType = DataType::Bool;
    const NULLABLE: bool = true;

    fn from_zval_mut(zval: &mut Zval) -> Option<Self> {
        if !coerce(zval) {
            return None;
        }
        zval.bool().map(Flag)
    }

    /// Never used: every `Flag` parameter declares its default.
    fn from_missing() -> Option<Self> {
        Some(Flag(true))
    }
}

impl From<bool> for Flag {
    fn from(value: bool) -> Self {
        Flag(value)
    }
}

impl ext_php_rs::convert::StubLiteral for Flag {
    fn stub_literal(&self) -> String {
        if self.0 { "true" } else { "false" }.to_owned()
    }
}

/// A `UuidLayout` parameter with a default; nullable to ext-php-rs only, as `Flag` is.
pub struct LayoutArg(pub UuidLayout);

impl FromZvalMut<'_> for LayoutArg {
    const TYPE: DataType = <UuidLayout as ext_php_rs::convert::FromZval>::TYPE;
    const NULLABLE: bool = true;

    fn from_zval_mut(zval: &mut Zval) -> Option<Self> {
        if !coerce(zval) {
            return None;
        }
        <UuidLayout as ext_php_rs::convert::FromZval>::from_zval(zval).map(LayoutArg)
    }

    /// Never used: every `LayoutArg` parameter declares its default.
    fn from_missing() -> Option<Self> {
        Some(LayoutArg(UuidLayout::Rfc9562))
    }
}

impl From<UuidLayout> for LayoutArg {
    fn from(value: UuidLayout) -> Self {
        LayoutArg(value)
    }
}

impl ext_php_rs::convert::StubLiteral for LayoutArg {
    fn stub_literal(&self) -> String {
        ext_php_rs::convert::StubLiteral::stub_literal(&self.0)
    }
}

/// A `Uuid` parameter.
pub struct UuidArg<'a>(pub &'a Uuid);

impl<'a> FromZvalMut<'a> for UuidArg<'a> {
    const TYPE: DataType = <&Uuid as ext_php_rs::convert::FromZval>::TYPE;

    fn from_zval_mut(zval: &'a mut Zval) -> Option<Self> {
        if !coerce(zval) {
            return None;
        }
        let zval: &'a Zval = zval;
        <&Uuid as ext_php_rs::convert::FromZval>::from_zval(zval).map(UuidArg)
    }
}

/// A `DateTimeInterface|int|null $unixMillis` parameter, defaulting to `null`: registered as
/// `mixed` (ext-php-rs's argument info holds one type) and declared as the union by
/// `registration.rs`, so the check here runs against the union. `None` is `null`.
pub struct Millis<'a>(pub Option<&'a Zval>);

impl<'a> FromZvalMut<'a> for Millis<'a> {
    const TYPE: DataType = DataType::Mixed;
    const NULLABLE: bool = true;

    fn from_zval_mut(zval: &'a mut Zval) -> Option<Self> {
        if !coerce(zval) {
            return None;
        }
        let zval: &'a Zval = zval;
        Some(Millis((!zval.is_null()).then_some(zval)))
    }

    fn from_missing() -> Option<Self> {
        Some(Millis(None))
    }
}
