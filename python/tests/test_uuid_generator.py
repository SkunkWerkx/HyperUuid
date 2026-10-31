"""The generator doors: every UUID version's bits, timestamps, batches, SQL order and buffer
fills."""

import datetime
import re
import sys
import time
import uuid

import pytest

import hyperuuid

RFC_TEST_VECTOR_MS = 1_645_557_742_000


def _now_ms() -> int:
    """The wall clock in Unix-epoch milliseconds, read the way the core reads it. Pyodide's
    clock is whole milliseconds carried as a double nanosecond count, which can land a few
    nanoseconds under the millisecond it means, so it is rounded there and truncated on a real
    clock."""
    if sys.platform == "emscripten":
        return (time.time_ns() + 500_000) // 1_000_000
    return time.time_ns() // 1_000_000


def test_v4_has_version_and_variant_bits_set():
    """A v4 UUID carries version 4 and the RFC variant."""
    id_ = hyperuuid.new_v4()
    assert id_.version == 4
    assert id_.variant == uuid.RFC_4122


def test_v4_is_non_deterministic():
    """A hundred v4 UUIDs are a hundred different UUIDs."""
    results = {hyperuuid.new_v4() for _ in range(100)}
    assert len(results) == 100


def test_v5_matches_rfc_test_vector():
    """RFC 9562 Appendix A.4 official test vector."""
    id_ = hyperuuid.new_v5(uuid.NAMESPACE_DNS, "www.example.com")
    assert id_ == uuid.UUID("2ed6657d-e927-568b-95e1-2665a8aea6a2")


def test_v5_matches_python_docs_vector():
    """Same test vector Python's own uuid module documentation uses."""
    id_ = hyperuuid.new_v5(uuid.NAMESPACE_DNS, "python.org")
    assert id_ == uuid.UUID("886313e1-3b8a-5372-9b90-0c9aee199e5d")


def test_v5_matches_stdlib_uuid5():
    """hyperuuid's v5 should agree byte-for-byte with Python's own (SHA-1-based) uuid5."""
    for name in ("same-name", "café — 日本語"):
        assert hyperuuid.new_v5(uuid.NAMESPACE_URL, name) == uuid.uuid5(uuid.NAMESPACE_URL, name)


def test_v5_is_deterministic():
    """The same namespace and name give the same v5 UUID."""
    a = hyperuuid.new_v5(uuid.NAMESPACE_DNS, "same-name")
    b = hyperuuid.new_v5(uuid.NAMESPACE_DNS, "same-name")
    assert a == b


def test_v5_different_namespaces_differ():
    """The same name in two namespaces gives two v5 UUIDs."""
    dns = hyperuuid.new_v5(uuid.NAMESPACE_DNS, "test")
    url = hyperuuid.new_v5(uuid.NAMESPACE_URL, "test")
    assert dns != url


def test_v5_bytes_and_str_name_agree():
    """A str name and its UTF-8 bytes give the same v5 UUID."""
    a = hyperuuid.new_v5(uuid.NAMESPACE_URL, "test-name")
    b = hyperuuid.new_v5(uuid.NAMESPACE_URL, b"test-name")
    assert a == b


def test_nil_is_all_zero_bytes():
    """NIL is sixteen zero bytes."""
    assert hyperuuid.NIL.bytes == bytes(16)
    assert str(hyperuuid.NIL) == "00000000-0000-0000-0000-000000000000"


def test_max_is_all_one_bytes():
    """MAX is sixteen 0xFF bytes."""
    assert hyperuuid.MAX.bytes == b"\xff" * 16
    assert str(hyperuuid.MAX) == "ffffffff-ffff-ffff-ffff-ffffffffffff"


def test_v7_embeds_the_timestamp():
    """A v7 UUID's first 48 bits are its millisecond timestamp."""
    id_ = hyperuuid.new_v7(RFC_TEST_VECTOR_MS)
    embedded_ms = int.from_bytes(id_.bytes[0:6], "big")
    assert embedded_ms == RFC_TEST_VECTOR_MS


def test_v7_has_version_and_variant_bits_set():
    """A v7 UUID carries version 7 and the RFC variant."""
    id_ = hyperuuid.new_v7(RFC_TEST_VECTOR_MS)
    assert id_.version == 7
    assert id_.variant == uuid.RFC_4122


def test_v7_overflow_timestamp_raises():
    """A v7 timestamp past the cap raises ValueError."""
    with pytest.raises(ValueError):
        hyperuuid.new_v7(0x0001_0000_0000_0000)


