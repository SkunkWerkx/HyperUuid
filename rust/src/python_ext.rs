//! The Python backend: this crate linked straight into a CPython extension module via
//! PyO3. A call here is an ordinary `METH_FASTCALL` extension call into a direct Rust
//! call — no dlopen, no C-ABI hop, no per-call boxing, no ctypes marshalling. Ported from
//! HyperCast's proven `hypercast._native` pattern.
//!
//! UUID construction uses the fastuuid-style fast path — an instance allocated without
//! `UUID.__init__`, then its `int` and `is_safe` slots set — because `UUID.__init__`'s
//! validation costs more than the entire native call. It leans on `uuid.UUID`'s
//! `__slots__` layout, which has been stable for over a decade; the Python test suite pins
//! the invariant (fast-constructed UUIDs compare equal to `UUID(bytes=...)`-constructed
//! ones, fields included) so any future drift fails loudly instead of subtly.
//!
//! What a call costs here is almost entirely the Python objects it hands back, so those are
//! built through the C API's own entry points rather than by calling Python callables:
//! `PyType_GenericAlloc` and `PyObject_GenericSetAttr` are what `object.__new__` and
//! `object.__setattr__` do underneath, without a call, an argument tuple or a fresh `str`
//! per attribute name. All of it is in the stable ABI, so the one-wheel-per-platform build
//! is unchanged.

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use pyo3::exceptions::{
    PyMemoryError, PyOverflowError, PyRuntimeError, PyTypeError, PyValueError,
};
use pyo3::ffi;
use pyo3::intern;
use pyo3::prelude::*;
use pyo3::types::{PyByteArray, PyBytes, PyList, PyString};

use crate::{v4, v5, v6, v7, Uuid};

static UUID_CLASS: OnceLock<Py<PyAny>> = OnceLock::new();
static IS_SAFE_UNKNOWN: OnceLock<Py<PyAny>> = OnceLock::new();
static DATETIME_CLASS: OnceLock<Py<PyAny>> = OnceLock::new();
static UTC: OnceLock<Py<PyAny>> = OnceLock::new();
static SIXTY_FOUR: OnceLock<Py<PyAny>> = OnceLock::new();

fn cached<'py>(py: Python<'py>, cell: &'static OnceLock<Py<PyAny>>) -> PyResult<&'py Bound<'py, PyAny>> {
    cell.get()
        .map(|value| value.bind(py))
        .ok_or_else(|| PyRuntimeError::new_err("hyperuuid._native used before _bind"))
}

/// A Python `int` holding 16 big-endian bytes. The stable ABI has no byte-array constructor
/// for `int` before 3.14, so it goes through `PyLong_FromString` in base 16 — one allocation,
/// and measured faster than joining two 64-bit halves with a shift and an or (three).
fn int_from_be_bytes<'py>(py: Python<'py>, bytes: [u8; 16]) -> PyResult<Bound<'py, PyAny>> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    // 32 digits and the terminating NUL PyLong_FromString reads up to.
    let mut text = [0u8; 33];
    for (i, byte) in bytes.iter().enumerate() {
        text[2 * i] = HEX[(byte >> 4) as usize];
        text[2 * i + 1] = HEX[(byte & 15) as usize];
    }
    // SAFETY: `text` is NUL-terminated ASCII; the call returns a new reference, or null with
    // an exception set.
    unsafe {
        Bound::from_owned_ptr_or_err(py, ffi::PyLong_FromString(text.as_ptr().cast(), std::ptr::null_mut(), 16))
    }
}

