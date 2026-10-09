"""Version/variant inspection and the layout-aware doors against values minted at run time,
by this package and by the standard library; ``test_corpus.py`` pins the fixed vectors."""

import datetime
import itertools
import struct
import sys
import uuid

import pytest

import hyperuuid
from hyperuuid import Layout, Variant, _native

MS = 1_645_557_742_000
AT_MS = datetime.datetime(1970, 1, 1, tzinfo=datetime.timezone.utc) + datetime.timedelta(
    milliseconds=MS
)

# Pyodide is wasm32: a gigabyte-sized buffer is most of its address space.
_SMALL_ADDRESS_SPACE = sys.platform == "emscripten" or sys.maxsize < 2**32


def _minted():
    yield hyperuuid.new_v4(), 4
    yield hyperuuid.new_v5(uuid.NAMESPACE_DNS, "x"), 5
    yield hyperuuid.new_v6(MS), 6
    yield hyperuuid.new_v7(MS), 7
    yield uuid.uuid4(), 4
    yield uuid.uuid5(uuid.NAMESPACE_DNS, "x"), 5
    yield uuid.uuid1(), 1
    # Python 3.14 added these; on 3.11-3.13 there is nothing more to check.
    if hasattr(uuid, "uuid6"):
        yield uuid.uuid6(), 6
    if hasattr(uuid, "uuid7"):
        yield uuid.uuid7(), 7


def test_stdlib_and_package_values_report_their_version_and_the_rfc_variant():
    """version, variant and is_rfc agree with what minted the value, and with stdlib."""
    for id_, version in _minted():
        assert hyperuuid.version(id_) == version
        assert hyperuuid.version(id_) == id_.version  # the standard library agrees
        assert hyperuuid.variant(id_) is Variant.RFC9562
        assert id_.variant == uuid.RFC_4122
        assert hyperuuid.is_rfc(id_, version) is True
        assert hyperuuid.is_rfc(id_, 6 if version == 7 else 7) is False


def test_nil_and_max_are_classified_as_the_rfc_says():
    """Nil is version 0 of the NCS variant, Max version 15 of the future one; neither is RFC."""
    assert hyperuuid.version(hyperuuid.NIL) == 0
    assert hyperuuid.variant(hyperuuid.NIL) is Variant.NCS
    assert hyperuuid.version(hyperuuid.MAX) == 15
    assert hyperuuid.variant(hyperuuid.MAX) is Variant.FUTURE
    assert hyperuuid.is_rfc(hyperuuid.NIL, 0) is False
    assert hyperuuid.is_rfc(hyperuuid.MAX, 15) is False


def test_the_enums_carry_the_core_codes():
    """Layout and Variant are IntEnums holding the native core's codes."""
    assert [int(member) for member in Layout] == [1, 2]
    assert [int(member) for member in Variant] == [1, 2, 3, 4]
    # An int equal to a code is accepted where a Layout is, as an IntEnum invites.
    id_ = hyperuuid.v7_to_sql_order(hyperuuid.new_v7(MS))
    assert hyperuuid.version(id_, 2) == 7


def test_a_version_outside_the_nibble_never_matches():
    """is_rfc is False for a version past 15 or below 0, never an error."""
    id_ = hyperuuid.new_v7(MS)
    for version in (7 + 256, 16, -1, 2**70):
        assert hyperuuid.is_rfc(id_, version) is False
        assert hyperuuid.is_rfc(id_, version, Layout.SQL_SERVER) is False


def test_a_non_integer_version_is_a_type_error():
    """is_rfc's version must be an integer."""
    with pytest.raises(TypeError, match="version must be an int, not str"):
        hyperuuid.is_rfc(hyperuuid.new_v7(MS), "7")


