//! `HyperUuid\Uuid`: a 16-byte value with the 1.0 PHP class's API, defaults and messages.
//!
//! The bytes are a private PHP property named `bytes`, as the 1.0 class's
//! `private readonly string $bytes` was, so PHP's generic object machinery sees what it saw
//! then: `==` and `<=>` compare the property loosely (ext-php-rs's compare handler copies
//! every PHP-visible Rust property into its declared slot and runs the standard comparison),
//! and `var_dump`, `print_r` and `(array)` show it under the same mangled name. Reading,
//! writing or unsetting it from outside the class is refused for its visibility, and the
//! property's type takes no value back, so nothing writes it once the object is built.
//! `\Stringable` is not declared: the engine adds it for `__toString`, and a second
//! declaration fails the class at startup.
//!
//! `serialize` writes, and `unserialize` reads, the 1.0 class's payload byte for byte
//! (`O:14:"HyperUuid\Uuid":1:{s:21:"\0HyperUuid\Uuid\0bytes";s:16:"...";}`): `__serialize`
//! returns the property under its mangled name, and `__unserialize` builds the value from it.
//! ext-php-rs marks the class not serializable when it registers it; the bit is cleared
//! after every module's startup and before any request ([`allow_serialization`]).

// `timestamp`'s `throwOnMismatch` is PHP's parameter name (named arguments use it), and the
// macro also emits it outside the method, where an item-level allow does not reach.
#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::mem::ManuallyDrop;
use std::mem::offset_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::Mutex;

use ext_php_rs::args::Arg;
use ext_php_rs::binary::Binary;
use ext_php_rs::boxed::ZBox;
use ext_php_rs::builders::{ClassBuilder, FunctionBuilder};
use ext_php_rs::class::RegisteredClass;
use ext_php_rs::convert::{FromZval, IntoZval};
use ext_php_rs::error::Result as ZvalResult;
use ext_php_rs::exception::{PhpException, PhpResult};
use ext_php_rs::flags::{ClassFlags, DataType, MethodFlags, PropertyFlags};
use ext_php_rs::prelude::*;
use ext_php_rs::types::{ZendCallable, ZendClassObject, ZendHashTable, ZendObject, Zval};
use ext_php_rs::zend::{ClassEntry, ExecuteData, ExecutorGlobals, ce, run_handler};
use ext_php_rs::zend_fastcall;

use super::args::{Flag, Int, LayoutArg, Str, UuidArg};
use super::enums::{UuidLayout, UuidVariant};
use super::errors::{invalid_argument_exception, runtime_exception};
use super::{guard, value_name};
use crate::{Layout, Variant, v7};

// `\JsonSerializable`'s class entry, a data symbol ext/json exports (always built in since
// PHP 8.0); read like SPL's in errors.rs, and linked the same way on Windows.
#[cfg_attr(windows, link(name = "php8"))]
unsafe extern "C" {
    static php_json_serializable_ce: *mut ext_php_rs::ffi::zend_class_entry;
}

fn json_serializable() -> &'static ClassEntry {
    // SAFETY: ext/json writes the pointer once, in its MINIT, before shared extensions start.
    // This runs while the class registers, at MINIT and outside `guard`; a panic there is
    // caught by ext-php-rs's `startup_guard`, which logs "module startup panicked: ..." and
    // fails MINIT, so the extension doesn't load rather than unwinding into the engine.
    unsafe { php_json_serializable_ce.as_ref() }.expect("ext/json's JsonSerializable is registered")
}

/// The `bytes` property's mangled name, `\0Class\0property` as PHP spells a private
/// property: the key in `(array)` casts and in the 1.0 serialized payload.
const BYTES_KEY: &str = "\0HyperUuid\\Uuid\0bytes";

/// What the 1.0 class's readonly property said to a second write; said here to a second
/// `__construct` or `__unserialize` on a live value.
const READONLY_MESSAGE: &str = "Cannot modify readonly property HyperUuid\\Uuid::$bytes";

