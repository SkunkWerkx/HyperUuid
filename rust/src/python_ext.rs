//! The Python backend: this crate linked straight into a CPython extension module via
//! PyO3. A call here is an ordinary `METH_FASTCALL` extension call into a direct Rust
//! call — no dlopen, no C-ABI hop, no per-call boxing, no ctypes marshalling. Ported from
//! HyperCast's proven `hypercast_native` pattern.
//!
//! UUID construction uses the fastuuid-style fast path — `UUID.__new__` plus
//! `object.__setattr__` of the `int` and `is_safe` slots — because `UUID.__init__`'s
//! validation costs more than the entire native call. It leans on `uuid.UUID`'s
//! `__slots__` layout, which has been stable for over a decade; the Python test suite pins
//! the invariant (fast-constructed UUIDs compare equal to `UUID(bytes=...)`-constructed
//! ones, fields included) so any future drift fails loudly instead of subtly.

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use pyo3::exceptions::{PyOverflowError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDateTime, PyList};

use crate::{v4, v5, v6, v7, Uuid};

static UUID_CLASS: OnceLock<Py<PyAny>> = OnceLock::new();
static UUID_NEW: OnceLock<Py<PyAny>> = OnceLock::new();
static OBJECT_SETATTR: OnceLock<Py<PyAny>> = OnceLock::new();
static IS_SAFE_UNKNOWN: OnceLock<Py<PyAny>> = OnceLock::new();

fn cached<'py>(py: Python<'py>, cell: &'static OnceLock<Py<PyAny>>) -> PyResult<&'py Bound<'py, PyAny>> {
    cell.get()
        .map(|value| value.bind(py))
        .ok_or_else(|| PyRuntimeError::new_err("hyperuuid_native used before _bind"))
}

/// Builds a stdlib `uuid.UUID` from 16 RFC-ordered bytes via the pinned fast path.
fn make_uuid(py: Python<'_>, bytes: [u8; 16]) -> PyResult<Py<PyAny>> {
    let class = cached(py, &UUID_CLASS)?;
    let instance = cached(py, &UUID_NEW)?.call1((class,))?;
    let setattr = cached(py, &OBJECT_SETATTR)?;
    setattr.call1((&instance, "int", u128::from_be_bytes(bytes)))?;
    setattr.call1((&instance, "is_safe", cached(py, &IS_SAFE_UNKNOWN)?))?;
    Ok(instance.unbind())
}

/// Reads a `uuid.UUID`'s 16 RFC-ordered bytes back out via its `int` slot.
fn uuid_bytes(value: &Bound<'_, PyAny>) -> PyResult<[u8; 16]> {
    Ok(value.getattr("int")?.extract::<u128>()?.to_be_bytes())
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

#[derive(FromPyObject)]
enum Name<'py> {
    #[pyo3(transparent)]
    Str(Bound<'py, pyo3::types::PyString>),
    #[pyo3(transparent)]
    Bytes(Bound<'py, PyBytes>),
}

#[pyfunction]
fn new_v5(py: Python<'_>, namespace: Bound<'_, PyAny>, name: Name<'_>) -> PyResult<Py<PyAny>> {
    let namespace = Uuid::from_bytes(uuid_bytes(&namespace)?);
    // to_str() needs non-limited-API access, unavailable under abi3-py39; to_cow() is the
    // abi3-safe equivalent, but returns an owned Cow rather than borrowing directly from
    // `text` the way to_str() did, so the Cow needs its own binding to outlive the match.
    let name_owned;
    let name_bytes: &[u8] = match &name {
        Name::Str(text) => {
            name_owned = text.to_cow()?;
            name_owned.as_bytes()
        }
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

#[pyfunction]
#[pyo3(signature = (count, unix_millis = None))]
fn new_v6_batch(py: Python<'_>, count: usize, unix_millis: Option<u64>) -> PyResult<Py<PyAny>> {
    let mut raw = vec![0u8; count * 16];
    match v6::new_v6_batch(millis_or_now(unix_millis), count as u32, &mut raw) {
        Ok(()) => Ok(batch_list(py, &raw)?.into_any().unbind()),
        Err(v6::NewV6Error::TimestampOutOfRange) => Err(PyValueError::new_err(
            "unix_millis does not fit the 60-bit v6 timestamp field",
        )),
        Err(_) => Err(PyRuntimeError::new_err("uuid_new_v6_batch: random source failure")),
    }
}

#[pyfunction]
#[pyo3(signature = (count, unix_millis = None))]
fn new_v7_batch(py: Python<'_>, count: usize, unix_millis: Option<u64>) -> PyResult<Py<PyAny>> {
    let mut raw = vec![0u8; count * 16];
    match v7::new_v7_batch(millis_or_now(unix_millis), count as u32, &mut raw) {
        Ok(()) => Ok(batch_list(py, &raw)?.into_any().unbind()),
        Err(v7::NewV7Error::TimestampOutOfRange) => Err(PyValueError::new_err(
            "unix_millis must be non-negative and fit within 48 bits",
        )),
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
    Ok(PyDateTime::new(
        py,
        year as i32,
        month,
        day,
        hour as u8,
        minute as u8,
        second as u8,
        micros,
        Some(&pyo3::types::timezone_utc(py)),
    )?
    .into_any()
    .unbind())
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

/// Caches `uuid.UUID`, its `__new__`, `object.__setattr__`, and `SafeUUID.unknown` for the
/// pinned fast constructor.
#[pyfunction]
fn _bind(py: Python<'_>) -> PyResult<()> {
    let uuid_module = py.import("uuid")?;
    let class = uuid_module.getattr("UUID")?;
    let _ = UUID_NEW.set(class.getattr("__new__")?.unbind());
    let _ = UUID_CLASS.set(class.unbind());
    let _ = OBJECT_SETATTR.set(
        py.import("builtins")?.getattr("object")?.getattr("__setattr__")?.unbind(),
    );
    let _ = IS_SAFE_UNKNOWN.set(uuid_module.getattr("SafeUUID")?.getattr("unknown")?.unbind());
    Ok(())
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
    m.add_function(wrap_pyfunction!(v6_timestamp, m)?)?;
    m.add_function(wrap_pyfunction!(v7_timestamp, m)?)?;
    m.add_function(wrap_pyfunction!(v6_to_sql_order, m)?)?;
    m.add_function(wrap_pyfunction!(v6_from_sql_order, m)?)?;
    m.add_function(wrap_pyfunction!(v7_to_sql_order, m)?)?;
    m.add_function(wrap_pyfunction!(v7_from_sql_order, m)?)?;
    m.add_function(wrap_pyfunction!(_bind, m)?)?;
    Ok(())
}
