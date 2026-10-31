use hyperuuid::Uuid;
use hyperuuid::v7::{MAX_BATCH, MAX_UNIX_MILLIS, NewV7Error};
use napi::bindgen_prelude::{Either, Uint8Array};
use napi::{Env, JsDate, JsString, Status};
use napi_derive::napi;

/// The `code` property every HyperUuid error carries to JavaScript.
pub enum ErrorCode {
    RandomSource,
    TimestampRange,
    BatchTooLarge,
    WrongVersion,
    InvalidId,
    InvalidBuffer,
    InvalidCount,
    CounterExhausted,
    /// An engine-level failure (out of memory building a string) keeps napi's own status.
    Napi(Status),
}

impl AsRef<str> for ErrorCode {
    fn as_ref(&self) -> &str {
        match self {
            ErrorCode::RandomSource => "ERR_HYPERUUID_RANDOM_SOURCE",
            ErrorCode::TimestampRange => "ERR_HYPERUUID_TIMESTAMP_RANGE",
            ErrorCode::BatchTooLarge => "ERR_HYPERUUID_BATCH_TOO_LARGE",
            ErrorCode::WrongVersion => "ERR_HYPERUUID_WRONG_VERSION",
            ErrorCode::InvalidId => "ERR_HYPERUUID_INVALID_ID",
            ErrorCode::InvalidBuffer => "ERR_HYPERUUID_INVALID_BUFFER",
            ErrorCode::InvalidCount => "ERR_HYPERUUID_INVALID_COUNT",
            ErrorCode::CounterExhausted => "ERR_HYPERUUID_COUNTER_EXHAUSTED",
            ErrorCode::Napi(status) => status.as_ref(),
        }
    }
}

pub type Result<T> = napi::Result<T, ErrorCode>;

/// The core's version as `major.minor.patch`, unpacked from `hyperuuid_version()`.
#[napi(catch_unwind)]
pub fn native_version() -> String {
    let packed = hyperuuid::hyperuuid_version();
    format!(
        "{}.{}.{}",
        packed >> 16,
        (packed >> 8) & 0xff,
        packed & 0xff
    )
}

fn err(code: ErrorCode, message: &str) -> napi::Error<ErrorCode> {
    napi::Error::new(code, message)
}

const TIMESTAMP_MESSAGE: &str = "unix_millis must be non-negative and fit within 48 bits";
const BATCH_MESSAGE: &str = "batch count exceeds the 26-bit counter space (67108864 per call)";

/// The 48-bit millisecond a `time` argument stands for: now when absent, otherwise a number
/// or `Date` floored. NaN and infinities fail the range test, so they land in `TimestampRange`.
fn millis(time: Option<Either<f64, JsDate>>) -> Result<u64> {
    let value = match time {
        None => {
            return Ok(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_millis() as u64)
                .unwrap_or(0));
        }
        Some(Either::A(number)) => number,
        Some(Either::B(date)) => date
            .value_of()
            .map_err(|_| err(ErrorCode::TimestampRange, TIMESTAMP_MESSAGE))?,
    };
    if !(value >= 0.0 && value <= MAX_UNIX_MILLIS as f64) {
        return Err(err(ErrorCode::TimestampRange, TIMESTAMP_MESSAGE));
    }
    Ok(value.floor() as u64)
}

/// A batch size from JavaScript. Counts cross as `f64` because napi's `u32` is ToUint32
/// (`1.5` becomes 1, `NaN` becomes 0), which would turn a caller's mistake into a quiet result.
fn batch_count(value: f64) -> Result<u32> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 {
        return Err(err(
            ErrorCode::InvalidCount,
            "count must be a non-negative whole number",
        ));
    }
    if value > f64::from(MAX_BATCH) {
        return Err(err(ErrorCode::BatchTooLarge, BATCH_MESSAGE));
    }
    Ok(value as u32)
}

fn v7_error(error: NewV7Error, random_message: &str) -> napi::Error<ErrorCode> {
    match error {
        NewV7Error::TimestampOutOfRange => err(ErrorCode::TimestampRange, TIMESTAMP_MESSAGE),
        NewV7Error::BatchTooLarge => err(ErrorCode::BatchTooLarge, BATCH_MESSAGE),
        NewV7Error::BufferTooSmall => err(
            ErrorCode::InvalidBuffer,
            "buffer length must be a multiple of 16 (one whole UUID per 16 bytes)",
        ),
        NewV7Error::CounterExhausted => err(ErrorCode::CounterExhausted, &error.to_string()),
        _ => err(ErrorCode::RandomSource, random_message),
    }
}

