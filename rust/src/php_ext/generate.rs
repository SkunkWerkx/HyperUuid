//! `HyperUuid\HyperUuid`, the static generation doors, and `HyperUuid\Namespaces`, the RFC
//! 9562 §6.6 namespace UUIDs. Defaults, conversions and messages are the 1.0 classes'.

// `unixMillis` is PHP's parameter name (named arguments use it), and the macro also emits it
// outside the method, where an item-level allow does not reach.
#![allow(non_snake_case)]

use std::alloc::Layout;
use std::ptr::NonNull;
use std::time::{SystemTime, UNIX_EPOCH};

use ext_php_rs::alloc::{efree, emalloc};
use ext_php_rs::boxed::ZBox;
use ext_php_rs::convert::IntoZval;
use ext_php_rs::error::Result as ZvalResult;
use ext_php_rs::exception::PhpException;
use ext_php_rs::flags::DataType;
use ext_php_rs::prelude::*;
use ext_php_rs::types::{ZendCallable, ZendHashTable, ZendStr, Zval};
use ext_php_rs::zend::{ClassEntry, ExecutorGlobals, ce};

use super::args::{Int, Millis, Str, UuidArg};
use super::errors::{Thrown, exception, invalid_argument_exception, v7_batch_error, v7_error};
use super::uuid::{Uuid, UuidObject, constant};
use super::{guard, value_name};
use crate::{v4, v5, v7};

/// `HyperUuid::MAX_V7_BATCH`: the core's batch ceiling, 2^26.
const MAX_V7_BATCH: i64 = v7::MAX_BATCH as i64;

/// The 1.0 binding's `Runtime::v7BatchLimitMessage`.
pub(crate) fn v7_batch_limit_message(count: i64) -> String {
    format!(
        "count must be between 0 and {MAX_V7_BATCH} (a single version 7 batch takes at most the 26-bit counter space), got {count}"
    )
}

fn invalid_argument(message: impl Into<String>) -> PhpException {
    PhpException::new(message.into(), 0, invalid_argument_exception())
}

/// Stands in for an exception PHP code already threw (a `DateTime` subclass's own method,
/// say): `PhpException::throw` leaves a pending exception alone, so this is never seen.
fn pending() -> PhpException {
    PhpException::new("hyperuuid: exception pending".into(), 0, ce::exception())
}

/// The current time as the 1.0 binding read it: `(int) round(microtime(true) * 1000)`, the
/// wall clock rounded to the nearest millisecond. `microtime(true)` is `gettimeofday`'s
/// seconds plus microseconds over a million, as a double; this builds the same double.
fn now_millis() -> i64 {
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    #[allow(clippy::cast_precision_loss)] // as PHP's own double does
    let seconds = since.as_secs() as f64 + f64::from(since.subsec_micros()) / 1_000_000.0;
    #[allow(clippy::cast_possible_truncation)] // PHP's (int) cast of a value in range
    let millis = (seconds * 1000.0).round() as i64;
    millis
}

// The engine's `*` operator (`zend_operators.h`), declared `ZEND_FASTCALL`: the C convention
// except under MSVC, where it is `__vectorcall`. That changes the symbol too (`mul_function@@24`
// in php8.lib), so Windows declares the real convention and rustc emits the decorated name.
#[cfg(not(windows))]
unsafe extern "C" {
    fn mul_function(
        result: *mut ext_php_rs::ffi::zval,
        op1: *mut ext_php_rs::ffi::zval,
        op2: *mut ext_php_rs::ffi::zval,
    ) -> i32;
}

#[cfg(windows)]
#[link(name = "php8")]
unsafe extern "vectorcall" {
    fn mul_function(
        result: *mut ext_php_rs::ffi::zval,
        op1: *mut ext_php_rs::ffi::zval,
        op2: *mut ext_php_rs::ffi::zval,
    ) -> i32;
}

/// What 1.0's `unixMillisFrom` (declared `: int`, in a `declare(strict_types=1)` file) said
/// when its arithmetic produced a float.
fn float_millis() -> PhpException {
    PhpException::new(
        "HyperUuid\\HyperUuid::unixMillisFrom(): Return value must be of type int, float returned"
            .into(),
        0,
        ce::type_error(),
    )
}

