import datetime
import sys
import time
import uuid

import pytest

import hyperuuid

RFC_TEST_VECTOR_MS = 1_645_557_742_000


def test_v4_has_version_and_variant_bits_set():
    id_ = hyperuuid.new_v4()
    assert id_.version == 4
    assert id_.variant == uuid.RFC_4122


def test_v4_is_non_deterministic():
    results = {hyperuuid.new_v4() for _ in range(100)}
    assert len(results) == 100


def test_v5_matches_rfc_test_vector():
    # RFC 9562 Appendix A.4 official test vector.
    id_ = hyperuuid.new_v5(uuid.NAMESPACE_DNS, "www.example.com")
    assert id_ == uuid.UUID("2ed6657d-e927-568b-95e1-2665a8aea6a2")


def test_v5_matches_python_docs_vector():
    # Same test vector Python's own uuid module documentation uses.
    id_ = hyperuuid.new_v5(uuid.NAMESPACE_DNS, "python.org")
    assert id_ == uuid.UUID("886313e1-3b8a-5372-9b90-0c9aee199e5d")


def test_v5_matches_stdlib_uuid5():
    # hyperuuid's v5 should agree byte-for-byte with Python's own (SHA-1-based) uuid5.
    for name in ("same-name", "café — 日本語"):
        assert hyperuuid.new_v5(uuid.NAMESPACE_URL, name) == uuid.uuid5(uuid.NAMESPACE_URL, name)


def test_v5_is_deterministic():
    a = hyperuuid.new_v5(uuid.NAMESPACE_DNS, "same-name")
    b = hyperuuid.new_v5(uuid.NAMESPACE_DNS, "same-name")
    assert a == b


def test_v5_different_namespaces_differ():
    dns = hyperuuid.new_v5(uuid.NAMESPACE_DNS, "test")
    url = hyperuuid.new_v5(uuid.NAMESPACE_URL, "test")
    assert dns != url


def test_v5_bytes_and_str_name_agree():
    a = hyperuuid.new_v5(uuid.NAMESPACE_URL, "test-name")
    b = hyperuuid.new_v5(uuid.NAMESPACE_URL, b"test-name")
    assert a == b


def test_v6_embeds_the_timestamp():
    id_ = hyperuuid.new_v6(RFC_TEST_VECTOR_MS)
    expected = datetime.datetime.fromtimestamp(RFC_TEST_VECTOR_MS / 1000, tz=datetime.timezone.utc)
    assert hyperuuid.v6_timestamp(id_) == expected


def test_v6_has_version_and_variant_bits_set():
    id_ = hyperuuid.new_v6(RFC_TEST_VECTOR_MS)
    assert id_.version == 6
    assert id_.variant == uuid.RFC_4122


def test_v6_sets_the_node_id_multicast_bit():
    id_ = hyperuuid.new_v6(RFC_TEST_VECTOR_MS)
    assert id_.bytes[10] & 0x01 == 1


def test_v6_is_non_deterministic_within_the_same_millisecond():
    results = {hyperuuid.new_v6(RFC_TEST_VECTOR_MS) for _ in range(100)}
    assert len(results) == 100


def test_v6_current_timestamp_is_embedded():
    before = int(time.time() * 1000)
    id_ = hyperuuid.new_v6()
    after = int(time.time() * 1000)

    embedded_ms = int(hyperuuid.v6_timestamp(id_).timestamp() * 1000)
    assert before <= embedded_ms <= after


def test_v6_batch_returns_count_uuids_sharing_the_timestamp():
    ids = hyperuuid.new_v6_batch(10, RFC_TEST_VECTOR_MS)
    assert len(ids) == 10
    expected = datetime.datetime.fromtimestamp(RFC_TEST_VECTOR_MS / 1000, tz=datetime.timezone.utc)
    for id_ in ids:
        assert id_.version == 6
        assert hyperuuid.v6_timestamp(id_) == expected


def test_v6_batch_produces_pairwise_distinct_uuids():
    ids = hyperuuid.new_v6_batch(100, RFC_TEST_VECTOR_MS)
    assert len(set(ids)) == 100


def test_v6_batch_count_zero_returns_empty_list():
    assert hyperuuid.new_v6_batch(0, RFC_TEST_VECTOR_MS) == []


