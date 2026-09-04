"""The WebAssembly backend: the same Rust core, compiled to ``wasm32-wasip1`` and run inside
CPython by `wasmtime-py <https://github.com/bytecodealliance/wasmtime-py>`_ instead of linked
in as a compiled extension. Nothing here is a second implementation — every call below lands
in the identical ``uuid_*`` C-ABI export the PyO3 extension and every other binding in this
repo call, just across a guest/host memory boundary rather than a direct function call.

Selected by ``hyperuuid`` itself (see ``__init__``): ``HYPERUUID_WASM=1`` forces it, and it is
the automatic fallback when no ``_native`` wheel matches the running interpreter and
``wasmtime`` is importable. It exposes exactly the surface ``__init__`` consumes from
``_native`` — same names, same argument shapes, same exception types and messages, the same
fast-constructed ``uuid.UUID`` objects — so the package above it never knows which one it got.

Three things about the crossing are load-bearing:

* **Buffers come from the guest.** A wasm module only sees its own linear memory, so the host
  cannot hand it a pointer the way every native binding does. The module exports wasi-libc's
  ``malloc``/``free`` (see ``rust/.cargo/config.toml``), and every buffer this backend fills —
  the 16-byte scratch, a v5 name, a batch destination — is one the guest's own allocator
  handed out. Picking a host-side offset past the data segments instead was tried and
  corrupted a batch: dlmalloc claims the tail of the initial memory on its first allocation.
* **Calls are serialized.** A wasmtime ``Store`` is not thread-safe, and the same Rust core's
  v7 counter lives inside this one instance, so one process-wide lock guards every call. Under
  the GIL that lock is uncontended; on a free-threaded build it is what keeps two threads out
  of one store.
* **The call path avoids wasmtime-py's per-call type lookup.** ``Func.__call__`` re-fetches
  the function's type from the engine and builds and frees a ``FuncType`` plus one ``ValType``
  wrapper per parameter and result on every call — measured at ~38 µs per call, almost none
  of it in the guest. The argument and result arrays are built once here and handed straight
  to the same ``wasmtime_func_call`` C entry point the library uses after that bookkeeping,
  which drops a call to ~3 µs. That path touches ``wasmtime._ffi``, which is not public API,
  so it is bound inside a ``try`` at load time and degrades to the public call — slow, never
  broken — if a wasmtime release moves it.
"""

from __future__ import annotations

import ctypes
import datetime
import threading
import time
import uuid as _uuid
from pathlib import Path

_PACKAGE_DIR = Path(__file__).resolve().parent
_MODULE_PATH = _PACKAGE_DIR / "native" / "wasm32-wasip1" / "hyperuuid.wasm"
# Development loop: the in-repo cargo build, the same fallback the Ruby runtime takes for
# its native library, so `HYPERUUID_WASM=1 pytest` needs nothing staged by hand.
_REPO_BUILD_PATH = (
    _PACKAGE_DIR.parent.parent.parent / "rust" / "target" / "wasm32-wasip1" / "release" / "hyperuuid.wasm"
)

# Signatures of the exports this backend calls, as (parameter kinds, result kind) in wasm
# value-type terms — the C ABI in rust/src/ffi.rs, with every pointer an i32 offset into the
# guest's memory. Kept explicit rather than read back from the engine so the direct call path
# below never has to ask wasmtime for a type at call time.
_I32, _I64 = "i32", "i64"
_SIGNATURES = {
    "uuid_new_v4": ((_I32,), _I32),
    "uuid_new_v5": ((_I32, _I32, _I32, _I32), _I32),
    "uuid_new_v6": ((_I64, _I32), _I32),
    "uuid_v6_unix_millis": ((_I32,), _I64),
    "uuid_new_v6_batch": ((_I64, _I32, _I32), _I32),
    "uuid_new_v7": ((_I64, _I32), _I32),
    "uuid_v7_unix_millis": ((_I32,), _I64),
    "uuid_new_v7_batch": ((_I64, _I32, _I32), _I32),
    "uuid_v7_to_sql_order": ((_I32,), None),
    "uuid_v7_to_rfc_order": ((_I32,), None),
    "uuid_v6_to_sql_order": ((_I32,), None),
    "uuid_v6_to_rfc_order": ((_I32,), None),
    "malloc": ((_I32,), _I32),
    "free": ((_I32,), None),
}

_EPOCH = datetime.datetime(1970, 1, 1, tzinfo=datetime.timezone.utc)

_UUID_NEW = _uuid.UUID.__new__
_OBJECT_SETATTR = object.__setattr__
_IS_SAFE_UNKNOWN = _uuid.SafeUUID.unknown


