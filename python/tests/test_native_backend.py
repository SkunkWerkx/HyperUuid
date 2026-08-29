"""Pins the native backend's contracts: the fastuuid-style constructor produces objects
indistinguishable from UUID(bytes=...)-constructed ones (the __slots__ invariant it leans
on), and exceptions match the documented types."""

import datetime
import uuid

import hyperuuid


def test_fast_constructed_uuids_are_indistinguishable():
    # The pin for the UUID.__new__ + object.__setattr__ fast path: every observable
    # surface of a natively built UUID must match a stdlib-constructed twin.
    minted = hyperuuid.new_v7()
    twin = uuid.UUID(bytes=minted.bytes)
    assert minted == twin
    assert hash(minted) == hash(twin)
    assert minted.int == twin.int
    assert minted.bytes == twin.bytes
    assert minted.version == 7
    assert minted.is_safe is uuid.SafeUUID.unknown
    assert str(minted) == str(twin)


def test_exception_types_are_correct():
    try:
        hyperuuid.new_v7(2**48)
    except ValueError:
        pass
    else:
        raise AssertionError("expected ValueError for a 49-bit timestamp")
    # A spec-valid v7 timestamp past year 9999 must raise OverflowError.
    beyond = hyperuuid.new_v7(2**48 - 1)
    try:
        hyperuuid.v7_timestamp(beyond)
    except OverflowError:
        pass
    else:
        raise AssertionError("expected OverflowError past datetime's ceiling")


def test_timestamp_datetimes_match_the_timedelta_construction():
    minted = hyperuuid.new_v7(1_645_557_742_123)
    recovered = hyperuuid.v7_timestamp(minted)
    epoch = datetime.datetime(1970, 1, 1, tzinfo=datetime.timezone.utc)
    assert recovered == epoch + datetime.timedelta(milliseconds=1_645_557_742_123)
    assert recovered.tzinfo is not None