/// Builds a stdlib `uuid.UUID` from 16 RFC-ordered bytes via the pinned fast path.
fn make_uuid(py: Python<'_>, bytes: [u8; 16]) -> PyResult<Py<PyAny>> {
    let class = cached(py, &UUID_CLASS)?;
    let value = int_from_be_bytes(py, bytes)?;
    let is_safe = cached(py, &IS_SAFE_UNKNOWN)?;
    // SAFETY: UUID_CLASS is a type object (bound in _bind); PyType_GenericAlloc is the
    // tp_alloc `object.__new__` itself calls for it, and PyObject_GenericSetAttr is what
    // `object.__setattr__` calls — both check their arguments and report failure by
    // return value with an exception set.
    unsafe {
        let instance = Bound::from_owned_ptr_or_err(py, ffi::PyType_GenericAlloc(class.as_ptr().cast(), 0))?;
        if ffi::PyObject_GenericSetAttr(instance.as_ptr(), intern!(py, "int").as_ptr(), value.as_ptr()) != 0
            || ffi::PyObject_GenericSetAttr(instance.as_ptr(), intern!(py, "is_safe").as_ptr(), is_safe.as_ptr())
                != 0
        {
            return Err(PyErr::fetch(py));
        }
        Ok(instance.unbind())
    }
}