/// `Runtime::SQL_ORDER_WRONG_VERSION` in the 1.0 binding: the core's own text.
const SQL_ORDER_WRONG_VERSION: &str =
    "the value is not a UUID of the version this conversion takes";

/// The value of the `bytes` property: a binary zend string on the way out, and nothing on
/// the way in. A write from inside the class's scope (a closure bound to it) fails the
/// conversion, so the property stays as the constructor set it.
#[derive(Clone, Copy)]
pub(crate) struct Bytes([u8; 16]);

impl IntoZval for Bytes {
    const TYPE: DataType = DataType::String;
    const NULLABLE: bool = false;

    fn set_zval(self, zv: &mut Zval, _: bool) -> ZvalResult<()> {
        zv.set_binary(self.0.to_vec());
        Ok(())
    }
}

impl FromZval<'_> for Bytes {
    const TYPE: DataType = DataType::String;

    fn from_zval(_: &Zval) -> Option<Self> {
        None
    }
}

/// A parsed 16-byte RFC 9562 UUID value, held exactly as given: RFC 9562 order, or SQL
/// Server order for a value from `toSqlOrder()`, which the methods taking a `UuidLayout`
/// read in place. Casts to, and JSON-encodes as, the hyphenated hex string.
#[php_class]
#[php(name = "HyperUuid\\Uuid")]
#[php(flags = ClassFlags::Final)]
#[php(implements(ce = json_serializable, stub = "\\JsonSerializable"))]
#[php(modifier = add_unserialize)]
#[derive(Clone)]
pub struct Uuid {
    #[php(prop, flags = PropertyFlags::Private)]
    bytes: Bytes,
}

impl Uuid {
    /// Wraps 16 bytes, for the generation and serialization doors.
    pub(crate) fn from_bytes(b: [u8; 16]) -> Uuid {
        Uuid { bytes: Bytes(b) }
    }

    /// The 16 bytes, as held.
    pub(crate) fn as_array(&self) -> [u8; 16] {
        self.bytes.0
    }

    fn core(&self) -> crate::Uuid {
        crate::Uuid::from_bytes(self.bytes.0)
    }

    fn hyphenated(&self) -> String {
        self.core().to_string()
    }
}

fn layout(layout: &UuidLayout) -> Layout {
    match layout {
        UuidLayout::Rfc9562 => Layout::Rfc9562,
        UuidLayout::SqlServer => Layout::SqlServer,
    }
}

fn invalid_argument(message: impl Into<String>) -> PhpException {
    PhpException::new(message.into(), 0, invalid_argument_exception())
}

/// An `\InvalidArgumentException` whose message is `prefix` then `raw`, byte for byte: `raw`
/// is the caller's string, which need not be UTF-8 and may hold NUL bytes.
fn invalid_argument_bytes(prefix: &str, raw: &[u8]) -> PhpException {
    let mut message = prefix.as_bytes().to_vec();
    message.extend_from_slice(raw);
    let mut text = Zval::new();
    text.set_binary(message);
    let object = ZendObject::new(invalid_argument_exception());
    if object.try_call_method("__construct", vec![&text]).is_err() {
        return invalid_argument(format!("{prefix}{}", String::from_utf8_lossy(raw)));
    }
    match object.into_zval(false) {
        Ok(zv) => invalid_argument(prefix).with_object(zv),
        Err(_) => invalid_argument(format!("{prefix}{}", String::from_utf8_lossy(raw))),
    }
}

/// A `Uuid` object already in a zval, returned as a `HyperUuid\Uuid`.
pub struct UuidObject(Zval);

impl IntoZval for UuidObject {
    const TYPE: DataType = DataType::object(<Uuid as RegisteredClass>::CLASS_NAME);
    const NULLABLE: bool = false;

    fn set_zval(self, zv: &mut Zval, _: bool) -> ZvalResult<()> {
        *zv = self.0;
        Ok(())
    }
}