def test_sql_ordered_v6_and_v7_are_validated_and_read_in_place_without_confusion():
    """In SQL order a v6's random clock_seq sits where a v7's version nibble does and reads as 7
    one time in 16; enough draws that a confusion would surface."""
    for _ in range(2048):
        six = hyperuuid.v6_to_sql_order(hyperuuid.new_v6(MS))
        seven = hyperuuid.v7_to_sql_order(hyperuuid.new_v7(MS))

        assert hyperuuid.version(six, Layout.SQL_SERVER) == 6
        assert hyperuuid.version(seven, Layout.SQL_SERVER) == 7
        assert hyperuuid.is_rfc(six, 6, Layout.SQL_SERVER) is True
        assert hyperuuid.is_rfc(six, 7, Layout.SQL_SERVER) is False
        assert hyperuuid.is_rfc(seven, 7, Layout.SQL_SERVER) is True
        assert hyperuuid.is_rfc(seven, 6, Layout.SQL_SERVER) is False

        assert hyperuuid.v6_unix_millis(six, Layout.SQL_SERVER) == MS
        assert hyperuuid.v7_unix_millis(seven, Layout.SQL_SERVER) == MS
        assert hyperuuid.v6_timestamp(six, Layout.SQL_SERVER) == AT_MS
        assert hyperuuid.v7_timestamp(seven, Layout.SQL_SERVER) == AT_MS
        assert hyperuuid.get_timestamp(six, Layout.SQL_SERVER) == AT_MS
        assert hyperuuid.get_timestamp(seven, Layout.SQL_SERVER) == AT_MS
        # Read straight from SQL order matches permuting back first.
        assert hyperuuid.v7_unix_millis(seven, Layout.SQL_SERVER) == hyperuuid.v7_unix_millis(
            hyperuuid.v7_from_sql_order(seven)
        )


def test_non_sql_values_have_no_sql_version():
    """Bytes that don't form a SQL-ordered v6/v7 read as version 0 with no timestamp there.

    Fixed values only: a random v4 in RFC order carries the variant in octet 8 and a random
    octet 7, so one in 16 genuinely forms a SQL-ordered v7. That is the core answering
    correctly about the bytes; which layout a value is in is the caller's to know.
    """
    for id_ in (
        hyperuuid.NIL,
        hyperuuid.MAX,
        uuid.UUID("919108f7-52d1-4320-9bac-f847db4148a8"),  # v4, RFC 9562 A.3
        uuid.UUID("2ed6657d-e927-568b-95e1-2665a8aea6a2"),  # v5, RFC 9562 A.4
    ):
        assert hyperuuid.version(id_, Layout.SQL_SERVER) == 0
        assert hyperuuid.get_timestamp(id_, Layout.SQL_SERVER) is None


LAYOUT_DOORS = [
    pytest.param(lambda id_, layout: hyperuuid.version(id_, layout), id="version"),
    pytest.param(lambda id_, layout: hyperuuid.is_rfc(id_, 7, layout), id="is_rfc"),
    pytest.param(lambda id_, layout: hyperuuid.v6_unix_millis(id_, layout), id="v6_unix_millis"),
    pytest.param(lambda id_, layout: hyperuuid.v7_unix_millis(id_, layout), id="v7_unix_millis"),
    pytest.param(lambda id_, layout: hyperuuid.v6_timestamp(id_, layout), id="v6_timestamp"),
    pytest.param(lambda id_, layout: hyperuuid.v7_timestamp(id_, layout), id="v7_timestamp"),
    pytest.param(lambda id_, layout: hyperuuid.get_timestamp(id_, layout), id="get_timestamp"),
]


@pytest.mark.parametrize("door", LAYOUT_DOORS)
def test_an_unknown_layout_is_a_value_error(door):
    """A layout code that is not a Layout is refused, never passed through or guessed at."""
    id_ = hyperuuid.new_v7(MS)
    for layout in (0, 3, -1, 2**64):
        with pytest.raises(ValueError, match="layout must be Layout.RFC9562 or Layout.SQL_SERVER"):
            door(id_, layout)


@pytest.mark.parametrize("door", LAYOUT_DOORS)
def test_a_non_integer_layout_is_a_type_error(door):
    """A layout that is not an integer at all is a TypeError naming its type."""
    id_ = hyperuuid.new_v7(MS)
    for layout in ("sql_server", None, 1.0):
        with pytest.raises(TypeError, match="layout must be a hyperuuid.Layout"):
            door(id_, layout)


def test_the_extension_refuses_an_unknown_layout_code_itself():
    """A direct caller of _native gets the same refusal rather than a guessed layout."""
    id_ = hyperuuid.new_v7(MS)
    for call in (
        lambda: _native.version(id_, 3),
        lambda: _native.is_rfc(id_, 7, 0),
        lambda: _native.v6_unix_millis_in(id_, 3),
        lambda: _native.v7_unix_millis_in(id_, 3),
        lambda: _native.v6_timestamp(id_, 3),
        lambda: _native.v7_timestamp(id_, 3),
    ):
        with pytest.raises(ValueError, match="layout must be 1"):
            call()