/// The 1.0 binding's `unixMillisFrom`: null is now, an int passes through (the parameter's
/// check has already coerced what weak mode coerces), and a `\DateTimeInterface` is
/// `getTimestamp() * 1000 + intdiv((int) format('u'), 1000)`, in that order, with PHP's own
/// arithmetic: `getTimestamp()` may return anything, and the engine's `*` multiplies it as 1.0's
/// expression did, with the same warnings and `TypeError`s; a float result is the `TypeError`
/// 1.0's int return type raised. Anything else is the `TypeError` PHP raised for the union
/// parameter, naming `method` and the argument `position`.
///
/// One case keeps a documented difference: an int `getTimestamp()` whose milliseconds overflow
/// 64 bits is a `TimestampOutOfRangeException` (1.0's arithmetic overflowed to a float, which its
/// return type refused); no such instant fits 48 bits anyway.
fn unix_millis_from(value: Option<&Zval>, method: &str, position: u32) -> PhpResult<i64> {
    let Some(value) = value.map(Zval::dereference).filter(|v| !v.is_null()) else {
        return Ok(now_millis());
    };
    if let Some(millis) = value.long() {
        return Ok(millis);
    }
    let date_time = ClassEntry::try_find("DateTimeInterface");
    if let (Some(object), Some(date_time)) = (value.object(), date_time)
        && object.instance_of(date_time)
    {
        let unreadable = || {
            PhpException::new(
                "hyperuuid: could not read the DateTimeInterface's timestamp".into(),
                0,
                super::errors::runtime_exception(),
            )
        };
        let out_of_range = || v7_error(v7::NewV7Error::TimestampOutOfRange, "");
        let seconds = object.try_call_method("getTimestamp", vec![]);
        if ExecutorGlobals::has_exception() {
            return Err(pending());
        }
        let mut seconds = seconds.map_err(|_| unreadable())?;
        let int_seconds = seconds.long();
        let product = match int_seconds {
            Some(s) => Some(s.checked_mul(1000).ok_or_else(out_of_range)?),
            None => {
                let mut thousand = Zval::new();
                thousand.set_long(1000);
                let mut product = Zval::new();
                // SAFETY: three live zvals; the engine writes the product into the first.
                unsafe { mul_function(&raw mut product, &raw mut seconds, &raw mut thousand) };
                if ExecutorGlobals::has_exception() {
                    return Err(pending());
                }
                if product.is_double() {
                    None
                } else {
                    Some(product.long().ok_or_else(unreadable)?)
                }
            }
        };
        let micros = object.try_call_method("format", vec![&"u"]);
        if ExecutorGlobals::has_exception() {
            return Err(pending());
        }
        // `(int)` as PHP casts the string format() returns.
        let micros = micros.ok().and_then(|m| {
            ZendCallable::try_from_name("intval")
                .and_then(|intval| intval.try_call(vec![&m]))
                .ok()
        });
        if ExecutorGlobals::has_exception() {
            return Err(pending());
        }
        let micros = micros.and_then(|m| m.long()).ok_or_else(unreadable)?;
        // A float product stays a float with the microseconds added.
        let product = product.ok_or_else(float_millis)?;
        return match product.checked_add(micros / 1000) {
            Some(millis) => Ok(millis),
            None if int_seconds.is_some() => Err(out_of_range()),
            None => Err(float_millis()),
        };
    }
    Err(PhpException::new(
        format!(
            "HyperUuid\\HyperUuid::{method}(): Argument #{position} ($unixMillis) must be of type DateTimeInterface|int|null, {} given",
            value_name(value)
        ),
        0,
        ce::type_error(),
    ))
}

/// A millisecond count as the C ABI's `uint64_t` took it: a negative one wrapped far past the
/// 48-bit field, so it is out of range.
fn unsigned(millis: i64) -> Result<u64, v7::NewV7Error> {
    u64::try_from(millis).map_err(|_| v7::NewV7Error::TimestampOutOfRange)
}

/// A zeroed buffer in the request's heap (`emalloc`), freed when dropped.
///
/// A batch is built here, not in a Rust `Vec`, because the engine's fatal errors (a
/// `memory_limit` exhausted while the result is built, say) end the call with a longjmp that
/// skips every Rust destructor. Memory the request's heap holds is freed with the heap at the
/// end of the request; Rust-heap memory would be lost for the life of the process.
struct RequestBuffer {
    ptr: NonNull<u8>,
    len: usize,
}

impl RequestBuffer {
    /// `len` bytes, more than zero. Asking for more than `memory_limit` allows is the engine's
    /// fatal error, before anything else exists.
    fn zeroed(len: usize) -> PhpResult<Self> {
        let failed = || {
            PhpException::new(
                "hyperuuid: could not allocate the batch".into(),
                0,
                super::errors::runtime_exception(),
            )
        };
        let layout = Layout::from_size_align(len, 1).map_err(|_| failed())?;
        let ptr = NonNull::new(emalloc(layout)).ok_or_else(failed)?;
        // SAFETY: `ptr` is a fresh allocation of `len` bytes.
        unsafe { ptr.as_ptr().write_bytes(0, len) };
        Ok(Self { ptr, len })
    }