def _make_uuid(raw: bytes) -> _uuid.UUID:
    """The same fastuuid-style constructor ``_native`` uses — ``UUID.__new__`` plus direct
    slot assignment, because ``UUID.__init__``'s validation costs more than the guest call.
    ``tests/test_native_backend.py`` pins that the result is indistinguishable from
    ``UUID(bytes=...)``, and that pin runs against this backend too.
    """
    instance = _UUID_NEW(_uuid.UUID)
    _OBJECT_SETATTR(instance, "int", int.from_bytes(raw, "big"))
    _OBJECT_SETATTR(instance, "is_safe", _IS_SAFE_UNKNOWN)
    return instance


def _now_millis() -> int:
    return time.time_ns() // 1_000_000


def _u64(unix_millis: int | None) -> int:
    """``None`` means now; otherwise the value must be a ``u64``, matching PyO3's extraction
    of ``Option<u64>`` (a negative or 65-bit int fails there with ``OverflowError`` before the
    core ever sees it — reproduced here so the two backends reject identically).
    """
    if unix_millis is None:
        return _now_millis()
    value = int(unix_millis)
    if not 0 <= value < 1 << 64:
        raise OverflowError("unix_millis must fit in an unsigned 64-bit integer")
    return value


def _millis_datetime(millis: int) -> datetime.datetime:
    try:
        return _EPOCH + datetime.timedelta(milliseconds=millis)
    except OverflowError:
        # datetime cannot represent year 10000+, and the RFC's 48-bit field legitimately
        # reaches 10889. Same message the extension raises.
        raise OverflowError("embedded timestamp is past datetime's year-9999 ceiling") from None


class _Guest:
    """One instantiated module: its store, its memory, and a callable per export."""

    def __init__(self) -> None:
        import wasmtime

        path = _MODULE_PATH if _MODULE_PATH.is_file() else _REPO_BUILD_PATH
        if not path.is_file():
            raise ImportError(
                f"hyperuuid: {_MODULE_PATH} not found (this install was built without the "
                "wasm32-wasip1 module)"
            )
        engine = wasmtime.Engine()
        module = wasmtime.Module.from_file(engine, str(path))
        linker = wasmtime.Linker(engine)
        linker.define_wasi()
        self._store = wasmtime.Store(engine)
        # No stdio, no args, no env, no preopens: the module's only WASI need is random_get.
        self._store.set_wasi(wasmtime.WasiConfig())
        exports = linker.instantiate(self._store, module).exports(self._store)
        self._memory = exports["memory"]
        self._call = {name: self._bind(exports[name], name) for name in _SIGNATURES}

        # Guest-allocated scratch for the single-UUID doors, 16 in and 16 out, for the life
        # of the process — the same shape as the Java binding's per-thread segments and the
        # Ruby binding's per-thread Fiddle pointer, held once here because every call is
        # already serialized under the lock.
        self._in = self._malloc(16)
        self._out = self._malloc(16)
        # Grow-only buffers for the two variable-length inputs, so a steady stream of
        # same-sized batches or v5 names never touches the guest allocator again.
        self._name_ptr, self._name_cap = 0, 0
        self._batch_ptr, self._batch_cap = 0, 0

    # --- guest memory ---------------------------------------------------------------

    def _malloc(self, size: int) -> int:
        ptr = self._call["malloc"](size)
        if ptr == 0:
            raise MemoryError(f"hyperuuid: guest malloc({size}) failed")
        # Growing the guest memory can relocate it on the host side, and malloc is the only
        # call here that can grow it (no uuid_* export allocates), so the host address of
        # offset 0 is re-read exactly here and nowhere else — one fewer wasmtime call on
        # every read and write below.
        self._base = ctypes.addressof(self._memory.data_ptr(self._store).contents)
        return ptr

    def _read(self, ptr: int, length: int) -> bytes:
        return ctypes.string_at(self._base + ptr, length)

    def _write(self, ptr: int, data: bytes) -> None:
        ctypes.memmove(self._base + ptr, data, len(data))

    def _name_buffer(self, size: int) -> int:
        if size > self._name_cap:
            if self._name_ptr:
                self._call["free"](self._name_ptr)
            self._name_ptr, self._name_cap = self._malloc(size), size
        return self._name_ptr

    def _batch_buffer(self, size: int) -> int:
        if size > self._batch_cap:
            if self._batch_ptr:
                self._call["free"](self._batch_ptr)
            self._batch_ptr, self._batch_cap = self._malloc(size), size
        return self._batch_ptr

    # --- calls ----------------------------------------------------------------------

    def _bind(self, func, name):  # noqa: ANN001
        """Return a plain callable for one export. Tries the direct ``wasmtime_func_call``
        path (see the module docstring for why); falls back to ``Func.__call__`` if the
        private surface it needs is not where this was written against.
        """
        param_kinds, result_kind = _SIGNATURES[name]
        try:
            return self._bind_direct(func, param_kinds, result_kind, name)
        except Exception:  # noqa: BLE001 — any drift in wasmtime's internals lands here
            store = self._store
            if result_kind == _I64:
                return lambda *args: func(store, *args) & 0xFFFF_FFFF_FFFF_FFFF
            return lambda *args: func(store, *args)

    def _bind_direct(self, func, param_kinds, result_kind, name):  # noqa: ANN001
        from ctypes import POINTER, byref

        from wasmtime import _ffi as ffi

        n_params, n_results = len(param_kinds), 0 if result_kind is None else 1
        params = (ffi.wasmtime_val_t * max(n_params, 1))()
        results = (ffi.wasmtime_val_t * max(n_results, 1))()
        kinds = {_I32: int(ffi.WASMTIME_I32.value), _I64: int(ffi.WASMTIME_I64.value)}
        for slot, kind in zip(params, param_kinds):
            slot.kind = kinds[kind]
        setters = [
            (slot, "i64" if kind == _I64 else "i32") for slot, kind in zip(params, param_kinds)
        ]
        context = self._store._context()
        func_ref = byref(func._func)
        trap = POINTER(ffi.wasm_trap_t)()
        trap_ref = byref(trap)
        trap_size = ctypes.sizeof(trap)
        wasmtime_func_call = ffi.wasmtime_func_call
        wasmtime_error_delete = ffi.wasmtime_error_delete
        wasm_trap_delete = ffi.wasm_trap_delete

        def call(*args):  # noqa: ANN002, ANN202
            for (slot, field), value in zip(setters, args):
                # i64 slots take the u64 timestamp's two's-complement image; i32 slots are
                # offsets and counts, all well inside the positive range.
                setattr(slot.of, field, value - (1 << 64) if value >= 1 << 63 else value)
            error = wasmtime_func_call(context, func_ref, params, n_params, results, n_results, trap_ref)
            if error:
                wasmtime_error_delete(error)
                raise RuntimeError(f"hyperuuid: {name} failed inside the wasm guest")
            if trap:
                wasm_trap_delete(trap)
                ctypes.memset(trap_ref, 0, trap_size)
                raise RuntimeError(f"hyperuuid: {name} trapped inside the wasm guest")
            if result_kind is None:
                return None
            if result_kind == _I64:
                return results[0].of.i64 & 0xFFFF_FFFF_FFFF_FFFF
            return results[0].of.i32

        return call

    def call(self, name: str, *args: int) -> int | None:
        return self._call[name](*args)


