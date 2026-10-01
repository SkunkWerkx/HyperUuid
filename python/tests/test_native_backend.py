"""Pins the native backend's contracts: the run loads the backend it thinks it does, the
version probe names the core actually loaded, the fastuuid-style constructor produces objects
indistinguishable from UUID(bytes=...)-constructed ones (the __slots__ invariant it leans
on), and exceptions match the documented types. The file runs under ``HYPERUUID_WASM=1`` too,
where every pin but the first holds for the wasm backend just the same."""

import datetime
import importlib.metadata
import os
import re
import uuid
from pathlib import Path

import pytest

import hyperuuid


def test_the_run_loads_the_backend_it_was_asked_for():
    # The fallback is silent by design — an extension that fails to import hands over to the
    # wasm backend whenever wasmtime is installed — so without this a broken `_native` passes
    # the whole "native" run on the other backend and nobody finds out.
    expected = "wasm" if os.environ.get("HYPERUUID_WASM") else "native"
    assert hyperuuid.BACKEND == expected


def _expected_version() -> str:
    # The one version source is rust/Cargo.toml: maturin bakes it into the package metadata,
    # and the crate bakes it into hyperuuid_version — so the expectation is derived, never a
    # literal that goes stale on the next bump. The metadata is absent when the suite runs
    # off the source tree with no install, so fall back to the manifest itself.
    try:
        return importlib.metadata.version("hyperuuid")
    except importlib.metadata.PackageNotFoundError:
        pass
    for parent in Path(__file__).resolve().parents:
        manifest = parent / "rust" / "Cargo.toml"
        if manifest.is_file():
            found = re.search(r'^version\s*=\s*"([^"]+)"', manifest.read_text(encoding="utf-8"), re.MULTILINE)
            assert found, f"no version in {manifest}"
            return found.group(1)
    raise FileNotFoundError("rust/Cargo.toml not found")


def test_native_version_names_the_loaded_core():
    version = hyperuuid.native_version()
    assert re.fullmatch(r"\d+\.\d+\.\d+", version), version
    assert version == _expected_version()


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


# Every door that takes a timestamp, called with one. A timestamp no field can hold is one
# caller bug, so it is one exception — ValueError — whether the value merely overflows the
# field (the core's own check) or does not even fit the u64 the core takes (which used to
# surface as PyO3's OverflowError on one backend).
_TIMESTAMP_DOORS = [
    hyperuuid.new_v6,
    hyperuuid.new_v7,
    lambda millis: hyperuuid.new_v6_batch(2, millis),
    lambda millis: hyperuuid.new_v7_batch(2, millis),
    lambda millis: hyperuuid.fill_v6(bytearray(32), millis),
    lambda millis: hyperuuid.fill_v7(bytearray(32), millis),
]


@pytest.mark.parametrize("door", _TIMESTAMP_DOORS)
@pytest.mark.parametrize("millis", [-1, 2**63, 2**64 - 1, 2**64, 2**70])
def test_a_timestamp_out_of_range_is_always_a_value_error(door, millis):
    with pytest.raises(ValueError):
        door(millis)


@pytest.mark.parametrize("door", _TIMESTAMP_DOORS)
@pytest.mark.parametrize("millis", [1.5, "1645557742000", b"1", object()])
def test_a_timestamp_of_the_wrong_type_is_always_a_type_error(door, millis):
    # The wasm backend used to coerce these through int() — 1.5 minted at millisecond 1.
    with pytest.raises(TypeError):
        door(millis)


def test_an_empty_batch_or_fill_still_checks_its_timestamp():
    # The core checks the timestamp before it looks at the count, so "nothing to mint" is
    # not a way to slip a bad one through — on either backend.
    with pytest.raises(ValueError):
        hyperuuid.new_v7_batch(0, 2**48)
    with pytest.raises(ValueError):
        hyperuuid.new_v6_batch(0, 2**63)
    with pytest.raises(ValueError):
        hyperuuid.fill_v7(bytearray(0), 2**48)
    with pytest.raises(ValueError):
        hyperuuid.fill_v6(bytearray(0), 2**63)


@pytest.mark.parametrize("batch", [hyperuuid.new_v6_batch, hyperuuid.new_v7_batch])
@pytest.mark.parametrize("count", [-1, 2**32, 2**32 + 1, 2**60 + 1, 2**64])
def test_a_batch_count_out_of_range_is_refused_not_narrowed(batch, count):
    # The core's batch parameter is a u32. The extension used to take a usize and narrow it:
    # 2**32 + 1 minted one UUID and 2**60 + 1 (whose byte length wraps to 16) did too, each
    # returned as if it were the batch that was asked for.
    with pytest.raises(ValueError):
        batch(count, 1_645_557_742_000)


@pytest.mark.parametrize("batch", [hyperuuid.new_v6_batch, hyperuuid.new_v7_batch])
@pytest.mark.parametrize("count", [2.0, "2", None])
def test_a_batch_count_of_the_wrong_type_is_a_type_error(batch, count):
    with pytest.raises(TypeError):
        batch(count, 1_645_557_742_000)


def test_a_v5_name_that_is_neither_str_nor_bytes_is_a_type_error():
    for name in (1, 1.5, None, bytearray(b"name"), memoryview(b"name")):
        with pytest.raises(TypeError, match="must be str or bytes"):
            hyperuuid.new_v5(uuid.NAMESPACE_DNS, name)


def test_timestamp_datetimes_match_the_timedelta_construction():
    minted = hyperuuid.new_v7(1_645_557_742_123)
    recovered = hyperuuid.v7_timestamp(minted)
    epoch = datetime.datetime(1970, 1, 1, tzinfo=datetime.timezone.utc)
    assert recovered == epoch + datetime.timedelta(milliseconds=1_645_557_742_123)
    assert recovered.tzinfo is not None


def test_a_datetime_is_truncated_to_its_millisecond_never_rounded_into_the_next():
    # 999.6 µs short of the next millisecond: rounding (what float arithmetic did here) would
    # stamp the UUID *after* the moment it was asked for.
    base = datetime.datetime(2022, 2, 22, 19, 22, 22, 123000, tzinfo=datetime.timezone.utc)
    for micros in (0, 1, 499, 500, 999):
        asked = base + datetime.timedelta(microseconds=micros)
        assert hyperuuid.v7_timestamp(hyperuuid.new_v7(asked)) == base
        assert hyperuuid.v6_timestamp(hyperuuid.new_v6(asked)) == base


def test_a_naive_datetime_is_read_as_local_time():
    naive = datetime.datetime(2022, 2, 22, 19, 22, 22, 123000)
    assert hyperuuid.v7_timestamp(hyperuuid.new_v7(naive)) == naive.astimezone(datetime.timezone.utc)