    fn as_slice(&self) -> &[u8] {
        // SAFETY: `len` initialised bytes, owned by `self`.
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: `len` initialised bytes, owned by `self` and borrowed uniquely here.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}

impl Drop for RequestBuffer {
    fn drop(&mut self) {
        // SAFETY: allocated by `emalloc` in `zeroed`, and freed only here.
        unsafe { efree(self.ptr.as_ptr()) };
    }
}

/// A binary string the engine allocated, returned as a PHP `string`.
pub struct ZendBytes(ZBox<ZendStr>);

impl IntoZval for ZendBytes {
    const TYPE: DataType = DataType::String;
    const NULLABLE: bool = false;

    fn set_zval(self, zv: &mut Zval, _: bool) -> ZvalResult<()> {
        zv.set_zend_string(self.0);
        Ok(())
    }
}

/// One v7 batch of `count`, the 1.0 binding's `Runtime::newV7Batch`, handed as raw bytes to
/// `finish`, which builds the PHP value: the count is checked before anything is allocated,
/// and a count of 0 is empty without a timestamp check.
///
/// Every allocation from here on is the engine's (see `RequestBuffer`): the bytes, then the
/// value `finish` builds from them, so a fatal error at any point leaves nothing that only
/// Rust could free.
fn v7_batch<R>(
    count: i64,
    millis: i64,
    finish: impl FnOnce(&[u8]) -> PhpResult<R>,
) -> PhpResult<R> {
    if !(0..=MAX_V7_BATCH).contains(&count) {
        return Err(invalid_argument(v7_batch_limit_message(count)));
    }
    if count == 0 {
        return finish(&[]);
    }
    let millis = unsigned(millis).map_err(|e| v7_batch_error(e, count))?;
    // In range: 0 < count <= 2^26.
    let items =
        u32::try_from(count).map_err(|_| invalid_argument(v7_batch_limit_message(count)))?;
    let mut buffer = RequestBuffer::zeroed(items as usize * 16)?;
    v7::new_v7_batch(millis, items, buffer.as_mut_slice()).map_err(|e| v7_batch_error(e, count))?;
    finish(buffer.as_slice())
}

/// Static entry points of the binding.
#[php_class]
#[php(name = "HyperUuid\\HyperUuid")]
#[php(flags = ext_php_rs::flags::ClassFlags::Final)]
#[derive(Default)]
pub struct HyperUuid;

#[php_impl]
impl HyperUuid {
    /// Not instantiable, as the 1.0 class's private constructor made it.
    #[php(vis = "private")]
    pub fn __construct() -> PhpResult<Self> {
        guard(|| Ok(Self))
    }

    /// The most UUIDs one version 7 batch (`newV7Batch()`, `newV7BatchBytes()`) mints:
    /// 67,108,864, which is 2^20 steps' worth: a batch of n consumes ⌈n/64⌉ steps, so the
    /// largest batch takes exactly one millisecond of the stream.
    #[php(name = "MAX_V7_BATCH")] // the default renaming spells it MAX_V_7_BATCH
    const MAX_V7_BATCH: i64 = MAX_V7_BATCH;

    /// The loaded library's own version, "major.minor.patch".
    pub fn native_version() -> PhpResult<String> {
        guard(|| {
            let packed = crate::hyperuuid_version();
            Ok(format!(
                "{}.{}.{}",
                packed >> 16,
                (packed >> 8) & 0xFF,
                packed & 0xFF
            ))
        })
    }

    /// Creates a random UUID version 4 (RFC 9562 §5.4).
    pub fn new_v4() -> PhpResult<Uuid> {
        guard(|| {
            v4::new_v4()
                .map(|id| Uuid::from_bytes(id.into_bytes()))
                .map_err(|_| {
                    exception(
                        Thrown::RandomSource,
                        "uuid_new_v4 failed with code 1".into(),
                    )
                })
        })
    }

    /// Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a name,
    /// taken as the string's raw bytes. The same (namespace, name) pair always produces the
    /// same UUID.
    ///
    /// @throws \InvalidArgumentException If `$name` is 4 GiB or longer.
    pub fn new_v5(namespace: UuidArg<'_>, name: Str<'_>) -> PhpResult<Uuid> {
        guard(|| {
            // The 1.0 binding's C ABI took a uint32_t length; its refusal is kept.
            if u32::try_from(name.0.len()).is_err() {
                return Err(invalid_argument("name must be shorter than 4 GiB"));
            }
            let ns = crate::Uuid::from_bytes(namespace.0.as_array());
            Ok(Uuid::from_bytes(v5::new_v5(ns, name.0).into_bytes()))
        })
    }

