//! The JNI shim the Java binding loads on JDK 21 and later. It sits beside the core in one
//! process: the core's C exports are linked in (so they remain exported), and `JNI_OnLoad`
//! registers one table of natives on `NativeCore`. No `alloc`, no `std`, and no panic path:
//! every native either matches or returns the core's own result code.
//!
//! The natives are `pub unsafe extern "system" fn`: as `pub` fns taking raw pointers they
//! would otherwise fail clippy's `not_unsafe_ptr_arg_deref`. Their safety contract is
//! the JNI one (a live `JNIEnv`, and arrays of the declared type), hence the crate-wide allow.
#![cfg_attr(not(test), no_std)]
#![allow(clippy::missing_safety_doc)]

extern crate hyperuuid;

use core::ffi::{c_char, c_void};
use core::ptr::{null, null_mut};
use jni_sys::{
    JNI_ABORT, JNI_ERR, JNI_OK, JNI_VERSION_1_8, JNIEnv, JNINativeMethod, JavaVM, jbyte,
    jbyteArray, jclass, jint, jlong, jlongArray,
};

// The core's C exports, reached by symbol the way every binding reaches them, plus the two
// C runtime calls the array natives need (there is no `libc` dependency).
unsafe extern "C" {
    fn uuid_version(uuid_ptr: *const u8, layout_code: u32) -> u32;
    fn uuid_variant(uuid_ptr: *const u8) -> u32;
    fn uuid_get_timestamp(uuid_ptr: *const u8, layout_code: u32, millis_out: *mut u64) -> u32;
    fn uuid_is_rfc(uuid_ptr: *const u8, version: u32, layout_code: u32) -> u32;
    fn uuid_new_v4(out_ptr: *mut u8) -> i32;
    fn uuid_new_v5(ns_ptr: *const u8, name_ptr: *const u8, name_len: u32, out_ptr: *mut u8) -> i32;
    fn uuid_new_v7(unix_millis: u64, out_ptr: *mut u8) -> i32;
    fn uuid_v7_unix_millis(uuid_ptr: *const u8) -> u64;
    fn uuid_v7_unix_millis_in(uuid_ptr: *const u8, layout_code: u32) -> u64;
    fn uuid_new_v7_batch(unix_millis: u64, count: u32, out_ptr: *mut u8) -> i32;
    fn uuid_v7_to_sql_order(uuid_ptr: *mut u8) -> i32;
    fn uuid_v7_to_rfc_order(uuid_ptr: *mut u8) -> i32;
    fn hyperuuid_version() -> u32;
    fn malloc(size: usize) -> *mut c_void;
    fn free(p: *mut c_void);
}

/// The largest region a critical array access may cover (bytes). Above it, data is copied.
pub const CRITICAL_MAX: usize = 64 * 1024;

/// The largest batch the core accepts (2^26, its `v7::MAX_BATCH`; a test pins the two equal).
/// Above it the core answers 4, and the shim answers 4 itself rather than allocate for a
/// count it will refuse. The core checks the timestamp first, so for an out-of-range
/// timestamp it answers 2 where the shim answers 4 for an oversized count, or 3 for an
/// output array shorter than `count * 16`.
const MAX_BATCH: u64 = 1 << 26;

/// Bytes per `SetByteArrayRegion` when a malloc'd batch is copied out.
const COPY_CHUNK: usize = 1024 * 1024;

const CLASS: &[u8] = b"io/github/skunkwerkx/hyperuuid/NativeCore\0";

/// One registered native: NUL-terminated name and descriptor, and the function.
pub struct Native {
    name: &'static [u8],
    descriptor: &'static [u8],
    function: *const c_void,
}

macro_rules! native {
    ($name:literal, $descriptor:literal, $function:path) => {
        Native {
            name: concat!($name, "\0").as_bytes(),
            descriptor: concat!($descriptor, "\0").as_bytes(),
            function: $function as *const c_void,
        }
    };
}

const NATIVES: [Native; 14] = [
    native!("uuidNewV4", "([J)I", native_uuid_new_v4),
    native!("uuidNewV5", "(JJ[B[J)I", native_uuid_new_v5),
    native!("uuidNewV7", "(J[J)I", native_uuid_new_v7),
    native!("uuidV7UnixMillis", "(JJ)J", native_uuid_v7_unix_millis),
    native!(
        "uuidV7UnixMillisIn",
        "(JJI)J",
        native_uuid_v7_unix_millis_in
    ),
    native!("uuidNewV7Batch", "(JI[B)I", native_uuid_new_v7_batch),
    native!("uuidV7ToSqlOrder", "(JJ[J)I", native_uuid_v7_to_sql_order),
    native!("uuidV7ToRfcOrder", "(JJ[J)I", native_uuid_v7_to_rfc_order),
    native!("uuidVersion", "(JJI)I", native_uuid_version),
    native!("uuidVariant", "(JJ)I", native_uuid_variant),
    native!("uuidIsRfc", "(JJII)I", native_uuid_is_rfc),
    native!("uuidGetTimestamp", "(JJI[J)I", native_uuid_get_timestamp),
    native!("hyperuuidVersion", "()I", native_hyperuuid_version),
    native!("jniNatives", "()I", native_jni_natives),
];

