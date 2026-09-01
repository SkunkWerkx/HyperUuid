"""RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation. A
PyO3 extension (``hyperuuid._native``) links the Rust core directly into this CPython
extension module — no ctypes marshalling, no runtime bridge.

Returns stdlib ``uuid.UUID`` objects. For v5's namespace argument, use the RFC 9562
Section 6.6 well-known namespaces already in the standard library:
``uuid.NAMESPACE_DNS``, ``NAMESPACE_URL``, ``NAMESPACE_OID``, ``NAMESPACE_X500``.

Ships as real platform-specific wheels (linux/macOS/Windows, x64/arm64) built by
``maturin`` — no compiler needed to install.
"""

from __future__ import annotations

import datetime
import uuid as _uuid

from . import _native

_native._bind()

__all__ = [
    "new_v4",
    "new_v5",
    "new_v6",
    "new_v6_batch",
    "new_v7",
    "new_v7_batch",
    "fill_v6",
    "fill_v7",
    "v6_timestamp",
    "v7_timestamp",
    "get_timestamp",
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


def _unix_millis_from(value: int | datetime.datetime | None) -> int | None:
    """Convert ``value`` to a Unix-epoch millisecond int: ``None`` passes through (``_native``
    itself defaults that to the current time), a ``datetime.datetime`` is converted exactly
    via ``timestamp()`` (no float rounding — multiplied and rounded in one step), and an
    ``int`` (a raw millisecond count) passes through unchanged. Shared by every
    ``new_v6``/``new_v7``/batch door below so a caller can pass either a ``datetime.datetime``
    or a raw millisecond count interchangeably.
    """
    if isinstance(value, datetime.datetime):
        return round(value.timestamp() * 1000)
    return value


def new_v4() -> _uuid.UUID:
    """Create a random UUID version 4 (RFC 9562 §5.4)."""
    return _native.new_v4()


def new_v5(namespace: _uuid.UUID, name: str | bytes) -> _uuid.UUID:
    """Create a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a name.

    The same ``(namespace, name)`` pair always produces the same UUID. ``name`` may
    be ``str`` (encoded as UTF-8) or raw ``bytes``.
    """
    return _native.new_v5(namespace, name)


def new_v6(unix_millis: int | datetime.datetime | None = None) -> _uuid.UUID:
    """Create a time-sortable UUID version 6 (RFC 9562 §5.6), a field-compatible reordering
    of version 1 for better sort/index locality.

    Defaults to the current time; pass an explicit ``datetime.datetime`` or Unix-epoch
    millisecond timestamp to embed a specific time instead. ``clock_seq`` and ``node`` are
    randomly generated on every call — unlike version 7, there is no monotonic counter, so
    calls within the same millisecond are not guaranteed to sort in creation order.
    """
    return _native.new_v6(_unix_millis_from(unix_millis))


def v6_timestamp(uuid_value: _uuid.UUID) -> datetime.datetime:
    """Recover the UTC timestamp embedded in a version 6 UUID's timestamp field.

    Only meaningful when ``uuid_value.version == 6`` — the RFC 9562 bit layout doesn't
    distinguish "not a v6 UUID" from "v6 UUID with a very early timestamp", so the caller is
    responsible for checking ``version`` first if that matters. Unlike :func:`v7_timestamp`,
    this can't raise ``OverflowError``: v6's 60-bit tick count, offset from the 1582 UUID
    epoch rather than 1970, tops out around the year 5236 — well short of ``datetime``'s own
    year-9999 ceiling.
    """
    return _native.v6_timestamp(uuid_value)


def new_v6_batch(count: int, unix_millis: int | datetime.datetime | None = None) -> list[_uuid.UUID]:
    """Create ``count`` time-sortable version 6 UUIDs sharing one timestamp capture — one
    native call and one random-bytes fetch instead of ``count`` of each.

    Defaults to the current time; pass an explicit ``datetime.datetime`` or Unix-epoch
    millisecond timestamp to embed a specific time instead.
    """
    return _native.new_v6_batch(count, _unix_millis_from(unix_millis))


def new_v7(unix_millis: int | datetime.datetime | None = None) -> _uuid.UUID:
    """Create a time-sortable UUID version 7 (RFC 9562 §6.2).

    Defaults to the current time; pass an explicit ``datetime.datetime`` or Unix-epoch
    millisecond timestamp (non-negative, fitting in 48 bits) to embed a specific time instead.
    """
    return _native.new_v7(_unix_millis_from(unix_millis))


def v7_timestamp(uuid_value: _uuid.UUID) -> datetime.datetime:
    """Recover the UTC timestamp embedded in a version 7 UUID's ``unix_ts_ms`` field.

    Only meaningful when ``uuid_value.version == 7`` — the RFC 9562 bit layout doesn't
    distinguish "not a v7 UUID" from "v7 UUID with a very early timestamp", so the caller is
    responsible for checking ``version`` first if that matters.

    Raises ``OverflowError`` for a (spec-valid) embedded timestamp past year 9999 — the RFC's
    48-bit millisecond field holds values up to the year 10889, but ``datetime.datetime``
    cannot represent a year beyond 9999.
    """
    return _native.v7_timestamp(uuid_value)


def get_timestamp(uuid_value: _uuid.UUID) -> datetime.datetime | None:
    """Recover the UTC timestamp embedded in ``uuid_value``, or ``None`` if it isn't a
    version 6 or 7 UUID.

    Unlike :func:`v6_timestamp`/:func:`v7_timestamp`, this checks ``uuid_value.version``
    itself first, so a caller doesn't need to already know (or separately check) which
    version ``uuid_value`` is before asking — delegates straight to whichever of those two
    functions applies, no bit-layout logic duplicated here.
    """
    if uuid_value.version == 6:
        return v6_timestamp(uuid_value)
    if uuid_value.version == 7:
        return v7_timestamp(uuid_value)
    return None


def new_v7_batch(count: int, unix_millis: int | datetime.datetime | None = None) -> list[_uuid.UUID]:
    """Create ``count`` time-sortable version 7 UUIDs sharing one timestamp capture and one
    contiguous block of the monotonic counter — one native call and one random-bytes fetch
    instead of ``count`` of each.

    Defaults to the current time; pass an explicit ``datetime.datetime`` or Unix-epoch
    millisecond timestamp to embed a specific time instead.
    """
    return _native.new_v7_batch(count, _unix_millis_from(unix_millis))


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
    return _native.v7_to_sql_order(uuid_value)


def v7_from_sql_order(uuid_value: _uuid.UUID) -> _uuid.UUID:
    """Inverse of :func:`v7_to_sql_order` — convert a SQL-Server-ordered version 7 UUID back
    to RFC 9562 order."""
    return _native.v7_from_sql_order(uuid_value)


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
    return _native.v6_to_sql_order(uuid_value)


def v6_from_sql_order(uuid_value: _uuid.UUID) -> _uuid.UUID:
    """Inverse of :func:`v6_to_sql_order` — convert a SQL-Server-ordered version 6 UUID back
    to RFC 9562 order."""
    return _native.v6_from_sql_order(uuid_value)


def fill_v7(buffer: bytearray, unix_millis: int | datetime.datetime | None = None) -> None:
    """Fill ``buffer`` with raw RFC 9562-ordered version 7 UUID bytes, 16 per UUID.

    One native call writes straight into the ``bytearray`` you pass, sharing one timestamp
    capture and one contiguous block of the monotonic counter across the whole batch. No
    ``uuid.UUID`` objects are created at any point, which is the entire reason this exists.

    **Use this when bytes are what you actually want** — a database parameter, a wire format,
    a bulk ``COPY``. It is roughly **32x faster** than :func:`new_v7_batch` for 1000 UUIDs
    (about 20 µs versus 640 µs), because :func:`new_v7_batch` spends nearly all its time
    building a thousand ``uuid.UUID`` instances rather than in the native call.

    **Do not use it if you need ``uuid.UUID`` objects.** Filling bytes and then constructing
    UUIDs from them in Python is about *twice as slow* as calling :func:`new_v7_batch`, which
    builds them through a much faster path inside the extension. Reach for this only when the
    bytes are the destination, not a step on the way to objects.

    ``len(buffer)`` must be a multiple of 16 — one whole UUID per 16 bytes. A zero-length
    buffer is a no-op. Defaults to the current time; pass a ``datetime.datetime`` or a
    Unix-epoch millisecond timestamp to embed a specific time instead.

    ``bytearray`` specifically, not ``memoryview`` or NumPy arrays: the general writable
    buffer protocol needs ``Py_buffer``, which only entered CPython's stable ABI in 3.11, and
    this extension is built ``abi3-py39`` so a single wheel serves every supported Python.

    :raises ValueError: if ``len(buffer)`` is not a multiple of 16, or ``unix_millis`` does
        not fit the 48-bit ``unix_ts_ms`` field.
    """
    _native.fill_v7_bytes(buffer, _unix_millis_from(unix_millis))


def fill_v6(buffer: bytearray, unix_millis: int | datetime.datetime | None = None) -> None:
    """Fill ``buffer`` with raw RFC 9562-ordered version 6 UUID bytes, 16 per UUID.

    The version 6 counterpart to :func:`fill_v7`, with the same performance characteristics
    and the same guidance about when it is and isn't the right call — see that function.

    ``clock_seq`` and ``node`` are independently random per item; unlike version 7 there is no
    monotonic counter, so items minted within the same millisecond are not guaranteed to sort
    in creation order.

    :raises ValueError: if ``len(buffer)`` is not a multiple of 16, or ``unix_millis`` does
        not fit the 60-bit v6 timestamp field.
    """
    _native.fill_v6_bytes(buffer, _unix_millis_from(unix_millis))