fn format36(uuid: &Uuid) -> [u8; 36] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = [0u8; 36];
    let mut at = 0;
    for (index, byte) in uuid.as_bytes().iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            out[at] = b'-';
            at += 1;
        }
        out[at] = HEX[usize::from(byte >> 4)];
        out[at + 1] = HEX[usize::from(byte & 0xf)];
        at += 2;
    }
    out
}

/// The id as a JavaScript string, built straight from the 36 stack bytes. They are ASCII, so
/// latin1 is exact and V8 skips the UTF-8 decode a `String` return would cost.
fn js_id<'env>(env: &'env Env, uuid: &Uuid) -> Result<JsString<'env>> {
    env.create_string_latin1(format36(uuid))
        .map_err(|e| napi::Error::new(ErrorCode::Napi(e.status), e.reason))
}

fn batch_bytes(count: u32, time: Option<Either<f64, JsDate>>) -> Result<Vec<u8>> {
    let unix_millis = millis(time)?;
    let mut out = vec![0u8; count as usize * 16];
    hyperuuid::v7::new_v7_batch(unix_millis, count, &mut out)
        .map_err(|e| v7_error(e, "uuid_new_v7_batch: random source failure"))?;
    Ok(out)
}

/// A random v4 id as a string.
#[napi(catch_unwind)]
pub fn new_v4(env: &Env) -> Result<JsString<'_>> {
    let id = hyperuuid::v4::new_v4().map_err(|_| {
        err(
            ErrorCode::RandomSource,
            "uuid_new_v4: random source failure",
        )
    })?;
    js_id(env, &id)
}

/// A random v4 id as 16 bytes.
#[napi(catch_unwind)]
pub fn new_v4_bytes() -> Result<Uint8Array> {
    hyperuuid::v4::new_v4()
        .map(|u| Uint8Array::from(u.as_bytes().to_vec()))
        .map_err(|_| {
            err(
                ErrorCode::RandomSource,
                "uuid_new_v4: random source failure",
            )
        })
}

/// An ordered v7 id as a string, stamped with `time` (now when absent).
#[napi(catch_unwind)]
pub fn new_v7<'env>(env: &'env Env, time: Option<Either<f64, JsDate>>) -> Result<JsString<'env>> {
    let id = hyperuuid::v7::new_v7(millis(time)?)
        .map_err(|e| v7_error(e, "uuid_new_v7: random source failure"))?;
    js_id(env, &id)
}

/// An ordered v7 id as 16 bytes, stamped with `time` (now when absent).
#[napi(catch_unwind)]
pub fn new_v7_bytes(time: Option<Either<f64, JsDate>>) -> Result<Uint8Array> {
    hyperuuid::v7::new_v7(millis(time)?)
        .map(|u| Uint8Array::from(u.as_bytes().to_vec()))
        .map_err(|e| v7_error(e, "uuid_new_v7: random source failure"))
}

/// `count` ordered v7 ids as strings, stamped with `time` (now when absent).
#[napi(catch_unwind)]
pub fn new_v7_batch<'env>(
    env: &'env Env,
    count: f64,
    time: Option<Either<f64, JsDate>>,
) -> Result<Vec<JsString<'env>>> {
    let n = batch_count(count)?;
    let bytes = batch_bytes(n, time)?;
    bytes
        .as_chunks::<16>()
        .0
        .iter()
        .map(|raw| js_id(env, &Uuid::from_bytes(*raw)))
        .collect()
}

/// `count` ordered v7 ids packed into one byte array, stamped with `time` (now when absent).
#[napi(catch_unwind)]
pub fn new_v7_batch_bytes(count: f64, time: Option<Either<f64, JsDate>>) -> Result<Uint8Array> {
    let n = batch_count(count)?;
    Ok(Uint8Array::from(batch_bytes(n, time)?))
}

/// Fills `buffer` with `buffer.length / 16` ordered v7 ids in place, with no allocation.
#[napi(catch_unwind)]
pub fn fill_v7(env: &Env, mut buffer: Uint8Array, time: Option<Either<f64, JsDate>>) -> Result<()> {
    // SAFETY: the pointer is the caller's own buffer memory. No JavaScript runs between
    // conversion and the write (`value_of()` on a Date runs no user code), so the buffer
    // cannot be detached or resized, and only one buffer is taken, so nothing in Rust aliases
    // it. Caveat: a Uint8Array over a SharedArrayBuffer that a worker writes concurrently is a
    // data race the caller created, as for every napi typed-array door; shared buffers are
    // not rejected here.
    let out: &mut [u8] = unsafe { buffer.as_mut() };
    if !out.len().is_multiple_of(16) {
        return Err(err(
            ErrorCode::InvalidBuffer,
            "buffer length must be a multiple of 16 (one whole UUID per 16 bytes)",
        ));
    }
    if out.len() / 16 > MAX_BATCH as usize {
        return Err(err(ErrorCode::BatchTooLarge, BATCH_MESSAGE));
    }
    let ids = (out.len() / 16) as u32;
    hyperuuid::v7::new_v7_batch(millis(time)?, ids, out)
        .map_err(|e| v7_error(e, "uuid_new_v7_batch: random source failure"))?;
    // On the WebAssembly build the slice above is emnapi's guest-memory copy of the caller's
    // view, not the view itself, so the ids are copied back into JavaScript here. Natively the
    // slice is the view's own memory and there is nothing to sync; napi only defines `sync` for
    // wasm, hence the cfg.
    #[cfg(target_family = "wasm")]
    buffer.sync(env);
    #[cfg(not(target_family = "wasm"))]
    let _ = env;
    Ok(())
}

