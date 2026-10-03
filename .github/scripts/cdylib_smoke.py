"""Loads the shared library every FFI binding loads and calls all thirteen C exports once.

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
u8p, u32, u64, i32 = ctypes.c_char_p, ctypes.c_uint32, ctypes.c_uint64, ctypes.c_int32
for name, restype, argtypes in [
    ("hyperuuid_version", u32, []),
    ("uuid_new_v4", i32, [u8p]),
    ("uuid_new_v5", i32, [u8p, u8p, u32, u8p]),
    ("uuid_new_v6", i32, [u64, u8p]),
    ("uuid_new_v6_batch", i32, [u64, u32, u8p]),
    ("uuid_v6_unix_millis", u64, [u8p]),
    ("uuid_v6_to_sql_order", None, [u8p]),
    ("uuid_v6_to_rfc_order", None, [u8p]),
    ("uuid_new_v7", i32, [u64, u8p]),
    ("uuid_new_v7_batch", i32, [u64, u32, u8p]),
    ("uuid_v7_unix_millis", u64, [u8p]),
    ("uuid_v7_to_sql_order", None, [u8p]),
    ("uuid_v7_to_rfc_order", None, [u8p]),
]:
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
for v_ in (6, 7):
    new, batch, millis = (getattr(lib, f"uuid_new_v{v_}"), getattr(lib, f"uuid_new_v{v_}_batch"),
                          getattr(lib, f"uuid_v{v_}_unix_millis"))
    check(new(ms, out) == 0 and uuid.UUID(bytes=out.raw).version == v_, f"uuid_new_v{v_}")
    check(millis(out) == ms, f"uuid_v{v_}_unix_millis")
    before = out.raw
    getattr(lib, f"uuid_v{v_}_to_sql_order")(out)
    getattr(lib, f"uuid_v{v_}_to_rfc_order")(out)
    check(out.raw == before, f"v{v_} SQL order round trip")
    many = ctypes.create_string_buffer(16 * 1000)
    check(batch(ms, 1000, many) == 0, f"uuid_new_v{v_}_batch failed")
    ids = [many.raw[i:i + 16] for i in range(0, 16000, 16)]
    check(len(set(ids)) == 1000, f"uuid_new_v{v_}_batch repeated an id")

print(f"cdylib smoke: {lib_path} {got}, all 13 exports called")