def test_v6_batch_overflow_timestamp_raises():
    with pytest.raises(ValueError):
        hyperuuid.new_v6_batch(1, 0xFFFF_FFFF_FFFF_FFFF)


def test_nil_is_all_zero_bytes():
    assert hyperuuid.NIL.bytes == bytes(16)
    assert str(hyperuuid.NIL) == "00000000-0000-0000-0000-000000000000"


def test_max_is_all_one_bytes():
    assert hyperuuid.MAX.bytes == b"\xff" * 16
    assert str(hyperuuid.MAX) == "ffffffff-ffff-ffff-ffff-ffffffffffff"


def test_v7_embeds_the_timestamp():
    id_ = hyperuuid.new_v7(RFC_TEST_VECTOR_MS)
    embedded_ms = int.from_bytes(id_.bytes[0:6], "big")
    assert embedded_ms == RFC_TEST_VECTOR_MS


def test_v7_has_version_and_variant_bits_set():
    id_ = hyperuuid.new_v7(RFC_TEST_VECTOR_MS)
    assert id_.version == 7
    assert id_.variant == uuid.RFC_4122


def test_v7_overflow_timestamp_raises():
    with pytest.raises(ValueError):
        hyperuuid.new_v7(0x0001_0000_0000_0000)


def test_v7_same_millisecond_batch_is_monotonically_ordered():
    ids = [hyperuuid.new_v7(RFC_TEST_VECTOR_MS) for _ in range(100)]
    assert ids == sorted(ids)


def test_v7_current_timestamp_is_embedded():
    before = int(time.time() * 1000)
    id_ = hyperuuid.new_v7()
    after = int(time.time() * 1000)

    embedded_ms = int.from_bytes(id_.bytes[0:6], "big")
    assert before <= embedded_ms <= after


def test_v7_timestamp_recovers_the_exact_millisecond():
    id_ = hyperuuid.new_v7(RFC_TEST_VECTOR_MS)
    expected = datetime.datetime.fromtimestamp(RFC_TEST_VECTOR_MS / 1000, tz=datetime.timezone.utc)
    assert hyperuuid.v7_timestamp(id_) == expected


def test_v7_timestamp_round_trips_zero_and_a_large_timestamp():
    assert hyperuuid.v7_timestamp(hyperuuid.new_v7(0)).timestamp() == 0

    # Largest ms value datetime.datetime (year <= 9999) can represent, not the RFC's own
    # 48-bit max (valid to year 10889) — see test below for that boundary.
    large_ms = int(datetime.datetime(9999, 1, 1, tzinfo=datetime.timezone.utc).timestamp() * 1000)
    recovered = hyperuuid.v7_timestamp(hyperuuid.new_v7(large_ms))
    assert int(recovered.timestamp() * 1000) == large_ms


def test_v7_timestamp_raises_past_datetime_year_range():
    # A legitimate RFC 9562 v7 UUID can embed a timestamp datetime.datetime can't hold.
    id_ = hyperuuid.new_v7(0x0000_FFFF_FFFF_FFFF)
    with pytest.raises(OverflowError):
        hyperuuid.v7_timestamp(id_)


@pytest.mark.skipif(sys.version_info < (3, 14), reason="stdlib uuid.uuid7() was added in Python 3.14")
def test_v7_timestamp_extracts_from_the_stdlib_native_generator():
    # Proves v7_timestamp isn't just reading back what our own new_v7 wrote — it's a plain
    # RFC 9562 bit-layout read, so it recovers the real embedded timestamp from a version 7
    # UUID minted by Python's own stdlib generator too.
    before = time.time()
    native = uuid.uuid7()
    after = time.time()

    got = hyperuuid.v7_timestamp(native)
    assert before - 0.001 <= got.timestamp() <= after + 0.001


def test_new_v6_accepts_a_datetime_in_place_of_a_millisecond_int():
    dt = datetime.datetime.fromtimestamp(RFC_TEST_VECTOR_MS / 1000, tz=datetime.timezone.utc)
    id_ = hyperuuid.new_v6(dt)
    assert hyperuuid.v6_timestamp(id_) == dt


