"""What a consumer writes, type-checked by ``tests/test_typing.py`` (never imported or run).

``assert_type`` fails the check when the checker sees ``Any`` — which is what every call here
was before the package shipped ``py.typed`` and a stub for the extension module.
"""

import datetime
import uuid
from typing import assert_type

import hyperuuid

assert_type(hyperuuid.BACKEND, str)
assert_type(hyperuuid.native_version(), str)
assert_type(hyperuuid.NIL, uuid.UUID)

assert_type(hyperuuid.new_v4(), uuid.UUID)
assert_type(hyperuuid.new_v5(uuid.NAMESPACE_DNS, "example.com"), uuid.UUID)
assert_type(hyperuuid.new_v5(uuid.NAMESPACE_DNS, b"example.com"), uuid.UUID)
assert_type(hyperuuid.new_v6(), uuid.UUID)
assert_type(hyperuuid.new_v7(1_645_557_742_000), uuid.UUID)
assert_type(hyperuuid.new_v7(datetime.datetime.now(datetime.timezone.utc)), uuid.UUID)
assert_type(hyperuuid.new_v6_batch(8), list[uuid.UUID])
assert_type(hyperuuid.new_v7_batch(8, 1_645_557_742_000), list[uuid.UUID])

minted = hyperuuid.new_v7()
assert_type(hyperuuid.v7_timestamp(minted), datetime.datetime)
assert_type(hyperuuid.v7_unix_millis(minted), int)
assert_type(hyperuuid.v6_timestamp(minted), datetime.datetime)
assert_type(hyperuuid.get_timestamp(minted), datetime.datetime | None)
assert_type(hyperuuid.v7_from_sql_order(hyperuuid.v7_to_sql_order(minted)), uuid.UUID)
assert_type(hyperuuid.v6_from_sql_order(hyperuuid.v6_to_sql_order(minted)), uuid.UUID)

buffer = bytearray(16 * 8)
assert_type(hyperuuid.fill_v7(buffer), None)
assert_type(hyperuuid.fill_v6(buffer, 1_645_557_742_000), None)

stored = hyperuuid.v7_to_sql_order(minted)
assert_type(hyperuuid.MAX_V7_BATCH, int)
assert_type(hyperuuid.version(minted), int)
assert_type(hyperuuid.version(stored, hyperuuid.Layout.SQL_SERVER), int)
assert_type(hyperuuid.variant(minted), hyperuuid.Variant)
assert_type(hyperuuid.is_rfc(minted, 7), bool)
assert_type(hyperuuid.is_rfc(stored, 7, hyperuuid.Layout.SQL_SERVER), bool)
assert_type(hyperuuid.v7_unix_millis(stored, hyperuuid.Layout.SQL_SERVER), int)
assert_type(hyperuuid.v6_unix_millis(stored, hyperuuid.Layout.SQL_SERVER), int)
assert_type(hyperuuid.v7_timestamp(stored, hyperuuid.Layout.SQL_SERVER), datetime.datetime)
assert_type(hyperuuid.v6_timestamp(stored, hyperuuid.Layout.SQL_SERVER), datetime.datetime)
assert_type(hyperuuid.get_timestamp(stored, hyperuuid.Layout.SQL_SERVER), datetime.datetime | None)
