//! The exception classes (all final, all `\RuntimeException`s) and the mapping from the
//! core's v7 errors to them. Messages match 1.0's PHP package's, byte for byte.

use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

use ext_php_rs::args::ArgInfoTables;
use ext_php_rs::builders::ClassBuilder;
use ext_php_rs::exception::PhpException;
use ext_php_rs::flags::ClassFlags;
use ext_php_rs::zend::ClassEntry;

use crate::v7;

// SPL's class entries, read from the PHP binary's exported data symbols. Not `ClassEntry::try_find`:
// it returns `None` while `ExecutorGlobals::class_table()` is null, which it is at MINIT, when the
// `extends` entries are read. A data symbol imported from a DLL needs the import library on
// Windows, as ext-php-rs's windows_build.rs adds to its own bindgen blocks; that name is
// `php8` for the NTS release build this crate ships (debug and ZTS builds use other names).
#[cfg_attr(windows, link(name = "php8"))]
unsafe extern "C" {
    static spl_ce_RuntimeException: *mut ext_php_rs::ffi::zend_class_entry;
    static spl_ce_InvalidArgumentException: *mut ext_php_rs::ffi::zend_class_entry;
}

/// `\RuntimeException`'s class entry.
pub(crate) fn runtime_exception() -> &'static ClassEntry {
    // SAFETY: SPL writes the pointer once, in its own MINIT, before shared extensions start, so
    // this unsynchronised read never races a write.
    unsafe { spl_ce_RuntimeException.as_ref() }.expect("SPL's RuntimeException is registered")
}

/// `\InvalidArgumentException`'s class entry.
pub(crate) fn invalid_argument_exception() -> &'static ClassEntry {
    // SAFETY: as for `runtime_exception`.
    unsafe { spl_ce_InvalidArgumentException.as_ref() }
        .expect("SPL's InvalidArgumentException is registered")
}

/// The binding's own exceptions, each a `final` subclass of `\RuntimeException`.
///
/// They are registered as plain PHP classes ([`register_exceptions`]), not `#[php_class]`
/// structs: a class backed by a Rust object gets ext-php-rs's `create_object` and constructor,
/// which replace the engine's exception initialiser (no file, line or trace) and refuse
/// `new` from PHP. A plain subclass inherits `\RuntimeException`'s `create_object` and
/// `__construct`, exactly as the 1.0 binding's `final class X extends \RuntimeException {}`.
#[derive(Clone, Copy)]
pub(crate) enum Thrown {
    /// `HyperUuid\CounterExhaustedException`: the v7 stream cannot advance.
    CounterExhausted = 0,
    /// `HyperUuid\TimestampOutOfRangeException`: a timestamp before 1970 or after
    /// 9999-12-31T23:59:59.999Z (253402300799999 ms, the generation cap).
    TimestampOutOfRange = 1,
    /// `HyperUuid\RandomSourceException`: the operating system's random source failed.
    RandomSource = 2,
}

/// The registered class entries, indexed by [`Thrown`]; written once, in MINIT.
static EXCEPTION_CLASSES: [AtomicPtr<ClassEntry>; 3] = [
    AtomicPtr::new(ptr::null_mut()),
    AtomicPtr::new(ptr::null_mut()),
    AtomicPtr::new(ptr::null_mut()),
];

const EXCEPTION_NAMES: [&str; 3] = [
    "HyperUuid\\CounterExhaustedException",
    "HyperUuid\\TimestampOutOfRangeException",
    "HyperUuid\\RandomSourceException",
];

macro_rules! keep_entry {
    ($slot:expr) => {{
        fn keep(ce: &'static mut ClassEntry, _: ArgInfoTables) {
            EXCEPTION_CLASSES[$slot].store(ce, Ordering::Release);
        }
        keep as fn(&'static mut ClassEntry, ArgInfoTables)
    }};
}

/// Registers the three exception classes. Called from MINIT, where SPL's classes exist.
pub(crate) fn register_exceptions() -> ext_php_rs::error::Result<()> {
    let keepers = [keep_entry!(0), keep_entry!(1), keep_entry!(2)];
    for (name, keep) in EXCEPTION_NAMES.iter().zip(keepers) {
        ClassBuilder::new(*name)
            .extends((runtime_exception, "\\RuntimeException"))
            .flags(ClassFlags::Final)
            .registration(keep)
            .register()?;
    }
    Ok(())
}

/// An exception of class `class` with `message`, created by the engine when thrown, so it
/// carries the PHP call site and trace.
pub(crate) fn exception(class: Thrown, message: String) -> PhpException {
    let entry = EXCEPTION_CLASSES[class as usize].load(Ordering::Acquire);
    // SAFETY: a class entry the engine registered in MINIT lives for the process.
    match unsafe { entry.cast_const().as_ref() } {
        Some(entry) => PhpException::new(message, 0, entry),
        // Unreachable once MINIT has run; never a different message.
        None => PhpException::new(message, 0, runtime_exception()),
    }
}

const COUNTER_EXHAUSTED: &str = "version 7 counter exhausted: the stream cannot advance without running more than 1000 ms ahead of the supplied timestamp or past its last millisecond";

/// The 1.0 binding's `Runtime::UNADDRESSABLE_BATCH`, its message for the C ABI's code 3.
const UNADDRESSABLE_BATCH: &str = "the batch is too large to address on this platform";

/// Maps a core v7 error to its PHP exception as the 1.0 binding mapped `op`'s return code:
/// 2 and 6 to their own classes, and every other code to `RandomSourceException`, "`op`
/// failed with code N". A single v7 never returns codes 3 and 4; a batch maps them through
/// [`v7_batch_error`].
pub(crate) fn v7_error(e: v7::NewV7Error, op: &str) -> PhpException {
    let code = match e {
        v7::NewV7Error::TimestampOutOfRange => {
            return exception(
                Thrown::TimestampOutOfRange,
                "unix_millis must be between 0 and 253402300799999 (9999-12-31T23:59:59.999Z)"
                    .into(),
            );
        }
        v7::NewV7Error::CounterExhausted => {
            return exception(Thrown::CounterExhausted, COUNTER_EXHAUSTED.into());
        }
        v7::NewV7Error::Random(_) => 1,
        v7::NewV7Error::BufferTooSmall => 3,
        v7::NewV7Error::BatchTooLarge => 4,
    };
    exception(
        Thrown::RandomSource,
        format!("{op} failed with code {code}"),
    )
}

/// Maps a core v7 batch error for a batch of `count`, as the 1.0 binding's
/// `Runtime::newV7Batch` mapped `uuid_new_v7_batch`'s codes 3 and 4 (both
/// `\InvalidArgumentException`); every other error as [`v7_error`]. The batch doors check the
/// count and size the buffer before calling the core, so 3 and 4 are mapped for parity, not
/// expected.
pub(crate) fn v7_batch_error(e: v7::NewV7Error, count: i64) -> PhpException {
    match e {
        v7::NewV7Error::BufferTooSmall => {
            PhpException::new(UNADDRESSABLE_BATCH.into(), 0, invalid_argument_exception())
        }
        v7::NewV7Error::BatchTooLarge => PhpException::new(
            super::generate::v7_batch_limit_message(count),
            0,
            invalid_argument_exception(),
        ),
        other => v7_error(other, "uuid_new_v7_batch"),
    }
}