def test_new_v7_accepts_a_datetime_in_place_of_a_millisecond_int():
    dt = datetime.datetime.fromtimestamp(RFC_TEST_VECTOR_MS / 1000, tz=datetime.timezone.utc)
    id_ = hyperuuid.new_v7(dt)
    assert hyperuuid.v7_timestamp(id_) == dt


def test_get_timestamp_returns_none_for_non_time_based_versions():
    assert hyperuuid.get_timestamp(hyperuuid.new_v4()) is None
    assert hyperuuid.get_timestamp(hyperuuid.new_v5(uuid.NAMESPACE_DNS, "test")) is None


def test_get_timestamp_matches_v6_timestamp():
    id_ = hyperuuid.new_v6(RFC_TEST_VECTOR_MS)
    assert hyperuuid.get_timestamp(id_) == hyperuuid.v6_timestamp(id_)


def test_get_timestamp_matches_v7_timestamp():
    id_ = hyperuuid.new_v7(RFC_TEST_VECTOR_MS)
    assert hyperuuid.get_timestamp(id_) == hyperuuid.v7_timestamp(id_)


def test_v7_batch_returns_count_uuids_sorted_and_sharing_the_timestamp():
    ids = hyperuuid.new_v7_batch(1000, RFC_TEST_VECTOR_MS)
    assert len(ids) == 1000
    assert ids == sorted(ids)
    for id_ in ids:
        embedded_ms = int.from_bytes(id_.bytes[0:6], "big")
        assert embedded_ms == RFC_TEST_VECTOR_MS


def test_v7_batch_continues_the_same_counter_sequence_as_individual_calls():
    before = hyperuuid.new_v7(RFC_TEST_VECTOR_MS)
    batch = hyperuuid.new_v7_batch(10, RFC_TEST_VECTOR_MS)
    after = hyperuuid.new_v7(RFC_TEST_VECTOR_MS)

    ids = [before, *batch, after]
    assert ids == sorted(ids)


def test_v7_batch_count_zero_returns_empty_list():
    assert hyperuuid.new_v7_batch(0, RFC_TEST_VECTOR_MS) == []


def test_v7_batch_overflow_timestamp_raises():
    with pytest.raises(ValueError):
        hyperuuid.new_v7_batch(1, 0x0001_0000_0000_0000)


def test_v7_to_sql_order_round_trips_through_v7_from_sql_order():
    id_ = hyperuuid.new_v7(RFC_TEST_VECTOR_MS)
    sql_ordered = hyperuuid.v7_to_sql_order(id_)
    assert sql_ordered != id_
    assert hyperuuid.v7_from_sql_order(sql_ordered) == id_


def test_v7_to_sql_order_preserves_version_and_variant_at_octets_7_and_8():
    sql_ordered = hyperuuid.v7_to_sql_order(hyperuuid.new_v7(RFC_TEST_VECTOR_MS))
    b = sql_ordered.bytes
    assert b[7] & 0xF0 == 0x70
    assert b[8] & 0xC0 == 0x80


def _sql_guid_key(uuid_value):
    # Replicates System.Data.SqlTypes.SqlGuid.CompareTo's fixed byte significance order — the
    # correctness oracle this project's C# test suite checks directly against the real type;
    # no equivalent exists in Python's stdlib to test against here, so this stands in for it.
    significance_order = [10, 11, 12, 13, 14, 15, 8, 9, 6, 7, 4, 5, 0, 1, 2, 3]
    b = uuid_value.bytes
    return tuple(b[i] for i in significance_order)


def test_v7_to_sql_order_sorts_by_creation_order_under_sqlguid_comparison():
    ids = [hyperuuid.new_v7(RFC_TEST_VECTOR_MS + i) for i in range(200)]
    # Same-millisecond run, so the counter (not just the timestamp) has to sort correctly too.
    ids += [hyperuuid.new_v7(RFC_TEST_VECTOR_MS + 1_000_000) for _ in range(200)]

    sql_ordered = [hyperuuid.v7_to_sql_order(id_) for id_ in ids]
    sorted_by_sqlguid = sorted(sql_ordered, key=_sql_guid_key)

    assert sql_ordered == sorted_by_sqlguid


def test_v6_to_sql_order_round_trips_through_v6_from_sql_order():
    id_ = hyperuuid.new_v6(RFC_TEST_VECTOR_MS)
    sql_ordered = hyperuuid.v6_to_sql_order(id_)
    assert sql_ordered != id_
    assert hyperuuid.v6_from_sql_order(sql_ordered) == id_