/// Reads a `uuid.UUID`'s 16 RFC-ordered bytes back out via its `int` slot: the low 64 bits
/// directly, the high 64 after a shift. Anything that is not an integer of at most 128
/// bits is refused by the conversions themselves (`TypeError`, `OverflowError`).
fn uuid_bytes(value: &Bound<'_, PyAny>) -> PyResult<[u8; 16]> {
    let py = value.py();
    let int = value.getattr(intern!(py, "int"))?;
    // SAFETY: the conversions take any object and report failure as u64::MAX with an
    // exception set, which is checked before the value is used.
    unsafe {
        let low = ffi::PyLong_AsUnsignedLongLongMask(int.as_ptr());
        if low == u64::MAX && !ffi::PyErr_Occurred().is_null() {
            return Err(PyErr::fetch(py));
        }
        let shifted = Bound::from_owned_ptr_or_err(
            py,
            ffi::PyNumber_Rshift(int.as_ptr(), cached(py, &SIXTY_FOUR)?.as_ptr()),
        )?;
        let high = ffi::PyLong_AsUnsignedLongLong(shifted.as_ptr());
        if high == u64::MAX && !ffi::PyErr_Occurred().is_null() {
            return Err(PyErr::fetch(py));
        }
        Ok(((u128::from(high) << 64) | u128::from(low)).to_be_bytes())
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

#[pyfunction]
fn new_v4(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let id = v4::new_v4().map_err(|_| PyRuntimeError::new_err("uuid_new_v4: random source failure"))?;
    make_uuid(py, *id.as_bytes())
}

/// A v5 name: `str` (its cached UTF-8, borrowed) or `bytes` — a zero-copy view into the
/// caller's own object either way.
enum Name<'py> {
    Str(Bound<'py, PyString>),
    Bytes(Bound<'py, PyBytes>),
}

// Hand-written rather than `#[derive(FromPyObject)]`: the derived two-variant extractor
// tries `Str` first, and a failed variant is not free — it builds a `TypeError` with a
// formatted message and a cause chain, which the next variant's success then throws away.
// That was about a microsecond on every `bytes` name, as much as the hash itself. Checking
// the type tag directly costs nothing on either path.
impl<'py> FromPyObject<'_, 'py> for Name<'py> {
    type Error = PyErr;

    fn extract(obj: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        if let Ok(text) = obj.cast::<PyString>() {
            Ok(Name::Str(text.to_owned()))
        } else if let Ok(bytes) = obj.cast::<PyBytes>() {
            Ok(Name::Bytes(bytes.to_owned()))
        } else {
            // PyO3 prefixes the argument name, so new_v5 raises
            // "argument 'name': must be str or bytes" — the wasm backend's own words.
            Err(PyTypeError::new_err("must be str or bytes"))
        }
    }
}

#[pyfunction]
fn new_v5(py: Python<'_>, namespace: Bound<'_, PyAny>, name: Name<'_>) -> PyResult<Py<PyAny>> {
    let namespace = Uuid::from_bytes(uuid_bytes(&namespace)?);
    // to_str() borrows the str's own cached UTF-8 with no copy. It is in the limited API from
    // 3.10, below this extension's floor (abi3-py311); under the old abi3-py39 build the
    // only option was to_cow(), which encoded and copied the name on every call.
    let name_bytes: &[u8] = match &name {
        Name::Str(text) => text.to_str()?.as_bytes(),
        Name::Bytes(bytes) => bytes.as_bytes(),
    };
    make_uuid(py, *v5::new_v5(namespace, name_bytes).as_bytes())
}

fn millis_or_now(unix_millis: Option<u64>) -> u64 {
    unix_millis.unwrap_or_else(now_millis)
}

#[pyfunction]
#[pyo3(signature = (unix_millis = None))]
fn new_v6(py: Python<'_>, unix_millis: Option<u64>) -> PyResult<Py<PyAny>> {
    match v6::new_v6(millis_or_now(unix_millis)) {
        Ok(id) => make_uuid(py, *id.as_bytes()),
        Err(v6::NewV6Error::TimestampOutOfRange) => Err(PyValueError::new_err(
            "unix_millis does not fit the 60-bit v6 timestamp field",
        )),
        Err(e @ v6::NewV6Error::BufferTooSmall) => Err(PyValueError::new_err(e.to_string())),
        Err(_) => Err(PyRuntimeError::new_err("uuid_new_v6: random source failure")),
    }
}

#[pyfunction]
#[pyo3(signature = (unix_millis = None))]
fn new_v7(py: Python<'_>, unix_millis: Option<u64>) -> PyResult<Py<PyAny>> {
    match v7::new_v7(millis_or_now(unix_millis)) {
        Ok(id) => make_uuid(py, *id.as_bytes()),
        Err(v7::NewV7Error::TimestampOutOfRange) => Err(PyValueError::new_err(
            "unix_millis must be non-negative and fit within 48 bits",
        )),
        Err(e @ v7::NewV7Error::BufferTooSmall) => Err(PyValueError::new_err(e.to_string())),
        Err(_) => Err(PyRuntimeError::new_err("uuid_new_v7: random source failure")),
    }
}

fn batch_list<'py>(py: Python<'py>, raw: &[u8]) -> PyResult<Bound<'py, PyList>> {
    let list = PyList::empty(py);
    for chunk in raw.chunks_exact(16) {
        list.append(make_uuid(py, chunk.try_into().unwrap())?)?;
    }
    Ok(list)
}

/// The zeroed destination for a `count`-UUID batch, 16 bytes each.
///
/// `count` arrives as a `u32` because that is the core's own batch parameter: taking a
/// `usize` and narrowing it mints a different number of UUIDs than the caller asked for
/// (`2**32 + 1` used to come back as one). The allocation is fallible for the same reason
/// the count is typed — `vec![0; n]` aborts the whole interpreter when the allocator says
/// no, where an oversized request is the caller's to hear about as a `MemoryError`.
fn batch_buffer(count: u32) -> PyResult<Vec<u8>> {
    let too_large = || PyMemoryError::new_err(format!("cannot allocate a batch of {count} UUIDs"));
    // u32::MAX * 16 fits a 64-bit usize, not a 32-bit one.
    let len = (count as usize).checked_mul(16).ok_or_else(too_large)?;
    let mut raw = Vec::new();
    raw.try_reserve_exact(len).map_err(|_| too_large())?;
    raw.resize(len, 0);
    Ok(raw)
}

#[pyfunction]
#[pyo3(signature = (count, unix_millis = None))]
fn new_v6_batch(py: Python<'_>, count: u32, unix_millis: Option<u64>) -> PyResult<Py<PyAny>> {
    let mut raw = batch_buffer(count)?;
    match v6::new_v6_batch(millis_or_now(unix_millis), count, &mut raw) {
        Ok(()) => Ok(batch_list(py, &raw)?.into_any().unbind()),
        Err(v6::NewV6Error::TimestampOutOfRange) => Err(PyValueError::new_err(
            "unix_millis does not fit the 60-bit v6 timestamp field",
        )),
        Err(e @ v6::NewV6Error::BufferTooSmall) => Err(PyValueError::new_err(e.to_string())),
        Err(_) => Err(PyRuntimeError::new_err("uuid_new_v6_batch: random source failure")),
    }
}

#[pyfunction]
#[pyo3(signature = (count, unix_millis = None))]
fn new_v7_batch(py: Python<'_>, count: u32, unix_millis: Option<u64>) -> PyResult<Py<PyAny>> {
    let mut raw = batch_buffer(count)?;
    match v7::new_v7_batch(millis_or_now(unix_millis), count, &mut raw) {
        Ok(()) => Ok(batch_list(py, &raw)?.into_any().unbind()),
        Err(v7::NewV7Error::TimestampOutOfRange) => Err(PyValueError::new_err(
            "unix_millis must be non-negative and fit within 48 bits",
        )),
        Err(e @ v7::NewV7Error::BufferTooSmall) => Err(PyValueError::new_err(e.to_string())),
        Err(_) => Err(PyRuntimeError::new_err("uuid_new_v7_batch: random source failure")),
    }
}

/// Hinnant's civil_from_days — presenting embedded millis as a datetime without a
/// strftime round trip.
fn civil_from_days(days: i64) -> (i64, u8, u8) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_shifted = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_shifted + 2) / 5 + 1) as u8;
    let month = (if month_shifted < 10 { month_shifted + 3 } else { month_shifted - 9 }) as u8;
    (year + i64::from(month <= 2), month, day)
}