    /// Creates a time-sortable UUID version 7 (RFC 9562 §6.2). Defaults to the current time;
    /// pass an explicit DateTimeInterface or Unix-epoch millisecond timestamp (non-negative,
    /// fitting in 48 bits) to embed a specific time instead. The ordering contract is in the
    /// README, "v7 ordering, precisely".
    ///
    /// @throws TimestampOutOfRangeException If the timestamp doesn't fit 48 bits.
    /// @throws CounterExhaustedException If the stream can't advance.
    pub fn new_v7(unixMillis: Millis<'_>) -> PhpResult<Uuid> {
        guard(|| {
            let millis = unix_millis_from(unixMillis.0, "newV7", 1)?;
            let millis = unsigned(millis).map_err(|e| v7_error(e, "uuid_new_v7"))?;
            v7::new_v7(millis)
                .map(|id| Uuid::from_bytes(id.into_bytes()))
                .map_err(|e| v7_error(e, "uuid_new_v7"))
        })
    }

    /// Creates `count` time-sortable version 7 UUIDs sharing one timestamp capture and one
    /// contiguous block of the v7 stream, in strictly increasing order. Defaults to the
    /// current time.
    ///
    /// @throws \InvalidArgumentException If `$count` is negative or above `MAX_V7_BATCH`.
    /// @throws TimestampOutOfRangeException If the timestamp doesn't fit 48 bits.
    /// @throws CounterExhaustedException If the stream can't advance.
    pub fn new_v7_batch(count: Int, unixMillis: Millis<'_>) -> PhpResult<ZBox<ZendHashTable>> {
        guard(|| {
            let millis = unix_millis_from(unixMillis.0, "newV7Batch", 2)?;
            v7_batch(count.0, millis, |bytes| {
                let (ids, _) = bytes.as_chunks::<16>();
                // `ids.len()` is the checked count, at most 2^26.
                let mut array = ZendHashTable::with_capacity(u32::try_from(ids.len()).unwrap_or(0));
                for id in ids {
                    array.push(Uuid::from_bytes(*id))?;
                }
                Ok(array)
            })
        })
    }

    /// Returns `$count` version 7 UUIDs as one binary string of raw RFC 9562-ordered bytes,
    /// 16 per UUID, instead of an array of Uuid objects. Same ordering guarantee and limit as
    /// `newV7Batch()`.
    ///
    /// @throws \InvalidArgumentException If `$count` is negative or above `MAX_V7_BATCH`.
    /// @throws TimestampOutOfRangeException If the timestamp doesn't fit 48 bits.
    /// @throws CounterExhaustedException If the stream can't advance.
    pub fn new_v7_batch_bytes(count: Int, unixMillis: Millis<'_>) -> PhpResult<ZendBytes> {
        guard(|| {
            let millis = unix_millis_from(unixMillis.0, "newV7BatchBytes", 2)?;
            v7_batch(count.0, millis, |bytes| {
                Ok(ZendBytes(ZendStr::new(bytes, false)))
            })
        })
    }
}

/// Well-known namespace UUIDs defined in RFC 9562 Section 6.6. Each accessor returns one
/// instance per request, as the 1.0 class's function-static cache did.
#[php_class]
#[php(name = "HyperUuid\\Namespaces")]
#[php(flags = ext_php_rs::flags::ClassFlags::Final)]
#[derive(Default)]
pub struct Namespaces;

/// `6ba7b81N-9dad-11d1-80b4-00c04fd430c8`, the shape every RFC 9562 namespace shares.
const fn namespace(n: u8) -> [u8; 16] {
    [
        0x6b,
        0xa7,
        0xb8,
        0x10 | n,
        0x9d,
        0xad,
        0x11,
        0xd1,
        0x80,
        0xb4,
        0x00,
        0xc0,
        0x4f,
        0xd4,
        0x30,
        0xc8,
    ]
}

#[php_impl]
impl Namespaces {
    /// Not instantiable, as the 1.0 class's private constructor made it.
    #[php(vis = "private")]
    pub fn __construct() -> PhpResult<Self> {
        guard(|| Ok(Self))
    }

    /// The DNS namespace UUID.
    pub fn dns() -> PhpResult<UuidObject> {
        guard(|| constant(2, namespace(0)))
    }

    /// The URL namespace UUID.
    pub fn url() -> PhpResult<UuidObject> {
        guard(|| constant(3, namespace(1)))
    }

    /// The ISO OID namespace UUID.
    pub fn oid() -> PhpResult<UuidObject> {
        guard(|| constant(4, namespace(2)))
    }

    /// The X.500 DN namespace UUID.
    pub fn x500() -> PhpResult<UuidObject> {
        guard(|| constant(5, namespace(4)))
    }
}