/// A `\DateTimeImmutable` built by PHP, returned as one.
pub struct DateTimeImmutable(Zval);

impl IntoZval for DateTimeImmutable {
    const TYPE: DataType = DataType::object("DateTimeImmutable");
    const NULLABLE: bool = false;

    fn set_zval(self, zv: &mut Zval, _: bool) -> ZvalResult<()> {
        *zv = self.0;
        Ok(())
    }
}

thread_local! {
    /// `nil()`, `max()` and the four `Namespaces` (slots 2 to 5) for the current request, so
    /// each returns one instance per request, as the 1.0 classes' function-static caches did.
    /// Released at request shutdown ([`release_constants`]) and only there: the entries are
    /// `ManuallyDrop`, so the thread-local itself has no destructor, and nothing releases a
    /// zval at thread or process exit, after the engine's heap is gone.
    static CONSTANTS: RefCell<[Option<ManuallyDrop<Zval>>; 6]> =
        const { RefCell::new([None, None, None, None, None, None]) };

    /// Whether a request is running: set at the end of this extension's RINIT and cleared at
    /// the start of its RSHUTDOWN, and again in its post-RSHUTDOWN, which runs even when the
    /// RSHUTDOWNs don't. Outside it (another extension's RINIT ahead of ours, or the request
    /// shutdown of one after ours, such as ext/session's save handler), nothing is cached,
    /// since nothing would release the cache before the engine frees its heap.
    static IN_REQUEST: Cell<bool> = const { Cell::new(false) };
}

// The cache must stay a type with no destructor: one would run at thread exit, releasing zvals
// into a heap the engine has already destroyed.
const _: () = assert!(!std::mem::needs_drop::<[Option<ManuallyDrop<Zval>>; 6]>());

/// Whether the `$this` of the running `__construct` call already holds a `Uuid`. The
/// constructor handler runs inside the `__construct` frame, so the executor's current frame's
/// `This` is the object being constructed: uninitialised for `new Uuid(...)`, initialised
/// for an explicit call on a live value.
fn this_is_constructed() -> bool {
    let ex = ExecutorGlobals::get().current_execute_data;
    // SAFETY: while a PHP-called function runs, the current frame is a live `zend_execute_data`
    // whose `This` is a valid zval for the duration of the call.
    let Some(ex) = (unsafe { ex.as_ref() }) else {
        return false;
    };
    ex.This
        .object()
        .is_some_and(|obj| ZendClassObject::<Uuid>::from_zend_obj(obj).is_some())
}

fn readonly() -> PhpException {
    PhpException::new(READONLY_MESSAGE.into(), 0, ce::error())
}

/// A `Uuid` of exactly 16 bytes, refused as the constructor refuses anything else.
fn sixteen(bytes: &[u8]) -> PhpResult<Uuid> {
    match <[u8; 16]>::try_from(bytes) {
        Ok(b) => Ok(Uuid::from_bytes(b)),
        Err(_) => Err(invalid_argument("bytes must be exactly 16 bytes")),
    }
}

// The engine's hook for work after every MINIT and before any request (`zend.h`), which
// bindgen doesn't carry. Read and written only during startup, on the one startup thread.
#[cfg_attr(windows, link(name = "php8"))]
unsafe extern "C" {
    static mut zend_post_startup_cb: Option<unsafe extern "C" fn() -> i32>;
}

/// The post-startup callback that was installed before ours, called after ours.
static PREVIOUS_POST_STARTUP: Mutex<Option<unsafe extern "C" fn() -> i32>> = Mutex::new(None);

/// `MODULE_PERSISTENT` (`zend_modules.h`): loaded at startup, as opposed to by `dl()`.
const MODULE_PERSISTENT: i32 = 1;

