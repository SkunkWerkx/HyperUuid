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
    "new_v6_batch",
    "new_v7",
    "new_v7_batch",
    "v6_timestamp",
    "v7_timestamp",
    "v6_to_sql_order",
    "v6_from_sql_order",
    "v7_to_sql_order",
    "v7_from_sql_order",
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


def new_v6_batch(count: int, unix_millis: int | None = None) -> list[_uuid.UUID]:
    """Create ``count`` time-sortable version 6 UUIDs sharing one timestamp capture — one
    native call and one random-bytes fetch instead of ``count`` of each.

    Defaults to the current time.
    """
    if unix_millis is None:
        unix_millis = int(time.time() * 1000)
    raw = _runtime.new_v6_batch(count, unix_millis)
    return [_uuid.UUID(bytes=raw[i * 16 : i * 16 + 16]) for i in range(count)]


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


def new_v7_batch(count: int, unix_millis: int | None = None) -> list[_uuid.UUID]:
    """Create ``count`` time-sortable version 7 UUIDs sharing one timestamp capture and one
    contiguous block of the monotonic counter — one native call and one random-bytes fetch
    instead of ``count`` of each.

    Defaults to the current time.
    """
    if unix_millis is None:
        unix_millis = int(time.time() * 1000)
    raw = _runtime.new_v7_batch(count, unix_millis)
    return [_uuid.UUID(bytes=raw[i * 16 : i * 16 + 16]) for i in range(count)]


def v7_to_sql_order(uuid_value: _uuid.UUID) -> _uuid.UUID:
    """Convert an RFC 9562-ordered version 7 UUID to the byte order SQL Server's
    ``uniqueidentifier`` needs on the wire to sort by creation order.

    ``System.Data.SqlTypes.SqlGuid`` comparison — and therefore T-SQL ``ORDER BY`` on a
    ``uniqueidentifier`` column — doesn't compare a GUID's 16 bytes left to right; it uses a
    fixed, non-sequential byte significance order (most significant first): octets
    ``10,11,12,13,14,15, 8,9, 6,7, 4,5, 0,1,2,3``. This moves the timestamp and counter — the
    two fields that determine creation order — into those most-significant octets, and moves
    the trailing entropy, which carries no ordering information, into the least-significant
    ones as one intact block. The permutation itself is computed once in the native Rust core
    and verified there — and independently, against the real ``System.Data.SqlTypes.SqlGuid``
    comparator — in this project's C# test suite; this binding calls the same native function
    rather than reimplementing the math.

    Meaningful only for a genuine version 7 UUID; see :func:`v6_to_sql_order` for v6.
    """
    return _uuid.UUID(bytes=_runtime.v7_to_sql_order(uuid_value.bytes))


def v7_from_sql_order(uuid_value: _uuid.UUID) -> _uuid.UUID:
    """Inverse of :func:`v7_to_sql_order` — convert a SQL-Server-ordered version 7 UUID back
    to RFC 9562 order."""
    return _uuid.UUID(bytes=_runtime.v7_to_rfc_order(uuid_value.bytes))


def v6_to_sql_order(uuid_value: _uuid.UUID) -> _uuid.UUID:
    """Convert an RFC 9562-ordered version 6 UUID to the byte order SQL Server's
    ``uniqueidentifier`` needs on the wire to sort by creation order.

    Same ``SqlGuid`` significance order as :func:`v7_to_sql_order`, applied to v6's very
    different field layout. v6 has no monotonic counter the way v7 does; the only field that
    determines its creation order is the 60-bit timestamp itself, so this moves that whole
    timestamp — most significant chunk first — into the comparison's most significant octets.
    Everything after it — ``variant``, ``clock_seq``, and ``node`` (octets 8-15, already one
    contiguous run with no ordering value of its own — ``clock_seq``/``node`` are generated
    randomly on every call, not a counter, and ``variant`` is a fixed constant either way) —
    moves as that single 8-byte span into the remaining, less significant octets, in the same
    relative order, not individually reshuffled. Version and variant end up
    at different byte offsets than :func:`v7_to_sql_order`'s result (octet 8's top nibble and
    octet 6's top two bits here, not 7/8) — fine, since the two versions are separate
    functions and a caller always knows which one it's calling.

    Unlike v7, two version 6 UUIDs minted at the same millisecond have identical timestamp
    bits — ``clock_seq``/``node`` are independently random, not a counter — so this doesn't
    (and can't) make same-millisecond v6 UUIDs sort in creation order any more than plain RFC
    order already does. Distinct timestamps sort correctly; same-timestamp ties don't, by the
    RFC's own v6 design, not a limitation introduced here.

    Meaningful only for a genuine version 6 UUID.
    """
    return _uuid.UUID(bytes=_runtime.v6_to_sql_order(uuid_value.bytes))


def v6_from_sql_order(uuid_value: _uuid.UUID) -> _uuid.UUID:
    """Inverse of :func:`v6_to_sql_order` — convert a SQL-Server-ordered version 6 UUID back
    to RFC 9562 order."""
    return _uuid.UUID(bytes=_runtime.v6_to_rfc_order(uuid_value.bytes))