_lock = threading.Lock()
_guest: _Guest | None = None


def _get() -> _Guest:
    global _guest
    if _guest is None:
        with _lock:
            if _guest is None:
                _guest = _Guest()
    return _guest


def _bind() -> None:
    """Instantiate the module now rather than on the first call, so a missing ``wasmtime`` or
    a missing ``.wasm`` surfaces at import time exactly where the native extension's own
    import failure would. The name mirrors ``_native._bind`` because ``__init__`` calls
    whichever backend it picked through the same line.
    """
    _get()


# --- the surface __init__ consumes --------------------------------------------------


def _single(name: str, error_prefix: str, *args: int) -> _uuid.UUID:
    guest = _get()
    with _lock:
        rc = guest.call(name, *args, guest._out)
        if rc == 0:
            return _make_uuid(guest._read(guest._out, 16))
    _raise_for(rc, error_prefix, name)


def _raise_for(rc: int | None, error_prefix: str, name: str) -> None:
    if rc == 2:
        raise ValueError(
            "unix_millis does not fit the 60-bit v6 timestamp field"
            if "v6" in name
            else "unix_millis must be non-negative and fit within 48 bits"
        )
    raise RuntimeError(f"{error_prefix}: random source failure")


def new_v4() -> _uuid.UUID:
    """Random version 4 UUID from the guest's WASI ``random_get``."""
    return _single("uuid_new_v4", "uuid_new_v4")


def new_v5(namespace: _uuid.UUID, name: str | bytes) -> _uuid.UUID:
    """Deterministic version 5 UUID; ``str`` names are encoded as UTF-8."""
    if isinstance(name, str):
        name = name.encode("utf-8")
    elif not isinstance(name, bytes):
        raise TypeError("name must be str or bytes")
    guest = _get()
    with _lock:
        guest._write(guest._in, namespace.int.to_bytes(16, "big"))
        name_ptr = guest._in  # a zero-length name never dereferences its pointer
        if name:
            name_ptr = guest._name_buffer(len(name))
            guest._write(name_ptr, name)
        guest.call("uuid_new_v5", guest._in, name_ptr, len(name), guest._out)
        return _make_uuid(guest._read(guest._out, 16))