def test_v7_same_millisecond_batch_is_monotonically_ordered():
    """v7 UUIDs minted in one millisecond sort in minting order. The stream orders calls within
    10 s of its newest timestamp, so the millisecond is the present one; an old one is minted
    out of stream and ordered against nothing."""
    ms = _now_ms()
    ids = [hyperuuid.new_v7(ms) for _ in range(100)]
    assert ids == sorted(ids)


def test_v7_current_timestamp_is_embedded():
    """A v7 UUID minted with no timestamp carries the current time."""
    before = _now_ms()
    id_ = hyperuuid.new_v7()
    after = _now_ms()

    embedded_ms = int.from_bytes(id_.bytes[0:6], "big")
    # The stream may stamp a later ms that an earlier test moved it to.
    assert before <= embedded_ms <= after + 1_000


def test_v7_timestamp_recovers_the_exact_millisecond():
    """v7_timestamp recovers the exact millisecond minted."""
    id_ = hyperuuid.new_v7(RFC_TEST_VECTOR_MS)
    expected = datetime.datetime.fromtimestamp(RFC_TEST_VECTOR_MS / 1000, tz=datetime.timezone.utc)
    assert hyperuuid.v7_timestamp(id_) == expected


def test_v7_timestamp_round_trips_zero_and_a_large_timestamp():
    """v7_timestamp round-trips the epoch and the latest datetime can hold."""
    assert hyperuuid.v7_timestamp(hyperuuid.new_v7(0)).timestamp() == 0

    # Inside datetime's last year; the generation cap is its last millisecond (the test
    # below pins that boundary).
    large_ms = int(datetime.datetime(9999, 1, 1, tzinfo=datetime.timezone.utc).timestamp() * 1000)
    recovered = hyperuuid.v7_timestamp(hyperuuid.new_v7(large_ms))
    assert int(recovered.timestamp() * 1000) == large_ms


def test_v7_unix_millis_returns_the_embedded_integer():
    """v7_unix_millis returns the millisecond minted."""
    assert hyperuuid.v7_unix_millis(hyperuuid.new_v7(RFC_TEST_VECTOR_MS)) == RFC_TEST_VECTOR_MS
    assert hyperuuid.v7_unix_millis(hyperuuid.new_v7(0)) == 0
    # The generation cap, 9999-12-31T23:59:59.999Z, exactly.
    assert hyperuuid.v7_unix_millis(hyperuuid.new_v7(253_402_300_799_999)) == 253_402_300_799_999


def test_unix_millis_agrees_with_the_datetime_doors():
    """The millisecond doors agree with the datetime doors."""
    epoch = datetime.datetime(1970, 1, 1, tzinfo=datetime.timezone.utc)
    # Up to the start of datetime's last year; v7's milliseconds outlast datetime itself.
    for millis in (0, 1, 999, RFC_TEST_VECTOR_MS, 253_370_764_800_000):
        id_ = hyperuuid.new_v7(millis)
        assert hyperuuid.v7_unix_millis(id_) == millis
        assert hyperuuid.v7_timestamp(id_) == epoch + datetime.timedelta(milliseconds=millis)


def test_v7_timestamps_stop_at_the_last_millisecond_of_9999_in_both_directions():
    """Minting stops at 9999-12-31T23:59:59.999Z, the last instant datetime can name; one
    millisecond more is the out-of-range ValueError. The version-agnostic door treats a version 7
    another generator dated past that as it treats a version 6: no timestamp. The 48-bit field
    reaches 10889, so the per-version millisecond reader still reads such a value, and
    v7_timestamp, which builds a datetime, raises OverflowError for it as documented."""
    last = datetime.datetime(9999, 12, 31, 23, 59, 59, 999_000, tzinfo=datetime.timezone.utc)
    id_ = hyperuuid.new_v7(253_402_300_799_999)
    assert hyperuuid.v7_timestamp(id_) == last
    assert hyperuuid.get_timestamp(id_) == last
    with pytest.raises(ValueError):
        hyperuuid.new_v7(253_402_300_800_000)

    past = uuid.UUID("ffffffff-ffff-7000-8000-010203040506")
    assert hyperuuid.is_rfc(past, 7)
    assert hyperuuid.get_timestamp(past) is None
    assert hyperuuid.v7_unix_millis(past) == 0x0000_FFFF_FFFF_FFFF
    with pytest.raises(OverflowError):
        hyperuuid.v7_timestamp(past)