/// Arranges for `Uuid` to accept `unserialize` before any request runs. Called from MINIT.
///
/// ext-php-rs sets `NotSerializable` when it registers a class backed by a Rust object
/// (`ClassBuilder::register`), because such a class has no generic way to rebuild its Rust
/// half; `Uuid` has one, `__unserialize`. The bit can't be cleared in MINIT: `#[php_module]`
/// runs a module's own startup hooks before it registers any class, and no hook follows
/// registration. Nor can it wait for this extension's RINIT: another extension's RINIT may
/// unserialize first (ext/session decodes the session there under `session.auto_start`), and
/// a refusal there fails the request before this extension's RINIT runs, every request.
///
/// So a persistent load chains `zend_post_startup_cb`, which `zend_post_startup` calls once,
/// after every module's MINIT and before opcache's preload and the first request. It clears
/// the pointer before calling it, so a later startup in the same process starts unchained.
/// A `dl()` load comes after that point and gets no post-startup call; its RINIT, which
/// `dl()` runs straight after MINIT, clears the bit instead ([`allow_serialization_late`]).
pub(crate) fn allow_serialization(module_type: i32) {
    if module_type != MODULE_PERSISTENT {
        return;
    }
    // SAFETY: MINIT runs on the startup thread before any request, the only time the engine
    // or another extension touches this pointer.
    let previous = unsafe { ptr::replace(&raw mut zend_post_startup_cb, Some(post_startup)) };
    if let Ok(mut slot) = PREVIOUS_POST_STARTUP.lock() {
        *slot = previous;
    }
}

/// Clears the bit and finishes the rest of the registration (`registration::finish`), then
/// runs whatever post-startup callback was installed before this one.
unsafe extern "C" fn post_startup() -> i32 {
    // A panic must not unwind into the engine; failing to finish leaves the class refusing
    // `unserialize`, or a surface short of 1.0's, which the tests catch.
    let _ = catch_unwind(super::registration::finish);
    let previous = PREVIOUS_POST_STARTUP
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());
    match previous {
        // SAFETY: the callback the engine held before ours, called as the engine would have.
        Some(previous) => unsafe { previous() },
        None => 0,
    }
}

/// RINIT's fallback for a `dl()` load, which gets no post-startup call: clears the bit and
/// finishes the registration. It writes only while the bit is still set, so after a startup
/// load it only reads the class entry.
pub(crate) fn allow_serialization_late() {
    let _ = catch_unwind(|| {
        if Uuid::get_metadata()
            .ce()
            .flags()
            .contains(ClassFlags::NotSerializable)
        {
            super::registration::finish();
        }
    });
}

/// Clears `NotSerializable` on the class entry ext-php-rs registered for `Uuid`, found in the
/// compiler's class table: the executor's (`EG(class_table)`, which `zend_lookup_class_ex`
/// reads) isn't set up yet at post-startup.
pub(crate) fn clear_not_serializable() {
    if let Some(found) = super::registration::own_entry::<Uuid>() {
        // SAFETY: `found` is the live, registered class entry. This runs at post-startup,
        // before any request, or for a `dl()` load in that request's own RINIT; the extension
        // is NTS only, so no other thread reads it. ZTS: revisit, since the `dl()` path would
        // then write a class entry other threads share.
        unsafe { (*found).ce_flags &= !ClassFlags::NotSerializable.bits() };
    }
}

/// Adds `__unserialize(array $data): void` by hand: a method that `#[php_impl]` generates
/// refuses an object whose Rust half isn't built yet ("Failed to retrieve reference to
/// `$this`"), and that is exactly the object `unserialize` hands it.
fn add_unserialize(builder: ClassBuilder) -> ClassBuilder {
    builder.method(
        FunctionBuilder::new("__unserialize", unserialize_handler)
            .arg(Arg::new("data", DataType::Array))
            .returns(DataType::Void, false, false),
        MethodFlags::Public,
    )
}

zend_fastcall! {
    extern "C" fn unserialize_handler(ex: &mut ExecuteData, _retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| {
            if let Err(e) = guard(|| unserialize(ex)) {
                e.throw();
            }
        }));
    }
}

