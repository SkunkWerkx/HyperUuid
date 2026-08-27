"""ctypes plumbing for the native libhyperuuid shared library.

A native call shares this process's address space directly, so there's no alloc/dealloc
dance: buffers are plain ctypes arrays passed by reference.
"""

from __future__ import annotations

import ctypes
import importlib.resources
import threading

_SO_RESOURCE = "libhyperuuid.so"

_lib: ctypes.CDLL | None = None
_lib_lock = threading.Lock()


def _load() -> ctypes.CDLL:
    resource = importlib.resources.files(__package__).joinpath(_SO_RESOURCE)
    with importlib.resources.as_file(resource) as path:
        lib = ctypes.CDLL(str(path))

    lib.uuid_new_v4.argtypes = [ctypes.c_void_p]
    lib.uuid_new_v4.restype = ctypes.c_int32
    lib.uuid_new_v5.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint32, ctypes.c_void_p]
    lib.uuid_new_v5.restype = ctypes.c_int32
    lib.uuid_new_v6.argtypes = [ctypes.c_uint64, ctypes.c_void_p]
    lib.uuid_new_v6.restype = ctypes.c_int32
    lib.uuid_v6_unix_millis.argtypes = [ctypes.c_void_p]
    lib.uuid_v6_unix_millis.restype = ctypes.c_uint64
    lib.uuid_new_v6_batch.argtypes = [ctypes.c_uint64, ctypes.c_uint32, ctypes.c_void_p]
    lib.uuid_new_v6_batch.restype = ctypes.c_int32
    lib.uuid_new_v7.argtypes = [ctypes.c_uint64, ctypes.c_void_p]
    lib.uuid_new_v7.restype = ctypes.c_int32
    lib.uuid_v7_unix_millis.argtypes = [ctypes.c_void_p]
    lib.uuid_v7_unix_millis.restype = ctypes.c_uint64
    lib.uuid_new_v7_batch.argtypes = [ctypes.c_uint64, ctypes.c_uint32, ctypes.c_void_p]
    lib.uuid_new_v7_batch.restype = ctypes.c_int32
    lib.uuid_v7_to_sql_order.argtypes = [ctypes.c_void_p]
    lib.uuid_v7_to_sql_order.restype = None
    lib.uuid_v7_to_rfc_order.argtypes = [ctypes.c_void_p]
    lib.uuid_v7_to_rfc_order.restype = None
    return lib


def _get_lib() -> ctypes.CDLL:
    global _lib
    if _lib is None:
        with _lib_lock:
            if _lib is None:
                _lib = _load()
    return _lib


def new_v4() -> bytes:
    lib = _get_lib()
    out = (ctypes.c_ubyte * 16)()
    rc = lib.uuid_new_v4(ctypes.byref(out))
    if rc != 0:
        raise RuntimeError(f"uuid_new_v4 failed with code {rc} (random source failure)")
    return bytes(out)


def new_v5(namespace_bytes: bytes, name: bytes) -> bytes:
    lib = _get_lib()
    ns = (ctypes.c_ubyte * 16).from_buffer_copy(namespace_bytes)
    out = (ctypes.c_ubyte * 16)()
    if name:
        name_buf = (ctypes.c_ubyte * len(name)).from_buffer_copy(name)
        rc = lib.uuid_new_v5(ctypes.byref(ns), ctypes.byref(name_buf), len(name), ctypes.byref(out))
    else:
        rc = lib.uuid_new_v5(ctypes.byref(ns), None, 0, ctypes.byref(out))
    if rc != 0:
        raise RuntimeError(f"uuid_new_v5 failed with code {rc}")
    return bytes(out)


def new_v6(unix_millis: int) -> bytes:
    lib = _get_lib()
    out = (ctypes.c_ubyte * 16)()
    rc = lib.uuid_new_v6(unix_millis, ctypes.byref(out))
    if rc == 2:
        raise ValueError("unix_millis does not fit the 60-bit v6 timestamp field")
    if rc != 0:
        raise RuntimeError(f"uuid_new_v6 failed with code {rc} (random source failure)")
    return bytes(out)


def v6_unix_millis(uuid_bytes: bytes) -> int:
    lib = _get_lib()
    buf = (ctypes.c_ubyte * 16).from_buffer_copy(uuid_bytes)
    return lib.uuid_v6_unix_millis(ctypes.byref(buf))


def new_v6_batch(count: int, unix_millis: int) -> bytes:
    lib = _get_lib()
    if count == 0:
        return b""
    out = (ctypes.c_ubyte * (count * 16))()
    rc = lib.uuid_new_v6_batch(unix_millis, count, ctypes.byref(out))
    if rc == 2:
        raise ValueError("unix_millis does not fit the 60-bit v6 timestamp field")
    if rc != 0:
        raise RuntimeError(f"uuid_new_v6_batch failed with code {rc} (random source failure)")
    return bytes(out)


def new_v7(unix_millis: int) -> bytes:
    lib = _get_lib()
    out = (ctypes.c_ubyte * 16)()
    rc = lib.uuid_new_v7(unix_millis, ctypes.byref(out))
    if rc == 2:
        raise ValueError("unix_millis must be non-negative and fit within 48 bits")
    if rc != 0:
        raise RuntimeError(f"uuid_new_v7 failed with code {rc} (random source failure)")
    return bytes(out)


def v7_unix_millis(uuid_bytes: bytes) -> int:
    lib = _get_lib()
    buf = (ctypes.c_ubyte * 16).from_buffer_copy(uuid_bytes)
    return lib.uuid_v7_unix_millis(ctypes.byref(buf))


def new_v7_batch(count: int, unix_millis: int) -> bytes:
    lib = _get_lib()
    if count == 0:
        return b""
    out = (ctypes.c_ubyte * (count * 16))()
    rc = lib.uuid_new_v7_batch(unix_millis, count, ctypes.byref(out))
    if rc == 2:
        raise ValueError("unix_millis must be non-negative and fit within 48 bits")
    if rc != 0:
        raise RuntimeError(f"uuid_new_v7_batch failed with code {rc} (random source failure)")
    return bytes(out)


def v7_to_sql_order(uuid_bytes: bytes) -> bytes:
    lib = _get_lib()
    buf = (ctypes.c_ubyte * 16).from_buffer_copy(uuid_bytes)
    lib.uuid_v7_to_sql_order(ctypes.byref(buf))
    return bytes(buf)


def v7_to_rfc_order(uuid_bytes: bytes) -> bytes:
    lib = _get_lib()
    buf = (ctypes.c_ubyte * 16).from_buffer_copy(uuid_bytes)
    lib.uuid_v7_to_rfc_order(ctypes.byref(buf))
    return bytes(buf)
