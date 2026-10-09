"""RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation. A
PyO3 extension (``hyperuuid._native``) links the Rust core directly into this CPython
extension module — no ctypes marshalling, no runtime bridge.

Returns stdlib ``uuid.UUID`` objects. For v5's namespace argument, use the RFC 9562
Section 6.6 well-known namespaces already in the standard library:
``uuid.NAMESPACE_DNS``, ``NAMESPACE_URL``, ``NAMESPACE_OID``, ``NAMESPACE_X500``.

Ships as real platform-specific abi3 wheels (linux glibc and musl, macOS, Windows; x64 and
arm64, plus Pyodide's ``pyemscripten`` wasm32 for the browser) built by ``maturin`` — no
compiler needed to install. The package is typed:
``py.typed`` ships beside it, with a stub for the extension module.
"""

from __future__ import annotations

import datetime
import enum
import uuid as _uuid

from operator import index as _index

from . import _native

#: Which backend this process loaded: always ``"native"``, the PyO3 extension that links the
#: Rust core straight into CPython — the only backend, and the one every published wheel ships.
BACKEND: str = "native"

_native._bind()

__all__ = [
    "BACKEND",
    "Layout",
    "Variant",
    "MAX_V7_BATCH",
    "native_version",
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
    "v6_unix_millis",
    "v7_unix_millis",
    "get_timestamp",
    "version",
    "variant",
    "is_rfc",
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

#: The most UUIDs one version 7 batch (:func:`new_v7_batch`, :func:`fill_v7`) mints:
#: 67,108,864, the size of the 26-bit counter that orders UUIDs within a millisecond. A batch
#: any larger would have to wrap that counter twice and could not stay in order, so it is
#: refused with ``ValueError`` before anything is allocated or written. Version 6 has no
#: counter and no such limit.
#:
#: The roll-forward over the counter wrap (see :func:`new_v7_batch`) orders one batch, not the
#: stream. The next batch or :func:`new_v7` call in the same real millisecond starts its
#: counter just past the wrap and carries the supplied timestamp, so it sorts before the
#: previous batch's tail, which was stamped a millisecond later; two single :func:`new_v7`
#: calls either side of the wrap in one millisecond sort in reverse the same way. It happens
#: at most once per ``MAX_V7_BATCH`` UUIDs the process mints.
MAX_V7_BATCH: int = _native.MAX_V7_BATCH


class Layout(enum.IntEnum):
    """The byte order a UUID's 16 bytes are held in, for the inspection and timestamp functions
    that take one. The values are the native core's layout codes.

    There is no "unspecified" member: every function that takes a layout defaults it to
    :attr:`RFC9562`, the order every other function in this package takes and returns. Anything
    that is not one of these two — an unknown code, or a value that is not an integer at all —
    is refused (``ValueError``, ``TypeError``) rather than guessed at.
    """

    #: RFC 9562 network order: a ``uuid.UUID`` as the standard library and every other
    #: function here hold it.
    RFC9562 = 1
    #: The order :func:`v7_to_sql_order` and :func:`v6_to_sql_order` return, which SQL Server's
    #: ``uniqueidentifier`` sorts by creation order. Defined for versions 6 and 7 only.
    SQL_SERVER = 2


class Variant(enum.IntEnum):
    """The variant field of a UUID (RFC 9562 §4.1), which says how the rest of its bits are laid
    out; see :func:`variant`. Only :attr:`RFC9562` has versions. The values are the native
    core's variant codes.
    """

    #: ``0xxx``: reserved, Network Computing System backward compatibility. Includes Nil.
    NCS = 1
    #: ``10xx``: the variant RFC 9562 (and RFC 4122 before it) specifies.
    RFC9562 = 2
    #: ``110x``: reserved, Microsoft Corporation backward compatibility.
    MICROSOFT = 3
    #: ``111x``: reserved for future definition. Includes Max.
    FUTURE = 4


# Indexed by core code - 1, cheaper than the Variant(code) lookup.
_VARIANTS = (Variant.NCS, Variant.RFC9562, Variant.MICROSOFT, Variant.FUTURE)


def _layout_code(layout: Layout | int) -> int:
    """Validate ``layout`` before the extension sees it and return its core code: a
    :class:`Layout` member, or a plain ``int`` equal to one (``Layout`` is an ``IntEnum``).
    Anything that is not an integer is a ``TypeError``, an integer that is not a layout code a
    ``ValueError`` — the same split the rest of the module draws, and the one ``Layout(3)``
    itself makes.
    """
    if layout.__class__ is Layout:
        return int(layout)
    try:
        code = _index(layout)
    except TypeError:
        raise TypeError(f"layout must be a hyperuuid.Layout, not {type(layout).__name__}") from None
    if code != 1 and code != 2:
        raise ValueError(f"layout must be Layout.RFC9562 or Layout.SQL_SERVER, not {code!r}")
    return code


_EPOCH = datetime.datetime(1970, 1, 1, tzinfo=datetime.timezone.utc)
_MILLISECOND = datetime.timedelta(milliseconds=1)
_U64_MAX = (1 << 64) - 1
_U32_MAX = (1 << 32) - 1
# The core's own words for a timestamp its field cannot hold. A value that does not even fit
# the u64 the core takes is the same caller bug, so it raises the same error from here.
_V6_OUT_OF_RANGE = "unix_millis does not fit the 60-bit v6 timestamp field"
_V7_OUT_OF_RANGE = "unix_millis must be non-negative and fit within 48 bits"


def _unix_millis_from(value: int | datetime.datetime | None, out_of_range: str) -> int | None:
    """Convert ``value`` to a Unix-epoch millisecond int: ``None`` passes through (the extension
    defaults that to the current time), a ``datetime.datetime`` is converted in exact integer
    arithmetic and truncated to the millisecond — no float in between, so a time 0.9 ms into a
    millisecond stays in it rather than rounding into the next — and an ``int`` (a raw
    millisecond count) passes through unchanged. A naive ``datetime`` is read as local time,
    exactly as ``datetime.timestamp()`` reads it. Shared by every ``new_v6``/``new_v7``/batch
    door below so a caller can pass either a ``datetime.datetime`` or a raw millisecond count
    interchangeably.

    Validated here, before the extension sees it: anything that is not an integer or a
    ``datetime`` is a ``TypeError``, and an integer the core's unsigned 64-bit parameter
    cannot carry is the ``ValueError`` (``out_of_range``) the core itself raises for a
    timestamp past its field — one exception per caller bug.
    """
    if value is None:
        return None
    if isinstance(value, datetime.datetime):
        if value.utcoffset() is None:
            value = value.astimezone()
        value = (value - _EPOCH) // _MILLISECOND
    else:
        try:
            value = _index(value)
        except TypeError:
            raise TypeError(
                f"unix_millis must be an int, a datetime.datetime or None, not {type(value).__name__}"
            ) from None
    if not 0 <= value <= _U64_MAX:
        raise ValueError(out_of_range)
    return value


def _count_from(count: int) -> int:
    """Validate a batch ``count`` before the extension sees it: an integer from 0 to 4294967295,
    the range of the core's own unsigned 32-bit batch parameter. Anything else is refused
    here rather than narrowed into a batch of some other size.
    """
    try:
        count = _index(count)
    except TypeError:
        raise TypeError(f"count must be an int, not {type(count).__name__}") from None
    if not 0 <= count <= _U32_MAX:
        raise ValueError("count must be between 0 and 4294967295")
    return count


def native_version() -> str:
    """Return the version of the Rust core this process actually loaded, as
    ``"major.minor.patch"`` — decoded from the same packed ``hyperuuid_version`` export
    every other binding probes.
    """
    return _native.native_version()


def new_v4() -> _uuid.UUID:
    """Create a random UUID version 4 (RFC 9562 §5.4)."""
    return _native.new_v4()


def new_v5(namespace: _uuid.UUID, name: str | bytes) -> _uuid.UUID:
    """Create a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a name.

    The same ``(namespace, name)`` pair always produces the same UUID. ``name`` may
    be ``str`` (encoded as UTF-8) or raw ``bytes``.

    :raises TypeError: if ``namespace`` is not a ``uuid.UUID``, or ``name`` is neither
        ``str`` nor ``bytes``.
    """
    return _native.new_v5(namespace, name)


def new_v6(unix_millis: int | datetime.datetime | None = None) -> _uuid.UUID:
    """Create a time-sortable UUID version 6 (RFC 9562 §5.6), a field-compatible reordering
    of version 1 for better sort/index locality.

    Defaults to the current time; pass an explicit ``datetime.datetime`` or Unix-epoch
    millisecond timestamp to embed a specific time instead. ``clock_seq`` and ``node`` are
    randomly generated on every call — unlike version 7, there is no monotonic counter, so
    calls within the same millisecond are not guaranteed to sort in creation order.

    :raises TypeError: if ``unix_millis`` is not an int, a ``datetime.datetime`` or ``None``.
    :raises ValueError: if ``unix_millis`` is negative or does not fit the 60-bit v6
        timestamp field.
    """
    # The common arguments — nothing, or a plain int in range — need none of the conversion
    # below, and calling it costs more than a third of the whole mint.
    if unix_millis is None or (type(unix_millis) is int and 0 <= unix_millis <= _U64_MAX):
        return _native.new_v6(unix_millis)
    return _native.new_v6(_unix_millis_from(unix_millis, _V6_OUT_OF_RANGE))


def v6_timestamp(uuid_value: _uuid.UUID, layout: Layout = Layout.RFC9562) -> datetime.datetime:
    """Recover the UTC timestamp embedded in a version 6 UUID's timestamp field.

    ``layout`` is the byte order ``uuid_value`` is held in: :attr:`Layout.SQL_SERVER` reads a
    value straight from :func:`v6_to_sql_order` (or a ``uniqueidentifier`` column) in place,
    with no conversion back first.

    Only meaningful for a genuine version 6 UUID in that layout — the bit layout doesn't
    distinguish "not a v6 UUID" from "v6 UUID with a very early timestamp", so the caller is
    responsible for checking first if that matters (:func:`is_rfc`, or :func:`get_timestamp`,
    which checks for you). Unlike :func:`v7_timestamp`, this can't raise ``OverflowError``:
    v6's 60-bit tick count, offset from the 1582 UUID epoch rather than 1970, tops out around
    the year 5236 — well short of ``datetime``'s own year-9999 ceiling.

    :raises TypeError: if ``layout`` is not an integer.
    :raises ValueError: if ``layout`` is not a :class:`Layout`.
    """
    if layout is Layout.RFC9562:
        return _native.v6_timestamp(uuid_value)
    return _native.v6_timestamp(uuid_value, _layout_code(layout))


def new_v6_batch(
    count: int, unix_millis: int | datetime.datetime | None = None
) -> list[_uuid.UUID]:
    """Create ``count`` time-sortable version 6 UUIDs sharing one timestamp capture — one
    native call and one random-bytes fetch instead of ``count`` of each.

    Defaults to the current time; pass an explicit ``datetime.datetime`` or Unix-epoch
    millisecond timestamp to embed a specific time instead.

    :raises TypeError: if ``count`` is not an int, or ``unix_millis`` is not an int, a
        ``datetime.datetime`` or ``None``.
    :raises ValueError: if ``count`` is outside 0 to 4294967295, or ``unix_millis`` is
        negative or does not fit the 60-bit v6 timestamp field.
    :raises MemoryError: if a batch of ``count`` UUIDs cannot be allocated.
    """
    return _native.new_v6_batch(
        _count_from(count), _unix_millis_from(unix_millis, _V6_OUT_OF_RANGE)
    )


def new_v7(unix_millis: int | datetime.datetime | None = None) -> _uuid.UUID:
    """Create a time-sortable UUID version 7 (RFC 9562 §6.2).

    Defaults to the current time; pass an explicit ``datetime.datetime`` or Unix-epoch
    millisecond timestamp (non-negative, fitting in 48 bits) to embed a specific time instead.

    :raises TypeError: if ``unix_millis`` is not an int, a ``datetime.datetime`` or ``None``.
    :raises ValueError: if ``unix_millis`` is negative or does not fit the 48-bit
        ``unix_ts_ms`` field.
    """
    # See new_v6: the common arguments skip the conversion.
    if unix_millis is None or (type(unix_millis) is int and 0 <= unix_millis <= _U64_MAX):
        return _native.new_v7(unix_millis)
    return _native.new_v7(_unix_millis_from(unix_millis, _V7_OUT_OF_RANGE))


def v7_timestamp(uuid_value: _uuid.UUID, layout: Layout = Layout.RFC9562) -> datetime.datetime:
    """Recover the UTC timestamp embedded in a version 7 UUID's ``unix_ts_ms`` field.

    ``layout`` is the byte order ``uuid_value`` is held in: :attr:`Layout.SQL_SERVER` reads a
    value straight from :func:`v7_to_sql_order` (or a ``uniqueidentifier`` column) in place,
    with no conversion back first.

    Only meaningful for a genuine version 7 UUID in that layout — the bit layout doesn't
    distinguish "not a v7 UUID" from "v7 UUID with a very early timestamp", so the caller is
    responsible for checking first if that matters (:func:`is_rfc`, or :func:`get_timestamp`,
    which checks for you).

    :raises OverflowError: for a (spec-valid) embedded timestamp past year 9999 — the RFC's
        48-bit millisecond field holds values up to the year 10889, but ``datetime.datetime``
        cannot represent a year beyond 9999. :func:`v7_unix_millis` reads those.
    :raises TypeError: if ``layout`` is not an integer.
    :raises ValueError: if ``layout`` is not a :class:`Layout`.
    """
    if layout is Layout.RFC9562:
        return _native.v7_timestamp(uuid_value)
    return _native.v7_timestamp(uuid_value, _layout_code(layout))


def v6_unix_millis(uuid_value: _uuid.UUID, layout: Layout = Layout.RFC9562) -> int:
    """The timestamp embedded in a version 6 UUID as Unix-epoch milliseconds — the integer
    :func:`v6_timestamp` builds its ``datetime`` from, without building it. For a value that
    is going to be stored, compared or forwarded as a number, this is the cheaper call.

    Only meaningful for a genuine version 6 UUID in ``layout``, exactly as
    :func:`v6_timestamp`, which takes ``layout`` the same way.

    :raises TypeError: if ``layout`` is not an integer.
    :raises ValueError: if ``layout`` is not a :class:`Layout`.
    """
    if layout is Layout.RFC9562:
        return _native.v6_unix_millis(uuid_value)
    return _native.v6_unix_millis_in(uuid_value, _layout_code(layout))


def v7_unix_millis(uuid_value: _uuid.UUID, layout: Layout = Layout.RFC9562) -> int:
    """The timestamp embedded in a version 7 UUID as Unix-epoch milliseconds — the integer
    :func:`v7_timestamp` builds its ``datetime`` from, without building it. For a value that
    is going to be stored, compared or forwarded as a number, this is the cheaper call, and
    unlike :func:`v7_timestamp` it cannot raise for the timestamp: the whole 48-bit field
    fits an ``int``, past the year 9999 included.

    Only meaningful for a genuine version 7 UUID in ``layout``, exactly as
    :func:`v7_timestamp`, which takes ``layout`` the same way.

    :raises TypeError: if ``layout`` is not an integer.
    :raises ValueError: if ``layout`` is not a :class:`Layout`.
    """
    if layout is Layout.RFC9562:
        return _native.v7_unix_millis(uuid_value)
    return _native.v7_unix_millis_in(uuid_value, _layout_code(layout))


def get_timestamp(
    uuid_value: _uuid.UUID, layout: Layout = Layout.RFC9562
) -> datetime.datetime | None:
    """Recover the UTC timestamp embedded in ``uuid_value``, or ``None`` if it isn't an
    RFC 9562 version 6 or 7 UUID in ``layout``.

    Unlike :func:`v6_timestamp`/:func:`v7_timestamp`, this checks the value itself first, so a
    caller doesn't need to already know which version ``uuid_value`` is before asking. The
    variant is part of the check: a 6 or 7 in the version nibble under a variant that isn't
    RFC 9562's (as stdlib's ``uuid_value.version`` reads it, ``None``) carries no timestamp.
    The check and the read are one call into the native core. In :attr:`Layout.SQL_SERVER`
    the value is read in place; which layout ``uuid_value`` is in is the caller's to know
    (see :func:`version`).

    :raises OverflowError: for a version 7 timestamp past year 9999, as :func:`v7_timestamp`.
    :raises TypeError: if ``layout`` is not an integer.
    :raises ValueError: if ``layout`` is not a :class:`Layout`.
    """
    return _native.get_timestamp(uuid_value, _layout_code(layout))


def version(uuid_value: _uuid.UUID, layout: Layout = Layout.RFC9562) -> int:
    """The version of ``uuid_value``, held in ``layout``'s byte order.

    In :attr:`Layout.RFC9562` this is the version nibble, 0 through 15 — Nil reads as 0 and Max
    as 15 — and says nothing about the variant (stdlib's ``uuid_value.version`` is ``None``
    for a non-RFC variant; this is not). Use :func:`is_rfc` when the answer has to mean "an
    RFC 9562 UUID of version N".

    With the default layout, this reads ``uuid_value`` as RFC 9562 order. A SQL-ordered value —
    from :func:`v7_to_sql_order`/:func:`v6_to_sql_order`, or read back from a
    ``uniqueidentifier`` column — needs ``layout=Layout.SQL_SERVER``, because the RFC-order read
    looks at the wrong bytes there. There is deliberately no layout-agnostic check: the caller
    holding the value knows its order.

    In :attr:`Layout.SQL_SERVER` only versions 6 and 7 have an order, so this is 6 or 7 for
    bytes that form a SQL-ordered version 6 or 7 RFC 9562 UUID, and 0 for bytes that don't.
    The two versions put their version nibble at different octets, and the native core checks
    the variant where each puts it too, so a SQL-ordered v6 never reads as a v7 or the other
    way round. The bytes alone cannot say which layout a value is in, so the caller must keep
    track of that: an RFC-ordered value read as :attr:`Layout.SQL_SERVER` can happen to form a
    valid SQL-ordered v7 (a random v4 does one time in 16).

    :raises TypeError: if ``uuid_value`` is not a ``uuid.UUID``, or ``layout`` is not an
        integer.
    :raises ValueError: if ``layout`` is not a :class:`Layout`.
    """
    return _native.version(uuid_value, _layout_code(layout))


def variant(uuid_value: _uuid.UUID) -> Variant:
    """The variant field of ``uuid_value`` (RFC 9562 §4.1), read in RFC 9562 order. Nil reads
    as :attr:`Variant.NCS` and Max as :attr:`Variant.FUTURE`, which is how the RFC classifies
    them.

    RFC 9562 order only, and there is no layout form: in SQL Server order the variant sits at a
    different byte for each version. Don't pass it a SQL-ordered value or a
    ``uniqueidentifier`` read-back. To validate one of those, use
    ``is_rfc(uuid_value, version, Layout.SQL_SERVER)``, which checks the variant where that
    version puts it.

    :raises TypeError: if ``uuid_value`` is not a ``uuid.UUID``.
    """
    return _VARIANTS[_native.variant(uuid_value) - 1]


def is_rfc(uuid_value: _uuid.UUID, version: int, layout: Layout = Layout.RFC9562) -> bool:
    """Whether ``uuid_value``, held in ``layout``'s byte order, is an RFC 9562 UUID of version
    ``version``: the RFC variant and that version, in one call. The guard to run before
    trusting a value's version-specific fields, such as a version 7's timestamp.

    With the default layout, this reads ``uuid_value`` as RFC 9562 order. A SQL-ordered value —
    from :func:`v7_to_sql_order`/:func:`v6_to_sql_order`, or read back from a
    ``uniqueidentifier`` column — needs ``layout=Layout.SQL_SERVER``, because the RFC-order read
    looks at the wrong bytes there: ``is_rfc(sql_ordered, 7)`` is not the check. There is
    deliberately no layout-agnostic check: the caller holding the value knows its order.

    In :attr:`Layout.SQL_SERVER` only versions 6 and 7 can be true; see :func:`version`. A
    ``version`` outside 0 to 15 is simply ``False``.

    :raises TypeError: if ``uuid_value`` is not a ``uuid.UUID``, or ``version`` or ``layout``
        is not an integer.
    :raises ValueError: if ``layout`` is not a :class:`Layout`.
    """
    code = _layout_code(layout)
    try:
        version = _index(version)
    except TypeError:
        raise TypeError(f"version must be an int, not {type(version).__name__}") from None
    if not 0 <= version <= 15:
        # Still a uuid.UUID check, so a wrong-typed value is the same TypeError either way.
        _native.version(uuid_value, code)
        return False
    return _native.is_rfc(uuid_value, version, code)


def new_v7_batch(
    count: int, unix_millis: int | datetime.datetime | None = None
) -> list[_uuid.UUID]:
    """Create ``count`` time-sortable version 7 UUIDs sharing one timestamp capture and one
    contiguous block of the monotonic counter — one native call and one random-bytes fetch
    instead of ``count`` of each.

    Defaults to the current time; pass an explicit ``datetime.datetime`` or Unix-epoch
    millisecond timestamp to embed a specific time instead.

    The batch is always in strictly increasing order. The counter is one process-wide sequence
    that wraps every 2**26 values, so a batch can straddle the wrap; the UUIDs from the wrap on
    carry ``unix_millis + 1`` rather than sorting before the ones ahead of them, so an embedded
    timestamp can be one millisecond past the one supplied, never more. That is also why a
    batch holds at most :data:`MAX_V7_BATCH` UUIDs.

    The roll-forward orders one batch, not the stream. The next batch or :func:`new_v7` call
    in the same real millisecond starts its counter just past the wrap and carries the supplied
    timestamp, so it sorts before the previous batch's tail, which was stamped a millisecond
    later; two single :func:`new_v7` calls either side of the wrap in one millisecond sort in
    reverse the same way. It happens at most once per :data:`MAX_V7_BATCH` UUIDs the process
    mints.

    :raises TypeError: if ``count`` is not an int, or ``unix_millis`` is not an int, a
        ``datetime.datetime`` or ``None``.
    :raises ValueError: if ``count`` is outside 0 to :data:`MAX_V7_BATCH` (checked before
        anything is allocated), or ``unix_millis`` is negative or does not fit the 48-bit
        ``unix_ts_ms`` field — including a batch that would roll forward past it.
    :raises MemoryError: if a batch of ``count`` UUIDs cannot be allocated.
    """
    return _native.new_v7_batch(
        _count_from(count), _unix_millis_from(unix_millis, _V7_OUT_OF_RANGE)
    )


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
    a bulk ``COPY``. It is roughly **15x faster** than :func:`new_v7_batch` for 1000 UUIDs
    (about 10 µs versus 140 µs), because :func:`new_v7_batch` spends nearly all its time
    building a thousand ``uuid.UUID`` instances rather than in the native call.

    **Do not use it if you need ``uuid.UUID`` objects.** Filling bytes and then constructing
    UUIDs from them in Python is about *twice as slow* as calling :func:`new_v7_batch`, which
    builds them through a much faster path inside the extension. Reach for this only when the
    bytes are the destination, not a step on the way to objects.

    ``len(buffer)`` must be a multiple of 16 — one whole UUID per 16 bytes — and hold at most
    :data:`MAX_V7_BATCH` UUIDs; a larger buffer is refused before a byte of it is written. A
    zero-length buffer writes nothing (the timestamp is still checked, as it is for a batch of
    zero). Defaults to the current time; pass a ``datetime.datetime`` or a Unix-epoch
    millisecond timestamp to embed a specific time instead. The UUIDs are in strictly
    increasing order, with the same possible one-millisecond roll-forward as
    :func:`new_v7_batch`, which likewise orders this one fill, not the stream: the next batch,
    fill or :func:`new_v7` call in the same millisecond can sort before its tail.

    ``bytearray`` specifically, not ``memoryview`` or NumPy arrays, for now: the general
    writable buffer protocol needs ``Py_buffer``, which entered CPython's stable ABI in 3.11.
    That is this extension's floor (``abi3-py311``), so the wider form is possible and not
    built yet.

    :raises TypeError: if ``buffer`` is not a ``bytearray``, or ``unix_millis`` is not an int,
        a ``datetime.datetime`` or ``None``.
    :raises ValueError: if ``len(buffer)`` is not a multiple of 16 or holds more than
        :data:`MAX_V7_BATCH` UUIDs, or ``unix_millis`` is negative or does not fit the 48-bit
        ``unix_ts_ms`` field.
    """
    _native.fill_v7_bytes(buffer, _unix_millis_from(unix_millis, _V7_OUT_OF_RANGE))


def fill_v6(buffer: bytearray, unix_millis: int | datetime.datetime | None = None) -> None:
    """Fill ``buffer`` with raw RFC 9562-ordered version 6 UUID bytes, 16 per UUID.

    The version 6 counterpart to :func:`fill_v7`, with the same performance characteristics
    and the same guidance about when it is and isn't the right call — see that function.

    ``clock_seq`` and ``node`` are independently random per item; unlike version 7 there is no
    monotonic counter, so items minted within the same millisecond are not guaranteed to sort
    in creation order.

    :raises TypeError: if ``buffer`` is not a ``bytearray``, or ``unix_millis`` is not an int,
        a ``datetime.datetime`` or ``None``.
    :raises ValueError: if ``len(buffer)`` is not a multiple of 16, or ``unix_millis`` is
        negative or does not fit the 60-bit v6 timestamp field.
    """
    _native.fill_v6_bytes(buffer, _unix_millis_from(unix_millis, _V6_OUT_OF_RANGE))