/// `Uuid::__unserialize`: builds the Rust half of the object `unserialize` allocated, from
/// the array `__serialize` returned (or 1.0's default serialization wrote, the same shape).
/// Every check runs before anything is stored, so a refused payload leaves the object as
/// unbuilt as it found it, and every method on it refuses.
fn unserialize(ex: &mut ExecuteData) -> PhpResult<()> {
    let mut data = Arg::new("data", DataType::Array);
    let (parser, this) = ex.parser_object();
    let this = this.map(ptr::from_mut);
    if parser.arg(&mut data).parse().is_err() {
        // The engine's ArgumentCountError is already thrown.
        return Ok(());
    }
    let Some(value) = data.zval() else {
        return Ok(());
    };
    let value = value.dereference();
    let Some(array) = value.array() else {
        return Err(PhpException::new(
            format!(
                "HyperUuid\\Uuid::__unserialize(): Argument #1 ($data) must be of type array, {} given",
                value_name(value)
            ),
            0,
            ce::type_error(),
        ));
    };
    // SAFETY: `this` is the running call's `$this`, live for the call.
    let target = this.and_then(|this| unsafe { class_object(this) });
    let Some(target) = target else {
        return Err(PhpException::new(
            "Failed to retrieve reference to `$this`".into(),
            0,
            ce::error(),
        ));
    };
    // A live value is never rewritten: the 1.0 class's readonly property refused a second
    // write, and `Uuid::nil()` is shared across the request.
    if target.obj.is_some() {
        return Err(readonly());
    }
    let uuid = from_data(array)?;
    target.initialize(uuid);
    Ok(())
}

/// The `ZendClassObject<Uuid>` around `object`, built or not, or `None` for an object this
/// class's `create_object` didn't allocate. ext-php-rs keeps its own unbuilt accessor
/// crate-private, so this repeats its checks: the handlers this class installs, then the
/// class, then the container at `std`'s offset in the `#[repr(C)]` struct.
///
/// # Safety
///
/// `object` must point at a live `zend_object` that nothing else borrows for the returned
/// lifetime.
unsafe fn class_object<'a>(object: *mut ZendObject) -> Option<&'a mut ZendClassObject<Uuid>> {
    let meta = Uuid::get_metadata();
    // SAFETY: the caller's contract.
    let std = unsafe { &*object };
    if !ptr::eq(std.handlers, meta.handlers()) || !std.instance_of(meta.ce()) {
        return None;
    }
    // SAFETY: an object carrying this class's handlers was allocated by its `create_object`
    // as a `ZendClassObject<Uuid>`, whose `std` field `object` points at.
    unsafe {
        object
            .byte_sub(offset_of!(ZendClassObject<Uuid>, std))
            .cast::<ZendClassObject<Uuid>>()
            .as_mut()
    }
}

/// A `Uuid` from serialized data: exactly one entry, the mangled `bytes` key, holding a
/// 16-byte string. A value of another type is refused with the engine's own message for the
/// 1.0 class's typed property; any other shape, which 1.0's serialize never wrote, as
/// invalid data.
fn from_data(data: &ZendHashTable) -> PhpResult<Uuid> {
    let invalid = || invalid_argument("invalid serialization data for HyperUuid\\Uuid");
    let value = data.get(BYTES_KEY).ok_or_else(invalid)?.dereference();
    if data.len() != 1 {
        return Err(invalid());
    }
    let Some(bytes) = value.zend_str() else {
        return Err(PhpException::new(
            format!(
                "Cannot assign {} to property HyperUuid\\Uuid::$bytes of type string",
                value_name(value)
            ),
            0,
            ce::type_error(),
        ));
    };
    sixteen(bytes.as_bytes())
}