@pytest.mark.parametrize(
    "call",
    [
        pytest.param(lambda value: hyperuuid.version(value), id="version"),
        pytest.param(lambda value: hyperuuid.variant(value), id="variant"),
        pytest.param(lambda value: hyperuuid.is_rfc(value, 7), id="is_rfc"),
        pytest.param(lambda value: hyperuuid.is_rfc(value, 99), id="is_rfc-out-of-range"),
    ],
)
def test_raw_bytes_are_not_a_uuid(call):
    """There is no raw-byte form: 16 bytes are a TypeError, like any other non-UUID."""
    with pytest.raises(TypeError, match="expected a uuid.UUID, not bytes"):
        call(bytes(16))


def test_max_v7_batch_is_the_counter_space():
    """MAX_V7_BATCH is 2**26, the core's own limit."""
    assert hyperuuid.MAX_V7_BATCH == 1 << 26 == _native.MAX_V7_BATCH


def test_a_v7_batch_past_the_counter_space_is_refused_before_allocating():
    """new_v7_batch refuses MAX_V7_BATCH + 1 before reserving its buffer — so this returns at
    once rather than after a gigabyte (and 67 million UUID objects) is built."""
    too_many = hyperuuid.MAX_V7_BATCH + 1
    for call in (
        lambda: hyperuuid.new_v7_batch(too_many, MS),
        lambda: hyperuuid.new_v7_batch(too_many),
        lambda: _native.new_v7_batch(too_many, MS),
    ):
        with pytest.raises(ValueError, match=str(hyperuuid.MAX_V7_BATCH)):
            call()
    # v6 has no counter and no such limit; a count past it is only a size, so it is not tried
    # here, but zero still works on both.
    assert hyperuuid.new_v7_batch(0, MS) == []


@pytest.mark.skipif(_SMALL_ADDRESS_SPACE, reason="needs a 1 GiB buffer")
def test_a_v7_fill_past_the_counter_space_is_refused_before_writing():
    """fill_v7 refuses a buffer of MAX_V7_BATCH + 1 UUIDs without writing a byte. The buffer is
    the caller's (and bytearray(n) is calloc'd, so its untouched pages cost nothing)."""
    buffer = bytearray((hyperuuid.MAX_V7_BATCH + 1) * 16)
    with pytest.raises(ValueError, match=str(hyperuuid.MAX_V7_BATCH)):
        hyperuuid.fill_v7(buffer, MS)
    assert buffer[:64] == bytes(64)
    del buffer


@pytest.mark.skipif(_SMALL_ADDRESS_SPACE, reason="needs a 1 GiB buffer")
def test_a_v7_fill_of_exactly_the_counter_space_is_strictly_increasing():
    """Exactly MAX_V7_BATCH UUIDs, 1 GiB of bytes, in strictly increasing order end to end, and
    at most a millisecond past the supplied timestamp (the roll-forward over the counter wrap).
    About ten seconds, nearly all of it the comparison."""
    count = hyperuuid.MAX_V7_BATCH
    buffer = bytearray(count * 16)
    hyperuuid.fill_v7(buffer, MS)
    records = (record for (record,) in struct.iter_unpack("16s", buffer))
    assert all(earlier < later for earlier, later in itertools.pairwise(records))
    last = uuid.UUID(bytes=bytes(buffer[-16:]))
    assert MS <= hyperuuid.v7_unix_millis(last) <= MS + 1


def test_get_timestamp_requires_the_rfc_variant():
    """A 6 or 7 nibble under another variant has no RFC version, so no timestamp; the
    version nibble still reads, and the per-version doors still read the field as asked."""
    seven = hyperuuid.new_v7(MS).bytes
    for top in (0x00, 0xC0, 0xE0):  # NCS, Microsoft, future
        id_ = uuid.UUID(bytes=seven[:8] + bytes([top | (seven[8] & 0x1F)]) + seven[9:])
        assert id_.version is None  # stdlib agrees there is no RFC version
        assert hyperuuid.version(id_) == 7
        assert hyperuuid.get_timestamp(id_) is None
        assert hyperuuid.v7_unix_millis(id_) == MS
    sql = bytearray(hyperuuid.v7_to_sql_order(hyperuuid.new_v7(MS)).bytes)
    sql[8] &= 0x3F  # clear the variant where SQL order keeps it
    assert hyperuuid.get_timestamp(uuid.UUID(bytes=bytes(sql)), Layout.SQL_SERVER) is None