def test_v6_to_sql_order_preserves_version_and_variant():
    # Different offsets than v7's sql order — see v6_to_sql_order's docstring for why.
    sql_ordered = hyperuuid.v6_to_sql_order(hyperuuid.new_v6(RFC_TEST_VECTOR_MS))
    b = sql_ordered.bytes
    assert b[8] & 0xF0 == 0x60
    assert b[6] & 0xC0 == 0x80


def test_v6_to_sql_order_sorts_by_creation_order_under_sqlguid_comparison_for_distinct_timestamps():
    # Unlike v7, v6 has no counter — two UUIDs at the same millisecond aren't guaranteed to
    # sort in creation order even in plain RFC order, so this only exercises strictly
    # increasing timestamps, where the timestamp alone determines order with no tie to break.
    ids = [hyperuuid.new_v6(RFC_TEST_VECTOR_MS + i) for i in range(300)]

    sql_ordered = [hyperuuid.v6_to_sql_order(id_) for id_ in ids]
    sorted_by_sqlguid = sorted(sql_ordered, key=_sql_guid_key)

    assert sql_ordered == sorted_by_sqlguid


# ---- Destination-buffer fills -------------------------------------------------------


def test_fill_v7_fills_the_callers_buffer():
    count = 64
    buf = bytearray(count * 16)
    hyperuuid.fill_v7(buf, RFC_TEST_VECTOR_MS)
    ids = [uuid.UUID(bytes=bytes(buf[i * 16 : (i + 1) * 16])) for i in range(count)]
    assert all(i.version == 7 for i in ids)
    assert all(hyperuuid.v7_timestamp(i).timestamp() * 1000 == RFC_TEST_VECTOR_MS for i in ids)


def test_fill_v7_is_strictly_increasing():
    count = 256
    buf = bytearray(count * 16)
    hyperuuid.fill_v7(buf, RFC_TEST_VECTOR_MS)
    raw = [bytes(buf[i * 16 : (i + 1) * 16]) for i in range(count)]
    assert raw == sorted(raw)
    assert len(set(raw)) == count


def test_fill_v6_fills_the_callers_buffer():
    count = 32
    buf = bytearray(count * 16)
    hyperuuid.fill_v6(buf, RFC_TEST_VECTOR_MS)
    ids = [uuid.UUID(bytes=bytes(buf[i * 16 : (i + 1) * 16])) for i in range(count)]
    assert all(i.version == 6 for i in ids)


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
    with pytest.raises(ValueError):
        hyperuuid.fill_v7(bytearray(17), RFC_TEST_VECTOR_MS)
    with pytest.raises(ValueError):
        hyperuuid.fill_v6(bytearray(15), RFC_TEST_VECTOR_MS)


def test_fill_empty_buffer_is_a_no_op():
    hyperuuid.fill_v7(bytearray(0), RFC_TEST_VECTOR_MS)
    hyperuuid.fill_v6(bytearray(0), RFC_TEST_VECTOR_MS)


def test_fill_v7_rejects_an_out_of_range_timestamp():
    with pytest.raises(ValueError):
        hyperuuid.fill_v7(bytearray(16), 1 << 48)


def test_fill_v7_defaults_to_now():
    buf = bytearray(16)
    before = time.time()
    hyperuuid.fill_v7(buf)
    after = time.time()
    ts = hyperuuid.v7_timestamp(uuid.UUID(bytes=bytes(buf))).timestamp()
    assert before - 1 <= ts <= after + 1


def test_fill_fully_overwrites_a_reused_buffer():
    """The buffer-reuse case this API exists for: no byte of a previous fill may survive."""
    buf = bytearray(b"\xAA" * (8 * 16))
    hyperuuid.fill_v7(buf, RFC_TEST_VECTOR_MS)
    first = bytes(buf)
    assert b"\xAA" * 16 not in first
    hyperuuid.fill_v7(buf, RFC_TEST_VECTOR_MS)
    assert bytes(buf) != first, "second fill returned identical bytes"
    assert all(
        uuid.UUID(bytes=bytes(buf[i * 16 : (i + 1) * 16])).version == 7 for i in range(8)
    )