@pytest.mark.skipif(
    sys.version_info < (3, 14), reason="stdlib uuid.uuid7() was added in Python 3.14"
)
def test_v7_timestamp_extracts_from_the_stdlib_native_generator():
    """Proves v7_timestamp isn't just reading back what our own new_v7 wrote — it's a plain RFC 9562
    bit-layout read, so it recovers the real embedded timestamp from a version 7 UUID minted by
    Python's own stdlib generator too.
    """
    before = time.time_ns() // 1_000_000
    native = uuid.uuid7()
    after = time.time_ns() // 1_000_000

    # Whole milliseconds, as v7 holds them: a float comparison at today's epoch seconds rounds
    # the 1 ms slack itself, and failed when uuid7()'s clock read 1 ms behind time.time()'s.
    got = hyperuuid.v7_timestamp(native)
    epoch = datetime.datetime(1970, 1, 1, tzinfo=datetime.timezone.utc)
    got_ms = (got - epoch) // datetime.timedelta(milliseconds=1)
    assert before - 1 <= got_ms <= after + 1


def test_new_v7_accepts_a_datetime_in_place_of_a_millisecond_int():
    """new_v7 takes a datetime where it takes milliseconds."""
    dt = datetime.datetime.fromtimestamp(RFC_TEST_VECTOR_MS / 1000, tz=datetime.timezone.utc)
    id_ = hyperuuid.new_v7(dt)
    assert hyperuuid.v7_timestamp(id_) == dt


def test_get_timestamp_returns_none_for_non_time_based_versions():
    """get_timestamp is None for v4 and v5, which carry no time."""
    assert hyperuuid.get_timestamp(hyperuuid.new_v4()) is None
    assert hyperuuid.get_timestamp(hyperuuid.new_v5(uuid.NAMESPACE_DNS, "test")) is None


def test_get_timestamp_returns_none_for_a_version_6():
    """Version 6 is not supported, so a v6 from another generator carries no timestamp."""
    assert hyperuuid.get_timestamp(uuid.UUID("1ec9414c-232a-6b00-b3c8-9f6bdeced846")) is None


def test_get_timestamp_matches_v7_timestamp():
    """get_timestamp agrees with v7_timestamp on a v7 UUID."""
    id_ = hyperuuid.new_v7(RFC_TEST_VECTOR_MS)
    assert hyperuuid.get_timestamp(id_) == hyperuuid.v7_timestamp(id_)


def test_v7_batch_returns_count_uuids_sorted_and_sharing_the_timestamp():
    """new_v7_batch returns count sorted UUIDs, all carrying the given timestamp."""
    ids = hyperuuid.new_v7_batch(1000, RFC_TEST_VECTOR_MS)
    assert len(ids) == 1000
    assert ids == sorted(ids)
    for id_ in ids:
        embedded_ms = int.from_bytes(id_.bytes[0:6], "big")
        assert embedded_ms == RFC_TEST_VECTOR_MS


def test_v7_batch_continues_the_same_counter_sequence_as_individual_calls():
    """A v7 batch sorts between the single UUIDs minted before and after it."""
    now = _now_ms()  # in stream: an old millisecond is minted out of stream, unordered
    before = hyperuuid.new_v7(now)
    batch = hyperuuid.new_v7_batch(10, now)
    after = hyperuuid.new_v7(now)

    ids = [before, *batch, after]
    assert ids == sorted(ids)


def test_v7_batch_count_zero_returns_empty_list():
    """A v7 batch of zero is an empty list."""
    assert hyperuuid.new_v7_batch(0, RFC_TEST_VECTOR_MS) == []


def test_v7_batch_overflow_timestamp_raises():
    """A v7 batch at a timestamp past the cap raises ValueError."""
    with pytest.raises(ValueError):
        hyperuuid.new_v7_batch(1, 0x0001_0000_0000_0000)


def test_v7_to_sql_order_round_trips_through_v7_from_sql_order():
    """v7_from_sql_order undoes v7_to_sql_order."""
    id_ = hyperuuid.new_v7(RFC_TEST_VECTOR_MS)
    sql_ordered = hyperuuid.v7_to_sql_order(id_)
    assert sql_ordered != id_
    assert hyperuuid.v7_from_sql_order(sql_ordered) == id_


def test_v7_to_sql_order_is_a_version_8_where_sql_server_shows_it():
    """The SQL layout is an RFC 9562 version 8: the 8 at octet 7's top nibble and the variant
    at octet 8's top bits, where the mixed-endian view (``bytes_le``, .NET's Guid order, and
    SQL Server's text form) reads them."""
    sql_ordered = hyperuuid.v7_to_sql_order(hyperuuid.new_v7(RFC_TEST_VECTOR_MS))
    b = sql_ordered.bytes
    assert b[7] & 0xF0 == 0x80
    assert b[8] & 0xC0 == 0x80
    mixed_endian = uuid.UUID(bytes_le=b)
    assert mixed_endian.version == 8
    assert mixed_endian.variant == uuid.RFC_4122
    assert hyperuuid.version(sql_ordered, hyperuuid.Layout.SQL_SERVER) == 7


