"""RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation,
calling directly into the native libhyperuuid shared library via ctypes — no runtime bridge.

Returns stdlib ``uuid.UUID`` objects. For v5's namespace argument, use the RFC 9562
Section 6.6 well-known namespaces already in the standard library:
``uuid.NAMESPACE_DNS``, ``NAMESPACE_URL``, ``NAMESPACE_OID``, ``NAMESPACE_X500``.

Needs a platform-specific native binary — this build ships linux-arm64 only. The same
ctypes.CDLL code also runs under Pyodide in the browser given an Emscripten-built
libhyperuuid.so "side module" (Pyodide has shipped real ctypes support since 0.18);
that additional build just isn't included here yet.
"""

from __future__ import annotations

import datetime
import time
import uuid as _uuid

from . import _runtime

__all__ = [
    "new_v4",
    "new_v5",
    "new_v6",
    "new_v7",
    "v6_timestamp",
    "v7_timestamp",
    "NIL",
    "MAX",
]

#: The RFC 9562 §5.9 Nil UUID — all 128 bits zero.
NIL = _uuid.UUID(bytes=bytes(16))

#: The RFC 9562 §5.10 Max UUID — all 128 bits one.
MAX = _uuid.UUID(bytes=b"\xff" * 16)


def new_v4() -> _uuid.UUID:
    """Create a random UUID version 4 (RFC 9562 §5.4)."""
    return _uuid.UUID(bytes=_runtime.new_v4())


def new_v5(namespace: _uuid.UUID, name: str | bytes) -> _uuid.UUID:
    """Create a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a name.

    The same ``(namespace, name)`` pair always produces the same UUID. ``name`` may
    be ``str`` (encoded as UTF-8) or raw ``bytes``.
    """
    name_bytes = name.encode("utf-8") if isinstance(name, str) else bytes(name)
    return _uuid.UUID(bytes=_runtime.new_v5(namespace.bytes, name_bytes))


def new_v6(unix_millis: int | None = None) -> _uuid.UUID:
    """Create a time-sortable UUID version 6 (RFC 9562 §5.6), a field-compatible reordering
    of version 1 for better sort/index locality.

    Defaults to the current time; pass an explicit Unix-epoch millisecond timestamp to embed
    a specific time instead. ``clock_seq`` and ``node`` are randomly generated on every call
    — unlike version 7, there is no monotonic counter, so calls within the same millisecond
    are not guaranteed to sort in creation order.
    """
    if unix_millis is None:
        unix_millis = int(time.time() * 1000)
    return _uuid.UUID(bytes=_runtime.new_v6(unix_millis))


def v6_timestamp(uuid_value: _uuid.UUID) -> datetime.datetime:
    """Recover the UTC timestamp embedded in a version 6 UUID's timestamp field.

    Only meaningful when ``uuid_value.version == 6`` — the RFC 9562 bit layout doesn't
    distinguish "not a v6 UUID" from "v6 UUID with a very early timestamp", so the caller is
    responsible for checking ``version`` first if that matters. Unlike :func:`v7_timestamp`,
    this can't raise ``OverflowError``: v6's 60-bit tick count, offset from the 1582 UUID
    epoch rather than 1970, tops out around the year 5236 — well short of ``datetime``'s own
    year-9999 ceiling.
    """
    millis = _runtime.v6_unix_millis(uuid_value.bytes)
    epoch = datetime.datetime(1970, 1, 1, tzinfo=datetime.timezone.utc)
    return epoch + datetime.timedelta(milliseconds=millis)


def new_v7(unix_millis: int | None = None) -> _uuid.UUID:
    """Create a time-sortable UUID version 7 (RFC 9562 §6.2).

    Defaults to the current time; pass an explicit Unix-epoch millisecond timestamp
    (non-negative, fitting in 48 bits) to embed a specific time instead.
    """
    if unix_millis is None:
        unix_millis = int(time.time() * 1000)
    return _uuid.UUID(bytes=_runtime.new_v7(unix_millis))


def v7_timestamp(uuid_value: _uuid.UUID) -> datetime.datetime:
    """Recover the UTC timestamp embedded in a version 7 UUID's ``unix_ts_ms`` field.

    Only meaningful when ``uuid_value.version == 7`` — the RFC 9562 bit layout doesn't
    distinguish "not a v7 UUID" from "v7 UUID with a very early timestamp", so the caller is
    responsible for checking ``version`` first if that matters.

    Raises ``OverflowError`` for a (spec-valid) embedded timestamp past year 9999 — the RFC's
    48-bit millisecond field holds values up to the year 10889, but ``datetime.datetime``
    cannot represent a year beyond 9999. Built from ``timedelta`` arithmetic on the epoch
    rather than ``fromtimestamp()``, which delegates to the platform C library and — on
    Windows specifically — raises ``OSError`` well before year 9999 rather than reaching
    datetime's own year-9999 ceiling (confirmed on a real windows-11-arm CI runner).
    """
    millis = _runtime.v7_unix_millis(uuid_value.bytes)
    epoch = datetime.datetime(1970, 1, 1, tzinfo=datetime.timezone.utc)
    return epoch + datetime.timedelta(milliseconds=millis)
