"""Pins the native backend's contracts: the fastuuid-style constructor produces objects
indistinguishable from UUID(bytes=...)-constructed ones (the __slots__ invariant it leans
on), both backends agree on values and exception types, and the backend actually in use is
the one this suite thinks it's testing."""

import datetime
import os
import pathlib
import subprocess
import sys
import uuid

import hyperuuid


def test_backend_marker_is_accurate():
    expected = "ctypes" if os.environ.get("HYPERUUID_PURE") else "native"
    assert hyperuuid.BACKEND == expected


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


def test_deterministic_surfaces_agree_across_backends():
    if hyperuuid.BACKEND != "native":
        return  # the comparison below spawns the pure backend; running it from pure is circular
    script = (
        "import hyperuuid, uuid, json;"
        "v5 = hyperuuid.new_v5(uuid.NAMESPACE_DNS, 'www.example.com');"
        "v7 = hyperuuid.new_v7(1645557742000);"
        "print(json.dumps([str(v5), str(hyperuuid.v7_to_sql_order(v7)), "
        "hyperuuid.v7_timestamp(v7).isoformat()]))"
    )
    src = str(pathlib.Path(__file__).resolve().parent.parent / "src")
    pure = subprocess.run(
        [sys.executable, "-c", script],
        capture_output=True, text=True, check=True,
        env={**os.environ, "HYPERUUID_PURE": "1", "PYTHONPATH": src},
    ).stdout
    native = subprocess.run(
        [sys.executable, "-c", script],
        capture_output=True, text=True, check=True,
        env={**{k: v for k, v in os.environ.items() if k != "HYPERUUID_PURE"}, "PYTHONPATH": src},
    ).stdout
    # v5 and the timestamp are fully deterministic; the sql-order round shape is too
    # (same v7 timestamp instant, though different random bits per process — compare the
    # deterministic fields only).
    import json

    pure_v5, _, pure_ts = json.loads(pure)
    native_v5, _, native_ts = json.loads(native)
    assert pure_v5 == native_v5
    assert pure_ts == native_ts


def test_exception_types_match_the_ctypes_contract():
    try:
        hyperuuid.new_v7(2**48)
    except ValueError:
        pass
    else:
        raise AssertionError("expected ValueError for a 49-bit timestamp")
    # A spec-valid v7 timestamp past year 9999 must raise OverflowError, matching the
    # documented ctypes behavior.
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