/// The request's shared instance of `bytes`, cached in `CONSTANTS[slot]`; outside a request
/// (see `IN_REQUEST`), a fresh instance each call.
pub(crate) fn constant(slot: usize, bytes: [u8; 16]) -> PhpResult<UuidObject> {
    if !IN_REQUEST.get() {
        return Ok(UuidObject(Uuid::from_bytes(bytes).into_zval(false)?));
    }
    CONSTANTS.with_borrow_mut(|cache| {
        if cache[slot].is_none() {
            cache[slot] = Some(ManuallyDrop::new(Uuid::from_bytes(bytes).into_zval(false)?));
        }
        match &cache[slot] {
            Some(zv) => Ok(UuidObject(zv.shallow_clone())),
            None => Err(PhpException::new(
                "hyperuuid: constant cache unavailable".into(),
                0,
                runtime_exception(),
            )),
        }
    })
}

/// Ends the request for the cache, then releases the request's cached constants; called first
/// thing in the module's request shutdown, while the engine can still free them.
pub(crate) fn release_constants() {
    IN_REQUEST.set(false);
    let released = CONSTANTS.with_borrow_mut(std::mem::take);
    for zv in released.into_iter().flatten() {
        drop(ManuallyDrop::into_inner(zv));
    }
}

/// Ends the request for the cache after the executor has shut down; called from the module's
/// post-RSHUTDOWN, which `php_request_shutdown` runs for every request, in its own `zend_try`,
/// after `zend_deactivate()` has freed the request's objects.
///
/// The RSHUTDOWNs themselves are skipped when a request never finished starting: if a module's
/// RINIT after this one bails out (a fatal error), `php_request_startup` catches it and leaves
/// `PG(modules_activated)` unset, and `php_request_shutdown` then runs no RSHUTDOWN at all, so
/// [`release_constants`] never runs. Without this, `IN_REQUEST` would stay set, with cached zvals
/// pointing at objects the engine has freed, until this module's next RINIT; a module whose
/// RINIT runs before ours in the next request (ext/session decoding a session under
/// `session.auto_start`, whose class's `__wakeup` calls `Uuid::nil()`, say) would be handed a
/// freed object. SAPIs that keep serving after a failed startup (the CLI server, Apache's
/// handler) reach that path.
pub(crate) fn end_request() {
    IN_REQUEST.set(false);
    forget_stale_constants();
}

/// Discards, without releasing, any cached constant left from an earlier request; called from
/// [`end_request`], and from the module's request startup for a request that never reached
/// post-RSHUTDOWN. The engine has freed that request's object store by now, so the leftover
/// zvals point at freed memory: dropping them would release it again, and handing them out
/// would be a use after free. Forgetting them (the entries are `ManuallyDrop`, so overwriting
/// them releases nothing) leaks nothing the engine still owns.
pub(crate) fn forget_stale_constants() {
    CONSTANTS.with_borrow_mut(|cache| cache.fill_with(|| None));
}

/// Starts the request for the cache; called last in the module's request startup.
pub(crate) fn begin_request() {
    IN_REQUEST.set(true);
}

/// `\DateTimeImmutable::createFromFormat('U.v', ..., new \DateTimeZone('UTC'))`, the 1.0
/// class's construction. (Its PHP 8.4+ branch, `createFromTimestamp` then `setMicrosecond`,
/// built an identical object.)
fn utc_instant(millis: u64) -> PhpResult<DateTimeImmutable> {
    let failed = || {
        PhpException::new(
            "hyperuuid: failed to construct timestamp from UUID".into(),
            0,
            runtime_exception(),
        )
    };
    let text = format!("{}.{:03}", millis / 1000, millis % 1000);
    let zone = ZendCallable::try_from_name("timezone_open")
        .and_then(|open| open.try_call(vec![&"UTC"]))
        .map_err(|_| failed())?;
    let instant = ZendCallable::try_from_name("DateTimeImmutable::createFromFormat")
        .and_then(|create| create.try_call(vec![&"U.v", &text, &zone]))
        .map_err(|_| failed())?;
    if instant.is_object() {
        Ok(DateTimeImmutable(instant))
    } else {
        Err(failed())
    }
}