fn millis_datetime(py: Python<'_>, millis: u64) -> PyResult<Py<PyAny>> {
    let seconds = (millis / 1_000) as i64;
    let micros = ((millis % 1_000) * 1_000) as u32;
    let days = seconds.div_euclid(86_400);
    let second_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    if year > 9_999 {
        // datetime cannot represent year 10000+, and the RFC's 48-bit field legitimately
        // reaches 10889.
        return Err(PyOverflowError::new_err("embedded timestamp is past datetime's year-9999 ceiling"));
    }
    let (hour, rest) = (second_of_day / 3_600, second_of_day % 3_600);
    let (minute, second) = (rest / 60, rest % 60);
    // `datetime(state, tzinfo)` is the constructor `datetime.__reduce__` names: ten bytes —
    // year (two, big-endian), month, day, hour, minute, second, microsecond (three,
    // big-endian) — and the tzinfo. It is the pickle format, so it cannot change under a
    // pickle written by an older Python, and it builds the same object as the eight-argument
    // constructor in a quarter of the time: one bytes object instead of seven ints, and no
    // per-field range checks to repeat (the fields above are in range by construction).
    let state = [
        (year >> 8) as u8,
        year as u8,
        month,
        day,
        hour as u8,
        minute as u8,
        second as u8,
        (micros >> 16) as u8,
        (micros >> 8) as u8,
        micros as u8,
    ];
    Ok(cached(py, &DATETIME_CLASS)?
        .call1((PyBytes::new(py, &state), cached(py, &UTC)?))?
        .unbind())
}

/// The embedded timestamp as the integer the core returns — Unix-epoch milliseconds — with
/// no `datetime` built around it.
#[pyfunction]
fn v6_unix_millis(uuid_value: Bound<'_, PyAny>) -> PyResult<u64> {
    Ok(v6::unix_millis(&Uuid::from_bytes(uuid_bytes(&uuid_value)?)))
}

#[pyfunction]
fn v7_unix_millis(uuid_value: Bound<'_, PyAny>) -> PyResult<u64> {
    Ok(v7::unix_millis(&Uuid::from_bytes(uuid_bytes(&uuid_value)?)))
}