/// The largest count one v7 batch call accepts.
#[napi]
pub const MAX_V7_BATCH: u32 = hyperuuid::v7::MAX_BATCH;

/// The byte order an id is read in.
#[napi]
pub enum Layout {
    Rfc9562 = 1,
    SqlServer = 2,
}

/// The variant bits of an id.
#[napi]
pub enum Variant {
    Ncs = 1,
    Rfc9562 = 2,
    Microsoft = 3,
    Future = 4,
}

/// The nil id, all zero bits.
#[napi]
pub const NIL: &str = "00000000-0000-0000-0000-000000000000";
/// The max id, all one bits.
#[napi]
pub const MAX: &str = "ffffffff-ffff-ffff-ffff-ffffffffffff";

/// A `layout` argument: exactly 1 or 2, or absent for RFC 9562. napi's own enum conversion
/// truncates to i32 (`1.5` would read as 1), so the number crosses as `f64` and is checked here.
fn layout_arg(layout: Option<f64>) -> napi::Result<hyperuuid::Layout, ErrorCode> {
    match layout {
        None => Ok(hyperuuid::Layout::Rfc9562),
        Some(1.0) => Ok(hyperuuid::Layout::Rfc9562),
        Some(2.0) => Ok(hyperuuid::Layout::SqlServer),
        Some(_) => Err(napi::Error::new(
            ErrorCode::Napi(Status::InvalidArg),
            "layout must be Layout.Rfc9562 (1) or Layout.SqlServer (2)".to_string(),
        )),
    }
}

const INVALID_ID: &str = "id must be 32 hex digits, 36 with hyphens, or exactly 16 bytes";

/// Value of each ASCII hex digit, 0xff for every other byte.
const NIBBLE: [u8; 256] = {
    let mut table = [0xffu8; 256];
    let mut i = 0;
    while i < 10 {
        table[b'0' as usize + i] = i as u8;
        i += 1;
    }
    let mut i = 0;
    while i < 6 {
        table[b'a' as usize + i] = 10 + i as u8;
        table[b'A' as usize + i] = 10 + i as u8;
        i += 1;
    }
    table
};

/// Reads an id from 32 hex digits, 8-4-4-4-12 hyphenated, or exactly 16 bytes. Any case.
/// The core's own parser takes only the hyphenated form, so the leniency lives here.
fn parse16(id: Either<String, Uint8Array>) -> Result<Uuid> {
    let invalid = || err(ErrorCode::InvalidId, INVALID_ID);
    let text = match id {
        Either::B(bytes) => {
            let raw: [u8; 16] = bytes.as_ref().try_into().map_err(|_| invalid())?;
            return Ok(Uuid::from_bytes(raw));
        }
        Either::A(text) => text,
    };
    let text = text.as_bytes();
    let hyphenated = match text.len() {
        32 => false,
        36 => true,
        _ => return Err(invalid()),
    };
    let mut raw = [0u8; 16];
    let mut at = 0;
    for byte in &mut raw {
        if hyphenated && matches!(at, 8 | 13 | 18 | 23) {
            if text[at] != b'-' {
                return Err(invalid());
            }
            at += 1;
        }
        let high = NIBBLE[usize::from(text[at])];
        let low = NIBBLE[usize::from(text[at + 1])];
        if (high | low) > 0xf {
            return Err(invalid());
        }
        *byte = high << 4 | low;
        at += 2;
    }
    Ok(Uuid::from_bytes(raw))
}

/// A `Date` for a millisecond count. 48-bit values always fit, so the error arm is unreachable
/// in practice; it maps to the range code rather than leaking a bare napi status.
fn date<'env>(env: &'env Env, ms: u64) -> Result<JsDate<'env>> {
    env.create_date(ms as f64)
        .map_err(|_| err(ErrorCode::TimestampRange, TIMESTAMP_MESSAGE))
}