def _sql_guid_key(uuid_value):
    # Replicates System.Data.SqlTypes.SqlGuid.CompareTo's fixed byte significance order — the
    # correctness oracle this project's C# test suite checks directly against the real type;
    # no equivalent exists in Python's stdlib to test against here, so this stands in for it.
    significance_order = [10, 11, 12, 13, 14, 15, 8, 9, 6, 7, 4, 5, 0, 1, 2, 3]
    b = uuid_value.bytes
    return tuple(b[i] for i in significance_order)


def test_v7_to_sql_order_sorts_by_creation_order_under_sqlguid_comparison():
    """v7 UUIDs in SQL order sort by creation under SQL Server's uniqueidentifier order."""
    now = _now_ms()  # in stream: an old millisecond is minted out of stream, unordered
    ids = [hyperuuid.new_v7(now + i) for i in range(200)]
    # Same-millisecond run, so the counter (not just the timestamp) has to sort correctly too.
    ids += [hyperuuid.new_v7(now + 200) for _ in range(200)]

    sql_ordered = [hyperuuid.v7_to_sql_order(id_) for id_ in ids]
    sorted_by_sqlguid = sorted(sql_ordered, key=_sql_guid_key)

    assert sql_ordered == sorted_by_sqlguid


_WRONG_VERSION = "the value is not a UUID of the version this conversion takes"


def _raises_exactly(message):
    return pytest.raises(ValueError, match=f"^{re.escape(message)}$")


def test_sql_order_conversions_refuse_the_other_version_and_the_other_order():
    """Each conversion takes only a version 7 in its own order: another version is refused, and
    an RFC value is not taken as a SQL-ordered one."""
    six = uuid.UUID("1ec9414c-232a-6b00-b3c8-9f6bdeced846")  # v6, RFC 9562 A.5
    # RFC 9562 A.6's v7 and its SQL form, fixed: a fresh v7's octet 7 reads as the SQL
    # layout's 8 one time in 16, and a fresh SQL-ordered one's octet 6 as an RFC 7 as often.
    seven = uuid.UUID("017f22e2-79b0-7cc3-98c4-dc0c0c07398f")
    sql_seven = uuid.UUID("0c07398f-dc0c-8c84-b30d-017f22e279b0")
    assert hyperuuid.v7_to_sql_order(seven) == sql_seven
    refused = [
        (hyperuuid.v7_to_sql_order, six),
        (hyperuuid.v7_to_sql_order, hyperuuid.new_v4()),
        (hyperuuid.v7_to_sql_order, hyperuuid.NIL),
        (hyperuuid.v7_to_sql_order, sql_seven),
        (hyperuuid.v7_from_sql_order, seven),
        (hyperuuid.v7_from_sql_order, six),
        (hyperuuid.v7_from_sql_order, hyperuuid.NIL),
    ]
    for convert, value in refused:
        with _raises_exactly(_WRONG_VERSION):
            convert(value)


def test_every_v7_converts_to_sql_order_and_back():
    """The layout holds all 122 of a v7's free bits, so every timestamp converts, the generation
    cap included, and the way back is exact."""
    for millis in (0, RFC_TEST_VECTOR_MS, 2**47 - 1, 2**47, 253_402_300_799_999):
        id_ = hyperuuid.new_v7(millis)
        sql_ordered = hyperuuid.v7_to_sql_order(id_)
        assert hyperuuid.v7_from_sql_order(sql_ordered) == id_
        assert hyperuuid.v7_unix_millis(sql_ordered, hyperuuid.Layout.SQL_SERVER) == millis


def test_a_sql_ordered_value_with_a_7_where_the_8_goes_is_refused():
    """The layout's version nibble (octet 7's top) is 8; a 7 there is not a SQL-ordered value,
    so it reads as version 0 and the way back refuses it."""
    sql = bytearray(hyperuuid.v7_to_sql_order(hyperuuid.new_v7(RFC_TEST_VECTOR_MS)).bytes)
    sql[7] = (sql[7] & 0x0F) | 0x70
    old_form = uuid.UUID(bytes=bytes(sql))
    assert hyperuuid.version(old_form, hyperuuid.Layout.SQL_SERVER) == 0
    with _raises_exactly(_WRONG_VERSION):
        hyperuuid.v7_from_sql_order(old_form)


