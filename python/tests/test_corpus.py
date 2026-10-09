"""Replays the shared conformance corpus (``corpus/*.json`` at the repository root), the same
files the Rust core's own suite replays, through this package's public API. Every value
crosses as a ``uuid.UUID`` the way a caller holds one: an RFC-ordered hex value as
``UUID(bytes=...)``, and a SQL-ordered one the same way, since ``v7_to_sql_order`` returns
the SQL Server wire bytes as the ``UUID``'s own ``bytes``. A vector that fails here is a break
in the cross-language contract, and for ``sql_order.json`` a change to data already persisted
in SQL Server."""

import datetime
import json
import uuid
from pathlib import Path

import pytest

import hyperuuid
from hyperuuid import Layout, Variant


def _corpus_directory() -> Path:
    for parent in Path(__file__).resolve().parents:
        corpus = parent / "corpus"
        if corpus.is_dir():
            return corpus
    raise FileNotFoundError(f"corpus directory not found above {Path(__file__).resolve()}")


CORPUS = _corpus_directory()

NAMESPACES = {
    "dns": uuid.NAMESPACE_DNS,
    "url": uuid.NAMESPACE_URL,
    "oid": uuid.NAMESPACE_OID,
    "x500": uuid.NAMESPACE_X500,
}
LAYOUTS = {"rfc9562": Layout.RFC9562, "sql_server": Layout.SQL_SERVER}
VARIANTS = {
    "ncs": Variant.NCS,
    "rfc9562": Variant.RFC9562,
    "microsoft": Variant.MICROSOFT,
    "future": Variant.FUTURE,
}

_EPOCH = datetime.datetime(1970, 1, 1, tzinfo=datetime.timezone.utc)
# datetime's ceiling, 9999-12-31T23:59:59.999 UTC, as Unix-epoch milliseconds.
_MAX_DATETIME_MILLIS = (datetime.datetime.max.replace(tzinfo=datetime.timezone.utc) - _EPOCH) // (
    datetime.timedelta(milliseconds=1)
)


def _corpus(name: str) -> list[dict]:
    return json.loads((CORPUS / name).read_text(encoding="utf-8"))


def _uuid(hex_digits: str) -> uuid.UUID:
    return uuid.UUID(bytes=bytes.fromhex(hex_digits))


def _ids(vectors: list[dict]) -> list[str]:
    return [json.dumps(vector, sort_keys=True) for vector in vectors]


V5 = _corpus("v5.json")
SQL_ORDER = _corpus("sql_order.json")
TIMESTAMP = _corpus("timestamp.json")
INSPECT = _corpus("inspect.json")


@pytest.mark.parametrize("vector", V5, ids=_ids(V5))
def test_v5_corpus(vector):
    """v5 matches the oracle through the bytes door, and through the str door when the name is
    text."""
    namespace = NAMESPACES[vector["namespace"]]
    expected = _uuid(vector["expect"])
    assert hyperuuid.new_v5(namespace, bytes.fromhex(vector["name_hex"])) == expected
    if "name" in vector:
        assert hyperuuid.new_v5(namespace, vector["name"]) == expected


@pytest.mark.parametrize("vector", SQL_ORDER, ids=_ids(SQL_ORDER))
def test_sql_order_corpus(vector):
    """Both directions of the SQL Server permutation, and the wire bytes a caller sends."""
    rfc, sql = _uuid(vector["rfc"]), _uuid(vector["sql"])
    to_sql, from_sql = {
        6: (hyperuuid.v6_to_sql_order, hyperuuid.v6_from_sql_order),
        7: (hyperuuid.v7_to_sql_order, hyperuuid.v7_from_sql_order),
    }[vector["version"]]
    assert to_sql(rfc) == sql
    assert from_sql(sql) == rfc
    # What reaches SQL Server is the UUID's bytes; they must be the corpus's wire bytes.
    assert to_sql(rfc).bytes == bytes.fromhex(vector["sql"])


@pytest.mark.parametrize("vector", TIMESTAMP, ids=_ids(TIMESTAMP))
def test_timestamp_corpus(vector):
    """Every timestamp door, in the vector's layout and, for RFC rows, without one."""
    layout = LAYOUTS[vector["layout"]]
    id_ = _uuid(vector["uuid"])
    version, millis = vector["version"], vector["unix_millis"]
    rfc = layout is Layout.RFC9562
    assert hyperuuid.version(id_, layout) == version

    if millis is not None and millis > _MAX_DATETIME_MILLIS:
        # A spec-valid v7 timestamp past year 9999 has no datetime: the datetime doors raise
        # OverflowError for it (documented on v7_timestamp) while the millisecond doors read it.
        assert version == 7
        assert hyperuuid.v7_unix_millis(id_, layout) == millis
        with pytest.raises(OverflowError):
            hyperuuid.get_timestamp(id_, layout)
        with pytest.raises(OverflowError):
            hyperuuid.v7_timestamp(id_, layout)
        if rfc:
            assert hyperuuid.v7_unix_millis(id_) == millis
            with pytest.raises(OverflowError):
                hyperuuid.get_timestamp(id_)
            with pytest.raises(OverflowError):
                hyperuuid.v7_timestamp(id_)
        return

    expected = None if millis is None else _EPOCH + datetime.timedelta(milliseconds=millis)
    assert hyperuuid.get_timestamp(id_, layout) == expected
    if rfc:
        assert hyperuuid.get_timestamp(id_) == expected
    if millis is None:
        return
    unix_millis, timestamp = {
        6: (hyperuuid.v6_unix_millis, hyperuuid.v6_timestamp),
        7: (hyperuuid.v7_unix_millis, hyperuuid.v7_timestamp),
    }[version]
    assert unix_millis(id_, layout) == millis
    assert timestamp(id_, layout) == expected
    if rfc:
        assert unix_millis(id_) == millis
        assert timestamp(id_) == expected


@pytest.mark.parametrize("vector", INSPECT, ids=_ids(INSPECT))
def test_inspect_corpus(vector):
    """version and is_rfc in the vector's layout, is_rfc false for every other version, and in
    RFC layout the variant and the no-layout forms."""
    layout = LAYOUTS[vector["layout"]]
    id_ = _uuid(vector["uuid"])
    version, rfc = vector["version"], vector["is_rfc"]

    assert hyperuuid.version(id_, layout) == version
    assert hyperuuid.is_rfc(id_, version, layout) is rfc
    for other in range(16):
        if other != version:
            assert hyperuuid.is_rfc(id_, other, layout) is False, other

    if "variant" not in vector:
        return
    assert hyperuuid.variant(id_) is VARIANTS[vector["variant"]]
    assert hyperuuid.version(id_) == version
    assert hyperuuid.is_rfc(id_, version) is rfc