#[php_impl]
impl Uuid {
    /// Wraps a raw 16-byte RFC 9562 (big-endian) UUID value.
    ///
    /// @throws \InvalidArgumentException If `$bytes` isn't exactly 16 bytes.
    pub fn __construct(bytes: Str<'_>) -> PhpResult<Self> {
        guard(|| {
            // ext-php-rs's constructor handler stores what this returns over whatever the
            // object already holds, so an explicit `$u->__construct(...)` on a live value
            // would rewrite it (a shared `Uuid::nil()` included). The 1.0 class refused that
            // as a readonly write; refuse it the same way before building anything.
            if this_is_constructed() {
                return Err(readonly());
            }
            sixteen(bytes.0)
        })
    }

    /// Parses an 8-4-4-4-12 hyphenated hex UUID string, in either letter case. Exactly that
    /// shape and nothing else: no braces, no `urn:uuid:` prefix, no unhyphenated or otherwise
    /// hyphenated hex.
    ///
    /// @throws \InvalidArgumentException If `$string` isn't a valid UUID string.
    pub fn parse(string: Str<'_>) -> PhpResult<Self> {
        guard(|| {
            match std::str::from_utf8(string.0)
                .ok()
                .and_then(|s| s.parse::<crate::Uuid>().ok())
            {
                Some(id) => Ok(Uuid::from_bytes(id.into_bytes())),
                None => Err(invalid_argument_bytes("invalid UUID string: ", string.0)),
            }
        })
    }

    /// The UUID's 16 raw bytes, as held.
    pub fn bytes(&self) -> PhpResult<Binary<u8>> {
        guard(|| Ok(Binary::new(self.bytes.0.to_vec())))
    }

    /// The version of this UUID, read in `$layout`'s byte order: the version nibble, 0
    /// through 15, in RFC 9562 order; in SQL Server order 7 for a value converted from
    /// version 7, or 0 for anything else.
    #[php(defaults(layout = UuidLayout::Rfc9562))]
    pub fn version(&self, layout: LayoutArg) -> PhpResult<i64> {
        guard(|| Ok(i64::from(self.core().version_in(self::layout(&layout.0)))))
    }

    /// The variant field of this RFC 9562-ordered UUID (RFC 9562 §4.1).
    pub fn variant(&self) -> PhpResult<UuidVariant> {
        guard(|| {
            Ok(match self.core().variant() {
                Variant::Ncs => UuidVariant::Ncs,
                Variant::Rfc9562 => UuidVariant::Rfc9562,
                Variant::Microsoft => UuidVariant::Microsoft,
                Variant::Future => UuidVariant::Future,
            })
        })
    }

    /// Whether this is an RFC 9562 UUID of version `$version`, held in `$layout`'s byte
    /// order. A `$version` outside 0-15 is simply never matched.
    #[php(defaults(layout = UuidLayout::Rfc9562))]
    pub fn is_rfc(&self, version: Int, layout: LayoutArg) -> PhpResult<bool> {
        guard(|| {
            Ok(u8::try_from(version.0)
                .is_ok_and(|version| self.core().is_rfc_in(version, self::layout(&layout.0))))
        })
    }

    /// The 8-4-4-4-12 hyphenated hex string representation.
    #[php(name = "__toString")]
    pub fn to_php_string(&self) -> PhpResult<String> {
        guard(|| Ok(self.hyphenated()))
    }

    /// The JSON representation: the same hyphenated hex string as `__toString()`.
    pub fn json_serialize(&self) -> PhpResult<String> {
        guard(|| Ok(self.hyphenated()))
    }

    /// Whether `$other` wraps the same 16 raw bytes.
    pub fn equals(&self, other: UuidArg<'_>) -> PhpResult<bool> {
        guard(|| Ok(self.bytes.0 == other.0.bytes.0))
    }