#[pyfunction]
fn v6_timestamp(py: Python<'_>, uuid_value: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    millis_datetime(py, v6::unix_millis(&Uuid::from_bytes(uuid_bytes(&uuid_value)?)))
}

#[pyfunction]
fn v7_timestamp(py: Python<'_>, uuid_value: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    millis_datetime(py, v7::unix_millis(&Uuid::from_bytes(uuid_bytes(&uuid_value)?)))
}

macro_rules! order_fns {
    ($($door:ident => $module:ident :: $function:ident),+ $(,)?) => {$(
        #[pyfunction]
        fn $door(py: Python<'_>, uuid_value: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
            let converted = $module::$function(&Uuid::from_bytes(uuid_bytes(&uuid_value)?));
            make_uuid(py, *converted.as_bytes())
        }
    )+};
}

order_fns! {
    v7_to_sql_order => v7::to_sql_order,
    v7_from_sql_order => v7::to_rfc_order,
    v6_to_sql_order => v6::to_sql_order,
    v6_from_sql_order => v6::to_rfc_order,
}

/// Caches `uuid.UUID` and `SafeUUID.unknown` for the pinned fast constructor, and
/// `datetime.datetime` and `timezone.utc` for the timestamps.
#[pyfunction]
fn _bind(py: Python<'_>) -> PyResult<()> {
    let uuid_module = py.import("uuid")?;
    let _ = UUID_CLASS.set(uuid_module.getattr("UUID")?.unbind());
    let _ = IS_SAFE_UNKNOWN.set(uuid_module.getattr("SafeUUID")?.getattr("unknown")?.unbind());
    let datetime_module = py.import("datetime")?;
    let _ = DATETIME_CLASS.set(datetime_module.getattr("datetime")?.unbind());
    let _ = UTC.set(datetime_module.getattr("timezone")?.getattr("utc")?.unbind());
    let _ = SIXTY_FOUR.set(64u8.into_pyobject(py)?.into_any().unbind());
    Ok(())
}

/// This library's version as `"major.minor.patch"`, decoded from the same packed
/// `hyperuuid_version` export every other binding probes.
#[pyfunction]
fn native_version() -> String {
    let packed = crate::hyperuuid_version();
    format!("{}.{}.{}", packed >> 16, (packed >> 8) & 0xff, packed & 0xff)
}

