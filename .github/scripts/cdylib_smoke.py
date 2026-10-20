"""Loads the shared library every FFI binding loads and calls every C export once.

ci.yml's check-cdylib job runs this against a freshly built no_std library on every platform
the forge ships: a library that links but will not load, or loads but misbehaves at the ABI,
fails here before any binding sees it. ctypes because Python is on every runner and opens
the library the way a host does (dlopen, LoadLibrary), with nothing compiled in between.

usage: python cdylib_smoke.py <path-to-library> <rust/Cargo.toml>
"""

import ctypes
import re
import sys
import uuid

lib_path, manifest = sys.argv[1], sys.argv[2]
want = re.search(r'^version = "([^"]+)"', open(manifest, encoding="utf-8").read(), re.M)[1]

lib = ctypes.CDLL(lib_path)
names = []
u8p, u32, u64, i32 = ctypes.c_char_p, ctypes.c_uint32, ctypes.c_uint64, ctypes.c_int32
for name, restype, argtypes in [
    ("hyperuuid_version", u32, []),
    ("uuid_new_v4", i32, [u8p]),
    ("uuid_new_v5", i32, [u8p, u8p, u32, u8p]),
    ("uuid_new_v7", i32, [u64, u8p]),
    ("uuid_new_v7_batch", i32, [u64, u32, u8p]),
    ("uuid_v7_unix_millis", u64, [u8p]),
    ("uuid_v7_to_sql_order", i32, [u8p]),
    ("uuid_v7_to_rfc_order", i32, [u8p]),
    ("uuid_version", u32, [u8p, u32]),
    ("uuid_variant", u32, [u8p]),
    ("uuid_is_rfc", u32, [u8p, u32, u32]),
    ("uuid_get_timestamp", u32, [u8p, u32, ctypes.POINTER(u64)]),
    ("uuid_v7_unix_millis_in", u64, [u8p, u32]),
]:
    names.append(name)
    fn = getattr(lib, name)
    fn.restype, fn.argtypes = restype, argtypes


def check(cond, what):
    if not cond:
        sys.exit(f"cdylib smoke: {what}")


v = lib.hyperuuid_version()
got = f"{v >> 16}.{(v >> 8) & 0xFF}.{v & 0xFF}"
check(got == want, f"hyperuuid_version reports {got}, rust/Cargo.toml says {want}")

out = ctypes.create_string_buffer(16)
check(lib.uuid_new_v4(out) == 0 and uuid.UUID(bytes=out.raw).version == 4, "uuid_new_v4")

name = b"www.example.com"
check(lib.uuid_new_v5(uuid.NAMESPACE_DNS.bytes, name, len(name), out) == 0, "uuid_new_v5 failed")
check(uuid.UUID(bytes=out.raw) == uuid.uuid5(uuid.NAMESPACE_DNS, name.decode()), "uuid_new_v5 vector")

ms = 1645557742000  # RFC 9562 Appendix A's timestamp
check(lib.uuid_new_v7(ms, out) == 0 and uuid.UUID(bytes=out.raw).version == 7, "uuid_new_v7")
check(lib.uuid_v7_unix_millis(out) == ms, "uuid_v7_unix_millis")
before = out.raw
check(lib.uuid_v7_to_sql_order(out) == 0, "uuid_v7_to_sql_order failed")
# SQL order is an RFC 9562 version 8, read in Guid (mixed-endian) order.
check(uuid.UUID(bytes_le=out.raw).version == 8, "SQL-ordered v7 is not a v8")
check(lib.uuid_v7_to_rfc_order(out) == 0, "uuid_v7_to_rfc_order failed")
check(out.raw == before, "v7 SQL order round trip")
# Each direction refuses what isn't its input (code 5) and leaves the bytes alone. Fixed values,
# because a random v7's octet 7 carries the SQL layout's 8 one time in 16.
v7 = bytes.fromhex("017f22e279b07cc398c4dc0c0c07398f")  # RFC 9562 Appendix A.6
seven = ctypes.create_string_buffer(v7, 16)
check(lib.uuid_v7_to_rfc_order(seven) == 5 and seven.raw == v7, "uuid_v7_to_rfc_order took an RFC v7")
v6 = bytes.fromhex("1ec9414c232a6b00b3c89f6bdeced846")  # RFC 9562 Appendix A.5
six = ctypes.create_string_buffer(v6, 16)
check(lib.uuid_v7_to_sql_order(six) == 5 and six.raw == v6, "uuid_v7_to_sql_order took a v6")
check(lib.uuid_version(out, 1) == 7 and lib.uuid_is_rfc(out, 7, 1) == 1, "v7 inspection")
check(lib.uuid_variant(out) == 2, "v7 variant")
check(lib.uuid_v7_to_sql_order(out) == 0, "uuid_v7_to_sql_order failed")
check(lib.uuid_version(out, 2) == 7 and lib.uuid_is_rfc(out, 7, 2) == 1, "SQL-ordered v7 inspection")
check(lib.uuid_v7_unix_millis_in(out, 2) == ms, "uuid_v7_unix_millis_in")
got_ms = u64(0)
check(lib.uuid_get_timestamp(out, 2, ctypes.byref(got_ms)) == 7 and got_ms.value == ms,
      "uuid_get_timestamp on SQL-ordered v7")
check(lib.uuid_get_timestamp(six, 1, ctypes.byref(got_ms)) == 0, "uuid_get_timestamp read a v6")
many = ctypes.create_string_buffer(16 * 1000)
check(lib.uuid_new_v7_batch(ms, 1000, many) == 0, "uuid_new_v7_batch failed")
ids = [many.raw[i:i + 16] for i in range(0, 16000, 16)]
check(len(set(ids)) == 1000, "uuid_new_v7_batch repeated an id")

# Past the 26-bit counter space a v7 batch is refused (code 4) before anything is written.
check(lib.uuid_new_v7_batch(ms, (1 << 26) + 1, out) == 4, "oversized v7 batch not refused")

print(f"cdylib smoke: {lib_path} {got}, all {len(names)} exports called")