/// The version nibble of an id, read in `layout` (RFC 9562 when absent).
#[napi(catch_unwind)]
pub fn version(
    id: Either<String, Uint8Array>,
    #[napi(ts_arg_type = "Layout | undefined | null")] layout: Option<f64>,
) -> Result<u8> {
    Ok(parse16(id)?.version_in(layout_arg(layout)?))
}

/// The variant of an id.
#[napi(catch_unwind)]
pub fn variant(id: Either<String, Uint8Array>) -> Result<Variant> {
    Ok(match parse16(id)?.variant() {
        hyperuuid::Variant::Ncs => Variant::Ncs,
        hyperuuid::Variant::Rfc9562 => Variant::Rfc9562,
        hyperuuid::Variant::Microsoft => Variant::Microsoft,
        hyperuuid::Variant::Future => Variant::Future,
    })
}

/// Whether the id is RFC variant with this version. The version crosses as `f64` because
/// napi's `u8` is ToUint8 (`263` would become 7); anything non-integral or outside 0..=15
/// names no UUID version, so it answers false.
#[napi(catch_unwind)]
pub fn is_rfc(
    id: Either<String, Uint8Array>,
    version: f64,
    #[napi(ts_arg_type = "Layout | undefined | null")] layout: Option<f64>,
) -> Result<bool> {
    let uuid = parse16(id)?;
    if !version.is_finite() || version.fract() != 0.0 || !(0.0..=15.0).contains(&version) {
        return Ok(false);
    }
    Ok(uuid.is_rfc_in(version as u8, layout_arg(layout)?))
}

/// The `Date` an id carries, or `null` when it is not an RFC 9562 v7 id in `layout`.
#[napi(catch_unwind)]
pub fn get_timestamp<'env>(
    env: &'env Env,
    id: Either<String, Uint8Array>,
    #[napi(ts_arg_type = "Layout | undefined | null")] layout: Option<f64>,
) -> Result<Option<JsDate<'env>>> {
    let uuid = parse16(id)?;
    hyperuuid::get_timestamp_in(&uuid, layout_arg(layout)?)
        .map(|t| date(env, t.to_unix_millis()))
        .transpose()
}

/// The 48 timestamp bits read unchecked, whatever the version.
#[napi(catch_unwind)]
pub fn v7_unix_millis(
    id: Either<String, Uint8Array>,
    #[napi(ts_arg_type = "Layout | undefined | null")] layout: Option<f64>,
) -> Result<f64> {
    Ok(hyperuuid::v7::unix_millis_in(&parse16(id)?, layout_arg(layout)?) as f64)
}

/// The 48 timestamp bits as a `Date`, read unchecked, whatever the version.
#[napi(catch_unwind)]
pub fn v7_timestamp<'env>(
    env: &'env Env,
    id: Either<String, Uint8Array>,
    #[napi(ts_arg_type = "Layout | undefined | null")] layout: Option<f64>,
) -> Result<JsDate<'env>> {
    let ms = hyperuuid::v7::unix_millis_in(&parse16(id)?, layout_arg(layout)?);
    date(env, ms)
}

/// A name-based v5 id. A string name is its UTF-8 bytes; bytes are used as given.
#[napi(catch_unwind)]
pub fn new_v5<'env>(
    env: &'env Env,
    namespace: Either<String, Uint8Array>,
    name: Either<String, Uint8Array>,
) -> Result<JsString<'env>> {
    let namespace = parse16(namespace)?;
    let id = match &name {
        Either::A(text) => hyperuuid::v5::new_v5(namespace, text.as_bytes()),
        Either::B(bytes) => hyperuuid::v5::new_v5(namespace, bytes.as_ref()),
    };
    js_id(env, &id)
}

const WRONG_VERSION: &str = "the value is not a UUID of the version this conversion takes";

/// The SQL Server byte order of a v7 id (the v8 form that sorts as v7 does there).
#[napi(catch_unwind)]
pub fn v7_to_sql_order<'env>(
    env: &'env Env,
    id: Either<String, Uint8Array>,
) -> Result<JsString<'env>> {
    let stored = hyperuuid::v7::to_sql_order(&parse16(id)?)
        .map_err(|_| err(ErrorCode::WrongVersion, WRONG_VERSION))?;
    js_id(env, &stored)
}

/// The inverse of `v7ToSqlOrder`.
#[napi(catch_unwind)]
pub fn v7_from_sql_order<'env>(
    env: &'env Env,
    id: Either<String, Uint8Array>,
) -> Result<JsString<'env>> {
    let id = hyperuuid::v7::to_rfc_order(&parse16(id)?)
        .map_err(|_| err(ErrorCode::WrongVersion, WRONG_VERSION))?;
    js_id(env, &id)
}