/// True when no two entries share a name. A `const fn` so a repeat fails the build.
const fn names_unique(natives: &[Native]) -> bool {
    let mut i = 0;
    while i < natives.len() {
        let mut j = i + 1;
        while j < natives.len() {
            if bytes_eq(natives[i].name, natives[j].name) {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

const fn bytes_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut k = 0;
    while k < a.len() {
        if a[k] != b[k] {
            return false;
        }
        k += 1;
    }
    true
}

const _: () = assert!(names_unique(&NATIVES));

/// The one allocation the shim makes. Production calls `malloc` directly; tests swap it.
#[cfg(not(test))]
unsafe fn alloc(n: usize) -> *mut u8 {
    // SAFETY: plain C `malloc`; the caller checks for null.
    unsafe { malloc(n) }.cast()
}
#[cfg(test)]
unsafe fn alloc(n: usize) -> *mut u8 {
    tests::alloc(n)
}

#[cfg(not(test))]
unsafe fn release(p: *mut u8) {
    // SAFETY: `p` came from `alloc` and is freed once.
    unsafe { free(p.cast()) }
}
#[cfg(test)]
unsafe fn release(p: *mut u8) {
    tests::release(p)
}

/// A null array access (a critical get or `malloc`) is never a core result. With an exception
/// already pending, return at once; otherwise throw `OutOfMemoryError`. The `-1` is never
/// mapped by Java: the pending exception wins.
unsafe fn fail_null(env: *mut JNIEnv) -> jint {
    // SAFETY: the caller passes a live env.
    unsafe {
        if ((**env).v1_2.ExceptionCheck)(env) {
            return -1;
        }
        let class = ((**env).v1_2.FindClass)(env, c"java/lang/OutOfMemoryError".as_ptr());
        if !class.is_null() {
            ((**env).v1_2.ThrowNew)(env, class, c"native memory unavailable".as_ptr());
        }
    }
    -1
}

/// A UUID as the 16 big-endian bytes the core reads.
fn bytes_of(msb: jlong, lsb: jlong) -> [u8; 16] {
    let mut b = [0u8; 16];
    b[..8].copy_from_slice(&msb.to_be_bytes());
    b[8..].copy_from_slice(&lsb.to_be_bytes());
    b
}

/// Writes a UUID to `out[0..2]` as (msb, lsb).
unsafe fn put_uuid(env: *mut JNIEnv, out: jlongArray, bytes: &[u8; 16]) {
    let mut msb = [0u8; 8];
    let mut lsb = [0u8; 8];
    msb.copy_from_slice(&bytes[..8]);
    lsb.copy_from_slice(&bytes[8..]);
    let pair = [i64::from_be_bytes(msb), i64::from_be_bytes(lsb)];
    // SAFETY: the caller passes a live env and a `long[]` of at least two elements.
    unsafe { ((**env).v1_2.SetLongArrayRegion)(env, out, 0, 2, pair.as_ptr()) };
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_uuid_new_v4(
    env: *mut JNIEnv,
    _class: jclass,
    out: jlongArray,
) -> jint {
    let mut b = [0u8; 16];
    // SAFETY: `b` is 16 live bytes.
    let rc = unsafe { uuid_new_v4(b.as_mut_ptr()) };
    if rc == 0 {
        // SAFETY: the caller passes a live env and a `long[]` of at least two elements.
        unsafe { put_uuid(env, out, &b) };
    }
    rc
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_uuid_new_v5(
    env: *mut JNIEnv,
    _class: jclass,
    ns_msb: jlong,
    ns_lsb: jlong,
    name: jbyteArray,
    out: jlongArray,
) -> jint {
    let ns = bytes_of(ns_msb, ns_lsb);
    let mut b = [0u8; 16];
    // SAFETY: the caller passes a live env and a `byte[]`.
    let len = unsafe { ((**env).v1_2.GetArrayLength)(env, name) };
    let rc = if len <= 0 {
        // SAFETY: a zero length never dereferences the name pointer.
        unsafe { uuid_new_v5(ns.as_ptr(), null(), 0, b.as_mut_ptr()) }
    } else if len as usize <= CRITICAL_MAX {
        // SAFETY: only the core call runs inside the critical region, which closes before
        // any other JNI call.
        unsafe {
            let p = ((**env).v1_2.GetPrimitiveArrayCritical)(env, name, null_mut());
            if p.is_null() {
                return fail_null(env);
            }
            let rc = uuid_new_v5(
                ns.as_ptr(),
                p.cast_const().cast(),
                len as u32,
                b.as_mut_ptr(),
            );
            ((**env).v1_2.ReleasePrimitiveArrayCritical)(env, name, p, JNI_ABORT);
            rc
        }
    } else {
        // SAFETY: `p` is `len` bytes, filled by `GetByteArrayRegion`, and freed on every path.
        unsafe {
            let p = alloc(len as usize);
            if p.is_null() {
                return fail_null(env);
            }
            ((**env).v1_2.GetByteArrayRegion)(env, name, 0, len, p.cast::<jbyte>());
            if ((**env).v1_2.ExceptionCheck)(env) {
                release(p);
                return -1;
            }
            let rc = uuid_new_v5(ns.as_ptr(), p, len as u32, b.as_mut_ptr());
            release(p);
            rc
        }
    };
    if rc == 0 {
        // SAFETY: the caller passes a live env and a `long[]` of at least two elements.
        unsafe { put_uuid(env, out, &b) };
    }
    rc
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_uuid_new_v7(
    env: *mut JNIEnv,
    _class: jclass,
    unix_millis: jlong,
    out: jlongArray,
) -> jint {
    let mut b = [0u8; 16];
    // SAFETY: `b` is 16 live bytes.
    let rc = unsafe { uuid_new_v7(unix_millis as u64, b.as_mut_ptr()) };
    if rc == 0 {
        // SAFETY: the caller passes a live env and a `long[]` of at least two elements.
        unsafe { put_uuid(env, out, &b) };
    }
    rc
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_uuid_v7_unix_millis(
    _env: *mut JNIEnv,
    _class: jclass,
    msb: jlong,
    lsb: jlong,
) -> jlong {
    let b = bytes_of(msb, lsb);
    // SAFETY: `b` is 16 live bytes.
    unsafe { uuid_v7_unix_millis(b.as_ptr()) as jlong }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_uuid_v7_unix_millis_in(
    _env: *mut JNIEnv,
    _class: jclass,
    msb: jlong,
    lsb: jlong,
    layout: jint,
) -> jlong {
    let b = bytes_of(msb, lsb);
    // SAFETY: `b` is 16 live bytes.
    unsafe { uuid_v7_unix_millis_in(b.as_ptr(), layout as u32) as jlong }
}

/// Fills `out` (a `byte[]` of `count * 16`) with a v7 batch. Up to `CRITICAL_MAX` bytes the
/// array is pinned for the one core call; above it the core writes into `malloc`'d memory
/// that is copied out in 1 MiB chunks. Neither path ever pins and mallocs in one call.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_uuid_new_v7_batch(
    env: *mut JNIEnv,
    _class: jclass,
    unix_millis: jlong,
    count: jint,
    out: jbyteArray,
) -> jint {
    if count == 0 {
        // SAFETY: a zero count never dereferences the output pointer.
        return unsafe { uuid_new_v7_batch(unix_millis as u64, 0, null_mut()) };
    }
    if count < 0 || count as u64 > MAX_BATCH {
        return 4;
    }
    let total = count as usize * 16;
    // SAFETY: the caller passes a live env and a `byte[]`. A short array is the core's
    // buffer-too-small code (3), never a write past the end.
    if (unsafe { ((**env).v1_2.GetArrayLength)(env, out) } as usize) < total {
        return 3;
    }
    if total <= CRITICAL_MAX {
        // SAFETY: only the core call runs inside the critical region; the array holds
        // `count * 16` bytes, checked above. Mode 0 copies the result back.
        unsafe {
            let p = ((**env).v1_2.GetPrimitiveArrayCritical)(env, out, null_mut());
            if p.is_null() {
                return fail_null(env);
            }
            let rc = uuid_new_v7_batch(unix_millis as u64, count as u32, p.cast());
            ((**env).v1_2.ReleasePrimitiveArrayCritical)(env, out, p, 0);
            rc
        }
    } else {
        // SAFETY: `p` is `total` live bytes, freed on every path.
        unsafe {
            let p = alloc(total);
            if p.is_null() {
                return fail_null(env);
            }
            let rc = uuid_new_v7_batch(unix_millis as u64, count as u32, p);
            if rc == 0 {
                let mut at = 0;
                while at < total {
                    let n = if total - at < COPY_CHUNK {
                        total - at
                    } else {
                        COPY_CHUNK
                    };
                    ((**env).v1_2.SetByteArrayRegion)(
                        env,
                        out,
                        at as jint,
                        n as jint,
                        p.add(at).cast::<jbyte>(),
                    );
                    if ((**env).v1_2.ExceptionCheck)(env) {
                        release(p);
                        return -1;
                    }
                    at += n;
                }
            }
            release(p);
            rc
        }
    }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_uuid_v7_to_sql_order(
    env: *mut JNIEnv,
    _class: jclass,
    msb: jlong,
    lsb: jlong,
    out: jlongArray,
) -> jint {
    let mut b = bytes_of(msb, lsb);
    // SAFETY: `b` is 16 live bytes.
    let rc = unsafe { uuid_v7_to_sql_order(b.as_mut_ptr()) };
    if rc == 0 {
        // SAFETY: the caller passes a live env and a `long[]` of at least two elements.
        unsafe { put_uuid(env, out, &b) };
    }
    rc
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_uuid_v7_to_rfc_order(
    env: *mut JNIEnv,
    _class: jclass,
    msb: jlong,
    lsb: jlong,
    out: jlongArray,
) -> jint {
    let mut b = bytes_of(msb, lsb);
    // SAFETY: `b` is 16 live bytes.
    let rc = unsafe { uuid_v7_to_rfc_order(b.as_mut_ptr()) };
    if rc == 0 {
        // SAFETY: the caller passes a live env and a `long[]` of at least two elements.
        unsafe { put_uuid(env, out, &b) };
    }
    rc
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_uuid_version(
    _env: *mut JNIEnv,
    _class: jclass,
    msb: jlong,
    lsb: jlong,
    layout: jint,
) -> jint {
    let b = bytes_of(msb, lsb);
    // SAFETY: `b` is 16 live bytes.
    unsafe { uuid_version(b.as_ptr(), layout as u32) as jint }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_uuid_variant(
    _env: *mut JNIEnv,
    _class: jclass,
    msb: jlong,
    lsb: jlong,
) -> jint {
    let b = bytes_of(msb, lsb);
    // SAFETY: `b` is 16 live bytes.
    unsafe { uuid_variant(b.as_ptr()) as jint }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_uuid_is_rfc(
    _env: *mut JNIEnv,
    _class: jclass,
    msb: jlong,
    lsb: jlong,
    version: jint,
    layout: jint,
) -> jint {
    let b = bytes_of(msb, lsb);
    // SAFETY: `b` is 16 live bytes.
    unsafe { uuid_is_rfc(b.as_ptr(), version as u32, layout as u32) as jint }
}

/// Returns the version, and writes the milliseconds to `out[0]` only when it is not 0.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_uuid_get_timestamp(
    env: *mut JNIEnv,
    _class: jclass,
    msb: jlong,
    lsb: jlong,
    layout: jint,
    out: jlongArray,
) -> jint {
    let b = bytes_of(msb, lsb);
    let mut millis = 0u64;
    // SAFETY: `b` is 16 live bytes and `millis` a live u64.
    let version = unsafe { uuid_get_timestamp(b.as_ptr(), layout as u32, &mut millis) };
    if version != 0 {
        let m = millis as jlong;
        // SAFETY: the caller passes a live env and a `long[]` of at least one element.
        unsafe { ((**env).v1_2.SetLongArrayRegion)(env, out, 0, 1, &m) };
    }
    version as jint
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_hyperuuid_version(_env: *mut JNIEnv, _class: jclass) -> jint {
    // SAFETY: takes no arguments.
    unsafe { hyperuuid_version() as jint }
}

/// The length of the registration table, never a literal.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_jni_natives(_env: *mut JNIEnv, _class: jclass) -> jint {
    NATIVES.len() as jint
}

/// Registers every native on `NativeCore`, eagerly, so a table that does not match the Java
/// declarations fails `System.load` (with the pending exception) instead of a later call.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn JNI_OnLoad(vm: *mut JavaVM, _reserved: *mut c_void) -> jint {
    let mut penv: *mut c_void = null_mut();
    // SAFETY: the JVM passes a live `JavaVM`.
    let got = unsafe { ((**vm).v1_2.GetEnv)(vm, &mut penv, JNI_VERSION_1_8) };
    // The null check is on `env`, the pointer dereferenced below, not on `penv`: CodeQL's
    // rust/access-invalid-pointer query takes `is_null()` as a barrier for its receiver only,
    // and a guard on `penv` leaves the cast's result unguarded in its eyes (alert 28).
    let env = penv.cast::<*const jni_sys::JNINativeInterface_>();
    if got != JNI_OK || env.is_null() {
        return JNI_ERR;
    }
    // SAFETY: `env` came from `GetEnv` above and is not null; the class name is NUL-terminated.
    let class = unsafe { ((**env).v1_2.FindClass)(env, CLASS.as_ptr().cast::<c_char>()) };
    if class.is_null() {
        return JNI_ERR;
    }
    let empty = JNINativeMethod {
        name: null_mut(),
        signature: null_mut(),
        fnPtr: null_mut(),
    };
    let mut methods = [empty; NATIVES.len()];
    let mut i = 0;
    while i < NATIVES.len() {
        methods[i] = JNINativeMethod {
            name: NATIVES[i].name.as_ptr().cast_mut().cast::<c_char>(),
            signature: NATIVES[i].descriptor.as_ptr().cast_mut().cast::<c_char>(),
            fnPtr: NATIVES[i].function.cast_mut(),
        };
        i += 1;
    }
    // SAFETY: `methods` holds `NATIVES.len()` entries of NUL-terminated strings.
    let rc = unsafe {
        ((**env).v1_2.RegisterNatives)(env, class, methods.as_ptr(), NATIVES.len() as jint)
    };
    if rc != JNI_OK {
        return JNI_ERR;
    }
    JNI_VERSION_1_8
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::RefCell;
    use core::ffi::c_void;
    use core::mem::MaybeUninit;
    use core::ptr::{addr_of_mut, null, null_mut};
    use jni_sys::{
        JNIEnv, JNINativeInterface_, jarray, jboolean, jbyte, jbyteArray, jint, jlong, jlongArray,
        jsize,
    };

    /// What the stub env records and how it behaves; one per test thread.
    #[derive(Default)]
    struct State {
        crit_gets: usize,
        crit_releases: usize,
        throws: usize,
        exception_checks: usize,
        mallocs: usize,
        frees: usize,
        crit_null: bool,
        pending: bool,
        malloc_null: bool,
        array_len: jsize,
        order: Vec<&'static str>,
        found_class: String,
        thrown_class: usize,
        byte_chunks: Vec<(usize, usize)>,
        release_mode: Option<jint>,
        pending_after_set_bytes: bool,
    }

    thread_local! {
        static STATE: RefCell<State> = RefCell::new(State::default());
    }

    fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
        STATE.with(|s| f(&mut s.borrow_mut()))
    }

    /// The test stand-ins for `alloc` and `release`: counted, and switchable to fail.
    pub(super) fn alloc(n: usize) -> *mut u8 {
        if with(|s| {
            if s.malloc_null {
                return true;
            }
            s.mallocs += 1;
            false
        }) {
            return null_mut();
        }
        unsafe { malloc(n) }.cast()
    }
    pub(super) fn release(p: *mut u8) {
        with(|s| s.frees += 1);
        unsafe { free(p.cast()) };
    }

    const CLASS_TOKEN: usize = 0x1000;

    unsafe extern "system" fn exception_check(_env: *mut JNIEnv) -> bool {
        with(|s| {
            s.exception_checks += 1;
            s.pending
        })
    }
    unsafe extern "system" fn find_class(_env: *mut JNIEnv, name: *const c_char) -> jclass {
        with(|s| {
            s.found_class = unsafe { core::ffi::CStr::from_ptr(name) }
                .to_str()
                .unwrap()
                .to_owned();
        });
        CLASS_TOKEN as jclass
    }
    unsafe extern "system" fn throw_new(
        _env: *mut JNIEnv,
        class: jclass,
        _msg: *const c_char,
    ) -> jint {
        with(|s| {
            s.throws += 1;
            s.thrown_class = class as usize;
        });
        0
    }
    unsafe extern "system" fn get_array_length(_env: *mut JNIEnv, _a: jarray) -> jsize {
        with(|s| s.array_len)
    }
    unsafe extern "system" fn get_critical(
        _env: *mut JNIEnv,
        array: jarray,
        _is_copy: *mut jboolean,
    ) -> *mut c_void {
        with(|s| {
            s.order.push("GetPrimitiveArrayCritical");
            if s.crit_null {
                return null_mut();
            }
            s.crit_gets += 1;
            array.cast()
        })
    }
    unsafe extern "system" fn release_critical(
        _env: *mut JNIEnv,
        _array: jarray,
        _p: *mut c_void,
        mode: jint,
    ) {
        with(|s| {
            s.release_mode = Some(mode);
            s.order.push("ReleasePrimitiveArrayCritical");
            s.crit_releases += 1;
        });
    }
    unsafe extern "system" fn get_byte_region(
        _env: *mut JNIEnv,
        array: jbyteArray,
        start: jsize,
        len: jsize,
        buf: *mut jbyte,
    ) {
        with(|s| s.order.push("GetByteArrayRegion"));
        unsafe {
            core::ptr::copy_nonoverlapping(
                array.cast::<jbyte>().add(start as usize),
                buf,
                len as usize,
            )
        };
    }
    unsafe extern "system" fn set_byte_region(
        _env: *mut JNIEnv,
        array: jbyteArray,
        start: jsize,
        len: jsize,
        buf: *const jbyte,
    ) {
        with(|s| {
            s.order.push("SetByteArrayRegion");
            s.byte_chunks.push((start as usize, len as usize));
            if s.pending_after_set_bytes {
                s.pending = true;
            }
        });
        unsafe {
            core::ptr::copy_nonoverlapping(
                buf,
                array.cast::<jbyte>().add(start as usize),
                len as usize,
            )
        };
    }
    unsafe extern "system" fn set_long_array_region(
        _env: *mut JNIEnv,
        array: jlongArray,
        start: jsize,
        len: jsize,
        buf: *const jlong,
    ) {
        with(|s| s.order.push("SetLongArrayRegion"));
        let dst = array.cast::<i64>();
        for i in 0..len as usize {
            unsafe { dst.add(start as usize + i).write(buf.add(i).read()) };
        }
    }

    /// A `JNIEnv` stub: only the entries a path calls are written, through their addresses,
    /// and the interface is never read as a whole. Arrays are plain pointers the test passes.
    struct StubEnv {
        _iface: Box<MaybeUninit<JNINativeInterface_>>,
        table: Box<*const JNINativeInterface_>,
    }

    impl StubEnv {
        fn new() -> Self {
            STATE.with(|s| *s.borrow_mut() = State::default());
            let mut iface = Box::new(MaybeUninit::<JNINativeInterface_>::uninit());
            let p = iface.as_mut_ptr();
            // SAFETY: each field is written through its own address.
            unsafe {
                addr_of_mut!((*p).v1_2.ExceptionCheck).write(exception_check);
                addr_of_mut!((*p).v1_2.FindClass).write(find_class);
                addr_of_mut!((*p).v1_2.ThrowNew).write(throw_new);
                addr_of_mut!((*p).v1_2.GetArrayLength).write(get_array_length);
                addr_of_mut!((*p).v1_2.GetPrimitiveArrayCritical).write(get_critical);
                addr_of_mut!((*p).v1_2.ReleasePrimitiveArrayCritical).write(release_critical);
                addr_of_mut!((*p).v1_2.GetByteArrayRegion).write(get_byte_region);
                addr_of_mut!((*p).v1_2.SetByteArrayRegion).write(set_byte_region);
                addr_of_mut!((*p).v1_2.SetLongArrayRegion).write(set_long_array_region);
            }
            let table = Box::new(p.cast_const());
            StubEnv {
                _iface: iface,
                table,
            }
        }
        fn env(&mut self) -> *mut JNIEnv {
            &mut *self.table as *mut *const JNINativeInterface_
        }
    }

    fn camel(snake: &str) -> String {
        let mut out = String::new();
        let mut up = false;
        for c in snake.chars() {
            if c == '_' {
                up = true;
            } else if up {
                out.push(c.to_ascii_uppercase());
                up = false;
            } else {
                out.push(c);
            }
        }
        out
    }

    fn name_of(n: &Native) -> &str {
        core::str::from_utf8(&n.name[..n.name.len() - 1]).unwrap()
    }
    fn desc_of(n: &Native) -> &str {
        core::str::from_utf8(&n.descriptor[..n.descriptor.len() - 1]).unwrap()
    }

    const EXPORTS: [&str; 13] = [
        "uuid_new_v4",
        "uuid_new_v5",
        "uuid_new_v7",
        "uuid_v7_unix_millis",
        "uuid_v7_unix_millis_in",
        "uuid_new_v7_batch",
        "uuid_v7_to_sql_order",
        "uuid_v7_to_rfc_order",
        "uuid_version",
        "uuid_variant",
        "uuid_is_rfc",
        "uuid_get_timestamp",
        "hyperuuid_version",
    ];

    #[test]
    fn table_has_fourteen_entries_and_jni_natives_reports_it() {
        assert_eq!(NATIVES.len(), 14);
        let n = unsafe { native_jni_natives(null_mut(), null_mut()) };
        assert_eq!(n, 14);
    }

    #[test]
    fn table_names_are_camel_of_every_export_plus_jni_natives() {
        let mut want: Vec<String> = EXPORTS.iter().map(|e| camel(e)).collect();
        want.push("jniNatives".into());
        want.sort();
        let mut got: Vec<String> = NATIVES.iter().map(|n| name_of(n).to_owned()).collect();
        got.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn table_descriptors_are_pinned() {
        let want = [
            ("uuidNewV4", "([J)I"),
            ("uuidNewV5", "(JJ[B[J)I"),
            ("uuidNewV7", "(J[J)I"),
            ("uuidV7UnixMillis", "(JJ)J"),
            ("uuidV7UnixMillisIn", "(JJI)J"),
            ("uuidNewV7Batch", "(JI[B)I"),
            ("uuidV7ToSqlOrder", "(JJ[J)I"),
            ("uuidV7ToRfcOrder", "(JJ[J)I"),
            ("uuidVersion", "(JJI)I"),
            ("uuidVariant", "(JJ)I"),
            ("uuidIsRfc", "(JJII)I"),
            ("uuidGetTimestamp", "(JJI[J)I"),
            ("hyperuuidVersion", "()I"),
            ("jniNatives", "()I"),
        ];
        assert_eq!(want.len(), NATIVES.len());
        for (name, desc) in want {
            let n = NATIVES.iter().find(|n| name_of(n) == name).expect(name);
            assert_eq!(desc_of(n), desc, "{name}");
        }
    }

    #[test]
    fn names_unique_rejects_a_repeat() {
        let one = NATIVES[0].name;
        let a = Native {
            name: one,
            descriptor: NATIVES[0].descriptor,
            function: null::<c_void>(),
        };
        let b = Native {
            name: one,
            descriptor: NATIVES[1].descriptor,
            function: null::<c_void>(),
        };
        assert!(!names_unique(&[a, b]));
        assert!(names_unique(&NATIVES));
    }

    #[test]
    fn scalar_natives_round_trip_through_the_stub_env() {
        let mut stub = StubEnv::new();
        let env = stub.env();
        let mut out = [0i64; 2];
        let arr = out.as_mut_ptr() as jlongArray;
        let millis: jlong = 0x0123_4567_89AB;
        unsafe {
            assert_eq!(native_uuid_new_v7(env, null_mut(), millis, arr), 0);
            assert_eq!(
                native_uuid_v7_unix_millis(env, null_mut(), out[0], out[1]),
                millis
            );
            assert_eq!(
                native_uuid_version(env, null_mut(), out[0], out[1], 1),
                7 as jint
            );
        }
    }

    /// Calls the batch native on a buffer of `len` bytes, as Java sizes it.
    fn batch(env: *mut JNIEnv, millis: jlong, count: jint, buf: &mut [u8]) -> jint {
        with(|s| s.array_len = buf.len() as jsize);
        unsafe { native_uuid_new_v7_batch(env, null_mut(), millis, count, buf.as_mut_ptr().cast()) }
    }

    const NS: (jlong, jlong) = (0x6ba7_b810_9dad_11d1, 0x80b4_00c0_4fd4_30c8u64 as jlong);

    #[test]
    fn a_null_critical_with_a_pending_exception_throws_nothing_more() {
        let mut stub = StubEnv::new();
        let env = stub.env();
        with(|s| {
            s.crit_null = true;
            s.pending = true;
            s.array_len = 5;
        });
        let mut name = *b"hello";
        let mut out = [0i64; 2];
        let rc = unsafe {
            native_uuid_new_v5(
                env,
                null_mut(),
                NS.0,
                NS.1,
                name.as_mut_ptr().cast(),
                out.as_mut_ptr().cast(),
            )
        };
        assert_eq!(rc, -1);
        assert_eq!(out, [0, 0]);
        with(|s| {
            assert_eq!(s.throws, 0);
            assert_eq!(s.crit_releases, 0);
        });
    }

    #[test]
    fn a_null_critical_without_an_exception_throws_out_of_memory() {
        let mut stub = StubEnv::new();
        let env = stub.env();
        with(|s| {
            s.crit_null = true;
            s.array_len = 5;
        });
        let mut name = *b"hello";
        let mut out = [0i64; 2];
        let rc = unsafe {
            native_uuid_new_v5(
                env,
                null_mut(),
                NS.0,
                NS.1,
                name.as_mut_ptr().cast(),
                out.as_mut_ptr().cast(),
            )
        };
        assert_eq!(rc, -1);
        assert_eq!(out, [0, 0]);
        with(|s| {
            assert_eq!(s.throws, 1);
            assert_eq!(s.found_class, "java/lang/OutOfMemoryError");
            assert_eq!(s.thrown_class, CLASS_TOKEN);
        });
    }

    #[test]
    fn a_null_malloc_throws_out_of_memory_and_frees_nothing() {
        let mut stub = StubEnv::new();
        let env = stub.env();
        with(|s| s.malloc_null = true);
        let mut buf = vec![0u8; 4097 * 16];
        let rc = batch(env, 1_000, 4097, &mut buf);
        assert_eq!(rc, -1);
        assert!(buf.iter().all(|b| *b == 0));
        with(|s| {
            assert_eq!(s.throws, 1);
            assert_eq!(s.frees, 0);
        });
    }

    #[test]
    fn a_batch_at_the_bound_pins_and_above_it_mallocs() {
        let mut stub = StubEnv::new();
        let env = stub.env();
        let mut buf = vec![0u8; 4097 * 16];
        let rc = batch(env, 1_000, 4096, &mut buf);
        assert_eq!(rc, 0);
        with(|s| {
            assert_eq!(
                (s.crit_gets, s.crit_releases, s.mallocs, s.frees),
                (1, 1, 0, 0)
            );
            assert_eq!(s.release_mode, Some(0));
        });
        assert_eq!(unsafe { uuid_version(buf.as_ptr(), 1) }, 7);
        let mut stub = StubEnv::new();
        let env = stub.env();
        let mut buf = vec![0u8; 4097 * 16];
        let rc = batch(env, 1_000, 4097, &mut buf);
        assert_eq!(rc, 0);
        with(|s| {
            assert_eq!((s.crit_gets, s.mallocs, s.frees), (0, 1, 1));
        });
        assert_eq!(unsafe { uuid_version(buf[4096 * 16..].as_ptr(), 1) }, 7);
    }

    #[test]
    fn the_critical_region_closes_before_any_other_call() {
        let mut stub = StubEnv::new();
        let env = stub.env();
        with(|s| s.array_len = 5);
        let mut name = *b"hello";
        let mut out = [0i64; 2];
        let rc = unsafe {
            native_uuid_new_v5(
                env,
                null_mut(),
                NS.0,
                NS.1,
                name.as_mut_ptr().cast(),
                out.as_mut_ptr().cast(),
            )
        };
        assert_eq!(rc, 0);
        with(|s| {
            assert_eq!(
                s.order,
                [
                    "GetPrimitiveArrayCritical",
                    "ReleasePrimitiveArrayCritical",
                    "SetLongArrayRegion"
                ]
            );
            assert_eq!(s.release_mode, Some(jni_sys::JNI_ABORT));
        });
        // The core, called directly, gives the same v5.
        let mut want = [0u8; 16];
        let ns = bytes_of(NS.0, NS.1);
        assert_eq!(
            unsafe { uuid_new_v5(ns.as_ptr(), name.as_ptr(), 5, want.as_mut_ptr()) },
            0
        );
        assert_eq!(
            out,
            [
                i64::from_be_bytes(want[..8].try_into().unwrap()),
                i64::from_be_bytes(want[8..].try_into().unwrap())
            ]
        );
    }

    #[test]
    fn a_name_above_the_bound_is_copied_not_pinned() {
        let mut stub = StubEnv::new();
        let env = stub.env();
        let mut name = vec![7u8; CRITICAL_MAX + 1];
        with(|s| s.array_len = (CRITICAL_MAX + 1) as jsize);
        let mut out = [0i64; 2];
        let rc = unsafe {
            native_uuid_new_v5(
                env,
                null_mut(),
                NS.0,
                NS.1,
                name.as_mut_ptr().cast(),
                out.as_mut_ptr().cast(),
            )
        };
        assert_eq!(rc, 0);
        with(|s| {
            assert_eq!((s.crit_gets, s.mallocs, s.frees), (0, 1, 1));
            assert_eq!(s.order, ["GetByteArrayRegion", "SetLongArrayRegion"]);
        });
        let ns = bytes_of(NS.0, NS.1);
        let mut want = [0u8; 16];
        assert_eq!(
            unsafe {
                uuid_new_v5(
                    ns.as_ptr(),
                    name.as_ptr(),
                    name.len() as u32,
                    want.as_mut_ptr(),
                )
            },
            0
        );
        assert_eq!(out[0], i64::from_be_bytes(want[..8].try_into().unwrap()));
    }

    #[test]
    fn a_large_batch_copies_out_in_one_mebibyte_chunks() {
        let mut stub = StubEnv::new();
        let env = stub.env();
        let count = 65_536 + 1;
        let mut buf = vec![0u8; count * 16];
        let rc = batch(env, 1_000, count as jint, &mut buf);
        assert_eq!(rc, 0);
        with(|s| assert_eq!(s.byte_chunks, [(0, 1_048_576), (1_048_576, 16)]));
        assert_eq!(unsafe { uuid_version(buf[1_048_576..].as_ptr(), 1) }, 7);
    }

    #[test]
    fn a_zero_length_passes_null_and_pins_nothing() {
        let mut stub = StubEnv::new();
        let env = stub.env();
        let rc = unsafe { native_uuid_new_v7_batch(env, null_mut(), 1_000, 0, null_mut()) };
        assert_eq!(rc, 0);
        let mut out = [0i64; 2];
        let rc = unsafe {
            native_uuid_new_v5(
                env,
                null_mut(),
                NS.0,
                NS.1,
                null_mut(),
                out.as_mut_ptr().cast(),
            )
        };
        assert_eq!(rc, 0);
        assert_ne!(out, [0, 0]);
        with(|s| assert_eq!((s.crit_gets, s.mallocs), (0, 0)));
    }

    #[test]
    fn a_batch_count_the_core_refuses_never_allocates() {
        let mut stub = StubEnv::new();
        let env = stub.env();
        for count in [-1, (1 << 26) + 1] {
            let rc = unsafe { native_uuid_new_v7_batch(env, null_mut(), 1_000, count, null_mut()) };
            assert_eq!(rc, 4);
        }
        with(|s| assert_eq!((s.crit_gets, s.mallocs), (0, 0)));
    }

    #[test]
    fn a_v7_round_trips_through_sql_order_and_back() {
        let mut stub = StubEnv::new();
        let env = stub.env();
        let mut v7 = [0i64; 2];
        let mut sql = [0i64; 2];
        let mut back = [0i64; 2];
        unsafe {
            assert_eq!(
                native_uuid_new_v7(env, null_mut(), 0x0123_4567_89AB, v7.as_mut_ptr().cast()),
                0
            );
            assert_eq!(
                native_uuid_v7_to_sql_order(env, null_mut(), v7[0], v7[1], sql.as_mut_ptr().cast()),
                0
            );
            assert_ne!(sql, v7);
            assert_eq!(
                native_uuid_v7_to_rfc_order(
                    env,
                    null_mut(),
                    sql[0],
                    sql[1],
                    back.as_mut_ptr().cast()
                ),
                0
            );
        }
        assert_eq!(back, v7);
    }

    #[test]
    fn a_core_error_on_a_pinned_batch_still_releases() {
        let mut stub = StubEnv::new();
        let env = stub.env();
        let mut buf = vec![0u8; 16];
        assert_eq!(batch(env, -1, 1, &mut buf), 2);
        with(|s| assert_eq!((s.crit_gets, s.crit_releases), (1, 1)));
    }

    #[test]
    fn a_core_error_on_a_malloced_batch_still_frees_and_copies_nothing() {
        let mut stub = StubEnv::new();
        let env = stub.env();
        let mut buf = vec![0u8; 4097 * 16];
        assert_eq!(batch(env, -1, 4097, &mut buf), 2);
        with(|s| {
            assert_eq!((s.mallocs, s.frees), (1, 1));
            assert!(s.byte_chunks.is_empty());
        });
    }

    #[test]
    fn an_exception_after_the_name_copy_frees_and_returns_the_sentinel() {
        let mut stub = StubEnv::new();
        let env = stub.env();
        let mut name = vec![7u8; CRITICAL_MAX + 1];
        with(|s| {
            s.array_len = (CRITICAL_MAX + 1) as jsize;
            s.pending = true;
        });
        let mut out = [0i64; 2];
        let rc = unsafe {
            native_uuid_new_v5(
                env,
                null_mut(),
                NS.0,
                NS.1,
                name.as_mut_ptr().cast(),
                out.as_mut_ptr().cast(),
            )
        };
        assert_eq!(rc, -1);
        assert_eq!(out, [0, 0]);
        with(|s| assert_eq!((s.mallocs, s.frees, s.throws), (1, 1, 0)));
    }

    #[test]
    fn an_exception_after_a_chunk_frees_and_stops_copying() {
        let mut stub = StubEnv::new();
        let env = stub.env();
        with(|s| s.pending_after_set_bytes = true);
        let mut buf = vec![0u8; 65_537 * 16];
        assert_eq!(batch(env, 1_000, 65_537, &mut buf), -1);
        with(|s| {
            assert_eq!(s.byte_chunks, [(0, 1_048_576)]);
            assert_eq!((s.mallocs, s.frees), (1, 1));
        });
    }

    #[test]
    fn a_short_output_array_is_buffer_too_small_on_both_paths() {
        for (count, len) in [(4096, 4096 * 16 - 1), (4097, 4097 * 16 - 1)] {
            let mut stub = StubEnv::new();
            let env = stub.env();
            let mut buf = vec![0u8; len];
            assert_eq!(batch(env, 1_000, count, &mut buf), 3);
            with(|s| assert_eq!((s.crit_gets, s.mallocs, s.byte_chunks.len()), (0, 0, 0)));
        }
    }

    #[test]
    fn a_name_of_exactly_the_bound_pins() {
        let mut stub = StubEnv::new();
        let env = stub.env();
        let mut name = vec![7u8; CRITICAL_MAX];
        with(|s| s.array_len = CRITICAL_MAX as jsize);
        let mut out = [0i64; 2];
        let rc = unsafe {
            native_uuid_new_v5(
                env,
                null_mut(),
                NS.0,
                NS.1,
                name.as_mut_ptr().cast(),
                out.as_mut_ptr().cast(),
            )
        };
        assert_eq!(rc, 0);
        with(|s| assert_eq!((s.crit_gets, s.mallocs), (1, 0)));
    }

    #[test]
    fn the_shims_max_batch_is_the_cores() {
        assert_eq!(MAX_BATCH, u64::from(hyperuuid::v7::MAX_BATCH));
    }
}