# ---- Destination-buffer fills -------------------------------------------------------


def test_fill_v7_fills_the_callers_buffer():
    """fill_v7 writes a v7 UUID into every 16 bytes of the caller's buffer."""
    count = 64
    buf = bytearray(count * 16)
    hyperuuid.fill_v7(buf, RFC_TEST_VECTOR_MS)
    ids = [uuid.UUID(bytes=bytes(buf[i * 16 : (i + 1) * 16])) for i in range(count)]
    assert all(i.version == 7 for i in ids)
    assert all(hyperuuid.v7_timestamp(i).timestamp() * 1000 == RFC_TEST_VECTOR_MS for i in ids)


def test_fill_v7_is_strictly_increasing():
    """The UUIDs fill_v7 writes strictly increase."""
    count = 256
    buf = bytearray(count * 16)
    hyperuuid.fill_v7(buf, RFC_TEST_VECTOR_MS)
    raw = [bytes(buf[i * 16 : (i + 1) * 16]) for i in range(count)]
    assert raw == sorted(raw)
    assert len(set(raw)) == count


def test_fill_matches_the_batch_form_structurally():
    """The two paths draw their own entropy, so they are not value-equal — but every item
    must decode to the same version carrying the same timestamp."""
    count = 16
    buf = bytearray(count * 16)
    hyperuuid.fill_v7(buf, RFC_TEST_VECTOR_MS)
    from_bytes = [uuid.UUID(bytes=bytes(buf[i * 16 : (i + 1) * 16])) for i in range(count)]
    from_batch = hyperuuid.new_v7_batch(count, RFC_TEST_VECTOR_MS)
    assert [i.version for i in from_bytes] == [i.version for i in from_batch]
    assert [hyperuuid.v7_timestamp(i) for i in from_bytes] == [
        hyperuuid.v7_timestamp(i) for i in from_batch
    ]


def test_fill_rejects_a_partial_uuid_buffer():
    """A buffer that is not a whole number of UUIDs raises ValueError."""
    with pytest.raises(ValueError):
        hyperuuid.fill_v7(bytearray(17), RFC_TEST_VECTOR_MS)
    with pytest.raises(ValueError):
        hyperuuid.fill_v7(bytearray(15), RFC_TEST_VECTOR_MS)


def test_fill_empty_buffer_is_a_no_op():
    """Filling an empty buffer does nothing."""
    hyperuuid.fill_v7(bytearray(0), RFC_TEST_VECTOR_MS)


def test_fill_v7_rejects_an_out_of_range_timestamp():
    """fill_v7 raises ValueError for a timestamp past the cap."""
    with pytest.raises(ValueError):
        hyperuuid.fill_v7(bytearray(16), 253_402_300_800_000)


def test_fill_v7_defaults_to_now():
    """fill_v7 with no timestamp writes the current time."""
    buf = bytearray(16)
    before = time.time()
    hyperuuid.fill_v7(buf)
    after = time.time()
    ts = hyperuuid.v7_timestamp(uuid.UUID(bytes=bytes(buf))).timestamp()
    assert before - 1 <= ts <= after + 1


def test_fill_fully_overwrites_a_reused_buffer():
    """The buffer-reuse case this API exists for: no byte of a previous fill may survive."""
    buf = bytearray(b"\xaa" * (8 * 16))
    hyperuuid.fill_v7(buf, RFC_TEST_VECTOR_MS)
    first = bytes(buf)
    assert b"\xaa" * 16 not in first
    hyperuuid.fill_v7(buf, RFC_TEST_VECTOR_MS)
    assert bytes(buf) != first, "second fill returned identical bytes"
    assert all(uuid.UUID(bytes=bytes(buf[i * 16 : (i + 1) * 16])).version == 7 for i in range(8))


@pytest.mark.parametrize(
    "call",
    [
        lambda: hyperuuid.new_v5("not a uuid", "name"),
        lambda: hyperuuid.new_v5(None, "name"),
        lambda: hyperuuid.v7_timestamp("not a uuid"),
        lambda: hyperuuid.v7_unix_millis(42),
        lambda: hyperuuid.v7_to_sql_order(b"\x00" * 16),
        lambda: hyperuuid.v7_from_sql_order(None),
    ],
)
def test_a_non_uuid_argument_is_a_type_error_naming_its_type(call):
    """Every UUID argument reads the value's `int`; something with none is the TypeError the README
    promises for a wrong-typed argument, not the AttributeError reading it raises.
    """
    with pytest.raises(TypeError, match=r"expected a uuid\.UUID, not "):
        call()