/// Fills a `bytearray` with raw RFC 9562-ordered UUID bytes, 16 per UUID.
///
/// This is the destination-buffer path, and it never constructs a single `uuid.UUID`. That is
/// the whole point in Python, where object construction — not the native call — dominates a
/// batch: `new_v7_batch(1000)` builds a thousand `uuid.UUID` instances, each an allocation
/// and two slot stores, while this writes the same 16000 bytes
/// with none of that. Measured at ~15x faster for a 1000-UUID batch, which lands it on the
/// same native ceiling the Go and C# bindings hit.
///
/// `bytearray` specifically, not the general writable buffer protocol, for now: `PyBuffer`
/// requires `Py_buffer`, which entered CPython's stable ABI in 3.11. That is this
/// extension's floor (`abi3-py311`), so `memoryview`, `mmap` and NumPy arrays are possible
/// here and simply not built yet.
fn fill_bytes_impl(
    buffer: &Bound<'_, PyByteArray>,
    unix_millis: Option<u64>,
    v7_not_v6: bool,
) -> PyResult<()> {
    let len = buffer.len();
    if len % 16 != 0 {
        return Err(PyValueError::new_err(
            "buffer length must be a multiple of 16 (one whole UUID per 16 bytes)",
        ));
    }
    // A zero-length buffer goes through too: the core writes nothing for a count of 0 but
    // still checks the timestamp, so an empty fill and an empty batch reject alike.
    // The core's batch count is a u32; a buffer past that (64 GiB) is refused, never narrowed
    // into a fill that silently stops short.
    let count = u32::try_from(len / 16).map_err(|_| {
        PyValueError::new_err("buffer holds more UUIDs than one batch can mint (4294967295)")
    })?;
    let millis = millis_or_now(unix_millis);

    // SAFETY: this slice aliases the bytearray's storage, so it must not be held across
    // anything that can run arbitrary Python — which could resize the bytearray and free the
    // buffer underneath it. Nothing below does: new_v6_batch/new_v7_batch are pure Rust over
    // a byte slice, take no Python handle, and cannot re-enter the interpreter. The slice is
    // dropped at the end of this call, before control returns to Python.
    let out = unsafe { buffer.as_bytes_mut() };

    if v7_not_v6 {
        match v7::new_v7_batch(millis, count, out) {
            Ok(()) => Ok(()),
            Err(v7::NewV7Error::TimestampOutOfRange) => Err(PyValueError::new_err(
                "unix_millis must be non-negative and fit within 48 bits",
            )),
            Err(e @ v7::NewV7Error::BufferTooSmall) => Err(PyValueError::new_err(e.to_string())),
            Err(_) => Err(PyRuntimeError::new_err("uuid_new_v7_batch: random source failure")),
        }
    } else {
        match v6::new_v6_batch(millis, count, out) {
            Ok(()) => Ok(()),
            Err(v6::NewV6Error::TimestampOutOfRange) => Err(PyValueError::new_err(
                "unix_millis does not fit the 60-bit v6 timestamp field",
            )),
            Err(e @ v6::NewV6Error::BufferTooSmall) => Err(PyValueError::new_err(e.to_string())),
            Err(_) => Err(PyRuntimeError::new_err("uuid_new_v6_batch: random source failure")),
        }
    }
}

#[pyfunction]
#[pyo3(signature = (buffer, unix_millis = None))]
fn fill_v7_bytes(buffer: &Bound<'_, PyByteArray>, unix_millis: Option<u64>) -> PyResult<()> {
    fill_bytes_impl(buffer, unix_millis, true)
}

#[pyfunction]
#[pyo3(signature = (buffer, unix_millis = None))]
fn fill_v6_bytes(buffer: &Bound<'_, PyByteArray>, unix_millis: Option<u64>) -> PyResult<()> {
    fill_bytes_impl(buffer, unix_millis, false)
}

// Name must match module-name's last segment in pyproject.toml ("hyperuuid._native") —
// PyO3 generates a PyInit_<name> symbol from this function's own name, and maturin/Python's
// import machinery look for PyInit__native specifically (confirmed via a real build warning,
// not assumed).
#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(new_v4, m)?)?;
    m.add_function(wrap_pyfunction!(new_v5, m)?)?;
    m.add_function(wrap_pyfunction!(new_v6, m)?)?;
    m.add_function(wrap_pyfunction!(new_v7, m)?)?;
    m.add_function(wrap_pyfunction!(new_v6_batch, m)?)?;
    m.add_function(wrap_pyfunction!(new_v7_batch, m)?)?;
    m.add_function(wrap_pyfunction!(fill_v6_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(fill_v7_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(v6_timestamp, m)?)?;
    m.add_function(wrap_pyfunction!(v7_timestamp, m)?)?;
    m.add_function(wrap_pyfunction!(v6_unix_millis, m)?)?;
    m.add_function(wrap_pyfunction!(v7_unix_millis, m)?)?;
    m.add_function(wrap_pyfunction!(v6_to_sql_order, m)?)?;
    m.add_function(wrap_pyfunction!(v6_from_sql_order, m)?)?;
    m.add_function(wrap_pyfunction!(v7_to_sql_order, m)?)?;
    m.add_function(wrap_pyfunction!(v7_from_sql_order, m)?)?;
    m.add_function(wrap_pyfunction!(native_version, m)?)?;
    m.add_function(wrap_pyfunction!(_bind, m)?)?;
    Ok(())
}