    /// The Unix-epoch millisecond timestamp embedded in an RFC 9562 version 7 UUID held in
    /// `$layout`'s byte order, or null for anything else.
    #[php(defaults(layout = UuidLayout::Rfc9562))]
    pub fn unix_millis(&self, layout: LayoutArg) -> PhpResult<Option<i64>> {
        guard(|| {
            // A 48-bit field: always within i64.
            Ok(self
                .millis(self::layout(&layout.0))
                .and_then(|ms| i64::try_from(ms).ok()))
        })
    }

    /// The UTC timestamp embedded in an RFC 9562 version 7 UUID held in `$layout`'s byte
    /// order. Throws for anything else, or returns null when `$throwOnMismatch` is false.
    ///
    /// @throws \InvalidArgumentException If `$throwOnMismatch` is true and this isn't an RFC
    ///     9562 version 7 UUID in that layout.
    #[php(defaults(throwOnMismatch = true, layout = UuidLayout::Rfc9562))]
    pub fn timestamp(
        &self,
        throwOnMismatch: Flag,
        layout: LayoutArg,
    ) -> PhpResult<Option<DateTimeImmutable>> {
        guard(|| {
            let core_layout = self::layout(&layout.0);
            let Some(millis) = self.millis(core_layout) else {
                if !throwOnMismatch.0 {
                    return Ok(None);
                }
                let version = self.core().version_in(core_layout);
                let variant = if version == 7 {
                    " with a non-RFC 9562 variant"
                } else {
                    ""
                };
                let place = match layout.0 {
                    UuidLayout::Rfc9562 => "",
                    UuidLayout::SqlServer => " in SqlServer order",
                };
                return Err(invalid_argument(format!(
                    "timestamp() is only defined for RFC 9562 version 7 UUIDs, got version {version}{variant}{place}"
                )));
            };
            utc_instant(millis).map(Some)
        })
    }

    /// Converts an RFC 9562-ordered version 7 UUID to SQL Server order, the byte order a
    /// `uniqueidentifier` sorts by creation order.
    ///
    /// @throws \InvalidArgumentException If this isn't an RFC 9562 version 7 UUID.
    pub fn to_sql_order(&self) -> PhpResult<Self> {
        guard(|| {
            v7::to_sql_order(&self.core())
                .map(|id| Uuid::from_bytes(id.into_bytes()))
                .map_err(|_| invalid_argument(SQL_ORDER_WRONG_VERSION))
        })
    }

    /// Inverse of `toSqlOrder()`: converts a SQL-ordered UUID back to the RFC 9562 version
    /// 7 it came from.
    ///
    /// @throws \InvalidArgumentException If these bytes aren't a SQL-ordered value converted
    ///     from version 7.
    #[php(name = "fromSqlOrder")]
    pub fn to_rfc_order(&self) -> PhpResult<Self> {
        guard(|| {
            v7::to_rfc_order(&self.core())
                .map(|id| Uuid::from_bytes(id.into_bytes()))
                .map_err(|_| invalid_argument(SQL_ORDER_WRONG_VERSION))
        })
    }

    /// The serialized form: the 16 bytes under the property's mangled name, the array PHP
    /// serialized for the 1.0 class.
    #[php(name = "__serialize")]
    pub fn serialize(&self) -> PhpResult<ZBox<ZendHashTable>> {
        guard(|| {
            let mut bytes = Zval::new();
            bytes.set_binary(self.as_array().to_vec());
            let mut data = ZendHashTable::new();
            data.insert(BYTES_KEY, bytes)?;
            Ok(data)
        })
    }

    /// The RFC 9562 §5.9 Nil UUID: all 128 bits zero.
    pub fn nil() -> PhpResult<UuidObject> {
        guard(|| constant(0, [0x00; 16]))
    }

    /// The RFC 9562 §5.10 Max UUID: all 128 bits one.
    pub fn max() -> PhpResult<UuidObject> {
        guard(|| constant(1, [0xFF; 16]))
    }
}

impl Uuid {
    fn millis(&self, layout: Layout) -> Option<u64> {
        crate::timestamp_in(&self.core(), layout).map(|(_, millis)| millis)
    }
}