def new_v6(unix_millis: int | None = None) -> _uuid.UUID:
    """Version 6 UUID at ``unix_millis`` (``None`` means now)."""
    return _single("uuid_new_v6", "uuid_new_v6", _u64(unix_millis))


def new_v7(unix_millis: int | None = None) -> _uuid.UUID:
    """Version 7 UUID at ``unix_millis`` (``None`` means now)."""
    return _single("uuid_new_v7", "uuid_new_v7", _u64(unix_millis))


def _batch_raw(name: str, count: int, unix_millis: int | None) -> bytes:
    count = int(count)
    if count < 0:
        raise OverflowError("can't convert negative int to unsigned")
    if count == 0:
        return b""
    millis = _u64(unix_millis)
    guest = _get()
    with _lock:
        out = guest._batch_buffer(count * 16)
        rc = guest.call(name, millis, count, out)
        if rc == 0:
            return guest._read(out, count * 16)
    _raise_for(rc, name, name)


def _batch_list(raw: bytes) -> list[_uuid.UUID]:
    return [_make_uuid(raw[i : i + 16]) for i in range(0, len(raw), 16)]


def new_v6_batch(count: int, unix_millis: int | None = None) -> list[_uuid.UUID]:
    """``count`` version 6 UUIDs sharing one timestamp capture."""
    return _batch_list(_batch_raw("uuid_new_v6_batch", count, unix_millis))


def new_v7_batch(count: int, unix_millis: int | None = None) -> list[_uuid.UUID]:
    """``count`` version 7 UUIDs sharing one timestamp capture and one counter block."""
    return _batch_list(_batch_raw("uuid_new_v7_batch", count, unix_millis))


def _fill_bytes(name: str, buffer: bytearray, unix_millis: int | None) -> None:
    if not isinstance(buffer, bytearray):
        raise TypeError("buffer must be a bytearray")
    length = len(buffer)
    if length % 16 != 0:
        raise ValueError("buffer length must be a multiple of 16 (one whole UUID per 16 bytes)")
    if length == 0:
        return
    # The guest cannot write into the caller's bytearray the way the extension does, so this
    # is one guest batch plus one copy out — still no uuid.UUID objects anywhere.
    buffer[:] = _batch_raw(name, length // 16, unix_millis)


def fill_v7_bytes(buffer: bytearray, unix_millis: int | None = None) -> None:
    """Fill ``buffer`` with raw version 7 UUID bytes, 16 per UUID."""
    _fill_bytes("uuid_new_v7_batch", buffer, unix_millis)


def fill_v6_bytes(buffer: bytearray, unix_millis: int | None = None) -> None:
    """Fill ``buffer`` with raw version 6 UUID bytes, 16 per UUID."""
    _fill_bytes("uuid_new_v6_batch", buffer, unix_millis)


def _unix_millis_of(name: str, uuid_value: _uuid.UUID) -> int:
    guest = _get()
    with _lock:
        guest._write(guest._in, uuid_value.int.to_bytes(16, "big"))
        return guest.call(name, guest._in)


def v6_timestamp(uuid_value: _uuid.UUID) -> datetime.datetime:
    """UTC timestamp embedded in a version 6 UUID."""
    return _millis_datetime(_unix_millis_of("uuid_v6_unix_millis", uuid_value))


def v7_timestamp(uuid_value: _uuid.UUID) -> datetime.datetime:
    """UTC timestamp embedded in a version 7 UUID."""
    return _millis_datetime(_unix_millis_of("uuid_v7_unix_millis", uuid_value))


def _reorder(name: str, uuid_value: _uuid.UUID) -> _uuid.UUID:
    guest = _get()
    with _lock:
        guest._write(guest._in, uuid_value.int.to_bytes(16, "big"))
        guest.call(name, guest._in)
        return _make_uuid(guest._read(guest._in, 16))


def v7_to_sql_order(uuid_value: _uuid.UUID) -> _uuid.UUID:
    """RFC order to SQL Server ``uniqueidentifier`` order, version 7."""
    return _reorder("uuid_v7_to_sql_order", uuid_value)


def v7_from_sql_order(uuid_value: _uuid.UUID) -> _uuid.UUID:
    """SQL Server order back to RFC order, version 7."""
    return _reorder("uuid_v7_to_rfc_order", uuid_value)


def v6_to_sql_order(uuid_value: _uuid.UUID) -> _uuid.UUID:
    """RFC order to SQL Server ``uniqueidentifier`` order, version 6."""
    return _reorder("uuid_v6_to_sql_order", uuid_value)


def v6_from_sql_order(uuid_value: _uuid.UUID) -> _uuid.UUID:
    """SQL Server order back to RFC order, version 6."""
    return _reorder("uuid_v6_to_rfc_order", uuid_value)
