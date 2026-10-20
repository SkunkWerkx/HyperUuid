"""The independent oracle behind corpus/*.json. It shares no code with the Rust core.

- v5 comes from hashlib.
- The SQL Server layout is derived in bit strings from RFC 9562's field tables: a v7's free
  bits, in order, poured into SqlGuid's compare slots around a version 8 nibble and the
  variant. The core shifts integers instead. The result is also checked against a
  line-for-line port of Norse Svartalfheim's SequentialGuidBytes (the permutation
  SequentialGuid also uses), which works in .NET's mixed-endian Guid layout and keeps a 7
  where this layout writes the 8.
- The v7 generation layout comes from RFC 9562's prose.

By default this re-derives every file and fails if any of them differs from what the
oracle computes, so a hand edit that changes an `expect` is caught here as well as in
the replays. `--write` rewrites the files; after that, `git diff` should show only
added vectors (corpus/README.md says why).

No dependencies: python3 corpus/oracle.py [--write]
"""
import hashlib, json, os, sys

out_dir = os.path.dirname(os.path.abspath(__file__))
WRITE = "--write" in sys.argv[1:]
STALE = []

NS = {
    "dns": bytes.fromhex("6ba7b8109dad11d180b400c04fd430c8"),
    "url": bytes.fromhex("6ba7b8119dad11d180b400c04fd430c8"),
    "oid": bytes.fromhex("6ba7b8129dad11d180b400c04fd430c8"),
    "x500": bytes.fromhex("6ba7b8149dad11d180b400c04fd430c8"),
}

def v5(ns, name):
    b = bytearray(hashlib.sha1(ns + name).digest()[:16])
    b[6] = (b[6] & 0x0F) | 0x50
    b[8] = (b[8] & 0x3F) | 0x80
    return bytes(b)

def v7_layout(ms, counter, entropy):
    c = counter & 0x3FFFFFF
    b = bytearray(16)
    b[0:6] = ms.to_bytes(6, "big")
    b[6] = 0x70 | (c >> 22)
    b[7] = (c >> 14) & 0xFF
    b[8] = 0x80 | ((c >> 8) & 0x3F)
    b[9] = c & 0xFF
    b[10:16] = entropy
    return bytes(b)

def native_of(rfc):
    """System.Guid(rfc, bigEndian: true).TryWriteBytes(native) — .NET's mixed-endian layout."""
    return bytes([rfc[3], rfc[2], rfc[1], rfc[0], rfc[5], rfc[4], rfc[7], rfc[6]]) + rfc[8:]

def prior_art_v7_to_sql(rfc):
    native = native_of(rfc)
    counter_hi = ((native[7] & 0x0F) << 8) | native[6]
    counter_lo = ((native[8] & 0x3F) << 8) | native[9]
    counter = (counter_hi << 14) | counter_lo
    top14 = (counter >> 12) & 0x3FFF
    bottom12 = counter & 0xFFF
    version = native[7] & 0xF0
    variant = native[8] & 0xC0
    sql = bytearray(16)
    sql[10], sql[11], sql[12], sql[13] = native[3], native[2], native[1], native[0]
    sql[14], sql[15] = native[5], native[4]
    sql[8] = variant | ((top14 >> 8) & 0x3F)
    sql[9] = top14 & 0xFF
    sql[6] = (bottom12 >> 4) & 0xFF
    sql[7] = version | (bottom12 & 0x0F)
    sql[4], sql[5] = native[10], native[11]
    sql[0:4] = native[12:16]
    return bytes(sql)  # new Guid(sql).ToByteArray() == sql

# ---- SQL Server layout, RFC 9562 version 8 (§5.8; §6.11 sends other byte orders here)
#
# SqlGuid compares wire octets 10-15, 8-9, 6-7, 4-5, 0-3, most significant first. The version
# nibble (8) and variant (10) sit where a mixed-endian Guid holds them: octet 7's top nibble,
# octet 8's top two bits. The 122 bits left fill those slots in compare order: 48 in 10-15,
# 14 after the variant in 8-9, 12 in 6 and under the version in 7, 48 in 4-5 then 0-3.

def bits(b): return "".join(format(x, "08b") for x in b)
def unbits(s): return int(s, 2).to_bytes(len(s) // 8, "big")

def sql_from_key(key):
    assert len(key) == 122
    sql = bytearray(16)
    sql[10:16] = unbits(key[0:48])
    sql[8:10] = unbits("10" + key[48:62])
    sql[6:8] = unbits(key[62:70] + "1000" + key[70:74])
    sql[4:6] = unbits(key[74:90])
    sql[0:4] = unbits(key[90:122])
    return bytes(sql)

def key_from_sql(sql):
    b = {i: bits(sql[i:i + 1]) for i in range(16)}
    return (b[10] + b[11] + b[12] + b[13] + b[14] + b[15] + b[8][2:] + b[9] + b[6] + b[7][4:]
            + b[4] + b[5] + b[0] + b[1] + b[2] + b[3])

# RFC 9562's v7 field table as bit offsets: unix_ts_ms 0-47, ver 48-51, rand_a 52-63,
# var 64-65, rand_b 66-127. Every bit but the version and variant is free, 122 of them.
def v7_to_sql(rfc):
    r = bits(rfc)
    assert r[48:52] == "0111" and r[64:66] == "10"
    return sql_from_key(r[0:48] + r[52:64] + r[66:128])

def sql_to_rfc(sql):
    k = key_from_sql(sql)
    return unbits(k[0:48] + "0111" + k[48:60] + "10" + k[60:122])

SIGNIFICANCE = [10, 11, 12, 13, 14, 15, 8, 9, 6, 7, 4, 5, 0, 1, 2, 3]
def sql_key(b):
    return bytes(b[i] for i in SIGNIFICANCE)

def v7_millis(b): return int.from_bytes(b[0:6], "big")

h = lambda b: b.hex()

def dump(name, rows):
    text = "[\n" + ",\n".join("  " + json.dumps(r, ensure_ascii=False) for r in rows) + "\n]\n"
    path = os.path.join(out_dir, name)
    if WRITE:
        with open(path, "w", encoding="utf-8") as f:
            f.write(text)
    else:
        try:
            with open(path, encoding="utf-8") as f:
                current = f.read()
        except FileNotFoundError:
            current = None
        if current != text:
            STALE.append(name)

# ---- v5
names = [
    ("dns", "www.example.com"),     # RFC 9562 Appendix A.4
    ("dns", "python.org"),          # Python uuid docs
    ("dns", ""),
    ("url", "https://example.com/"),
    ("oid", "1.3.6.1"),
    ("x500", "CN=example"),
    ("dns", "naïve café ☕ 𝄞"),       # 2-, 3- and 4-byte UTF-8
    ("dns", "a" * 255), ("dns", "a" * 256), ("dns", "a" * 257),  # stack-buffer threshold
    ("dns", "é" * 150),             # 300 UTF-8 bytes from 150 chars
    ("url", "x" * 1000),
]
rows = []
for ns, name in names:
    raw = name.encode("utf-8")
    rows.append({"namespace": ns, "name": name, "name_hex": raw.hex(), "expect": h(v5(NS[ns], raw))})
for ns, raw in [("dns", bytes([0xFF, 0xFE, 0x00, 0x80])), ("dns", bytes(range(64)))]:
    rows.append({"namespace": ns, "name_hex": raw.hex(), "expect": h(v5(NS[ns], raw))})
assert rows[0]["expect"] == "2ed6657de927568b95e12665a8aea6a2"
assert rows[1]["expect"] == "886313e13b8a53729b900c9aee199e5d"
dump("v5.json", rows)

# ---- v7 layout
ms_values = [0, 1, 1_645_557_742_000, 1_750_000_000_123, 1_800_000_000_000, 0xFFFF_FFFF_FFFF]
counters = [0, 7, 42, 0xFF, 0x100, 0xFFF, 0x1000, 0x2A_BCDE, 0x3FF_FFFF]
entropies = [bytes(6), bytes([1, 2, 3, 4, 5, 6]), bytes([9, 8, 7, 6, 5, 4]), b"\xff" * 6]
v7_rows = []
for i, ms in enumerate(ms_values):
    for j, c in enumerate(counters):
        e = entropies[(i + j) % len(entropies)]
        v7_rows.append({"unix_millis": ms, "counter": c, "entropy": h(e), "expect": h(v7_layout(ms, c, e))})
assert v7_layout(1_645_557_742_000, 0xCC3 << 14 | 0x18C4, bytes.fromhex("dc0c0c07398f")).hex() \
    == "017f22e279b07cc398c4dc0c0c07398f"  # RFC 9562 Appendix A.6
dump("v7_layout.json", v7_rows)

# ---- SQL order
sql_rows = []
for r in v7_rows:
    rfc = bytes.fromhex(r["expect"])
    sql_rows.append({"rfc": h(rfc), "sql": h(v7_to_sql(rfc))})
for r in sql_rows:
    rfc, sql = bytes.fromhex(r["rfc"]), bytes.fromhex(r["sql"])
    assert sql_to_rfc(sql) == rfc, r
    # Read as a .NET Guid (or SQL Server's text), every SQL-ordered value is an RFC v8.
    guid = native_of(sql)
    assert guid[6] >> 4 == 8 and guid[8] & 0xC0 == 0x80, r
    # The prior art's permutation, with the 8 in place of its 7.
    prior = bytearray(prior_art_v7_to_sql(rfc))
    prior[7] = 0x80 | (prior[7] & 0x0F)
    assert bytes(prior) == sql, r
# Sanity: SQL order sorts by (millis, counter).
v7s = sorted(v7_rows, key=lambda r: (r["unix_millis"], r["counter"]))
keys = [sql_key(v7_to_sql(bytes.fromhex(r["expect"]))) for r in v7s]
assert keys == sorted(keys)
dump("sql_order.json", sql_rows)

# ---- SQL order, refused: anything but a v7 going in, anything but the layout coming back
refused = []
seven = bytes.fromhex(v7_rows[20]["expect"])
seven_sql = v7_to_sql(seven)
cleared = bytearray(seven_sql); cleared[8] &= 0x3F
with_a_seven = bytearray(seven_sql); with_a_seven[7] = 0x70 | (with_a_seven[7] & 0x0F)  # 0.8.0's form
for direction, u, note in [
    ("to_sql", "1ec9414c232a6b00b3c89f6bdeced846", "a v6, RFC A.5"),
    ("to_sql", "919108f752d143209bacf847db4148a8", "a v4, RFC A.3"),
    ("to_sql", "00000000000000000000000000000000", "nil"),
    ("to_sql", "ffffffffffffffffffffffffffffffff", "max"),
    ("to_sql", seven_sql, "already SQL-ordered"),
    ("from_sql", seven, "an RFC-ordered v7"),
    ("from_sql", bytes(cleared), "a SQL-ordered v7 with its variant cleared"),
    ("from_sql", bytes(with_a_seven), "0.8.0's SQL order, a 7 where the 8 now is"),
    ("from_sql", "00000000000000000000000000000000", "nil"),
]:
    u = u if isinstance(u, str) else h(u)
    refused.append({"direction": direction, "uuid": u, "note": note})
dump("sql_order_refused.json", refused)

# ---- timestamp (RFC order; layout-aware rows join once the core has them)
ts = []
ts.append({"uuid": "017f22e279b07cc398c4dc0c0c07398f", "version": 7, "unix_millis": 1_645_557_742_000})  # RFC A.6
for r in v7_rows[::5]:
    ts.append({"uuid": r["expect"], "version": 7, "unix_millis": r["unix_millis"]})
# Only a version 7 carries a timestamp this library reads; v1 and v6 have one, and get none.
for u, v in [("00000000000000000000000000000000", 0), ("ffffffffffffffffffffffffffffffff", 15),
             ("919108f752d143209bacf847db4148a8", 4), (rows[0]["expect"], 5),
             ("c232ab00941411ecb3c89f6bdeced846", 1),     # RFC A.1
             ("1ec9414c232a6b00b3c89f6bdeced846", 6),     # RFC A.5
             # A 7 nibble under another variant is no RFC version, so no timestamp.
             ("017f22e279b07cc368c4dc0c0c07398f", 7),   # NCS (0xxx)
             ("017f22e279b07cc3d8c4dc0c0c07398f", 7),   # Microsoft (110x)
             ("017f22e279b07cc3f8c4dc0c0c07398f", 7)]:  # Future (111x)
    ts.append({"uuid": u, "version": v, "unix_millis": None})
for r in ts:
    r["layout"] = "rfc9562"
for r in ts:
    b = bytes.fromhex(r["uuid"])
    assert b[6] >> 4 == r["version"], r
    if r["unix_millis"] is not None:
        assert r["version"] == 7 and v7_millis(b) == r["unix_millis"], r

def is_rfc(b, v): return (b[8] & 0xC0) == 0x80 and b[6] >> 4 == v

def sql_version(sql):
    """The version a SQL-ordered value came from: 7 for an RFC v8 in Guid order, else 0."""
    return 7 if is_rfc(native_of(sql), 8) else 0

def variant(b):
    t = b[8] >> 5
    return "ncs" if t <= 3 else "rfc9562" if t <= 5 else "microsoft" if t == 6 else "future"

for r in sql_rows:
    sql = bytes.fromhex(r["sql"])
    assert sql_version(sql) == 7, r
    ts.append({"uuid": r["sql"], "layout": "sql_server", "version": 7,
               "unix_millis": v7_millis(bytes.fromhex(r["rfc"]))})
others = ["00000000000000000000000000000000", "ffffffffffffffffffffffffffffffff",
          "919108f752d143209bacf847db4148a8", "017f22e279b07cc398c4dc0c0c07398f",
          "1ec9414c232a6b00b3c89f6bdeced846"]
# A SQL-ordered v7 with its variant bits cleared: no longer a SQL-ordered RFC v7.
broken = bytearray.fromhex(next(r["sql"] for r in sql_rows if int(r["rfc"][:12], 16) == 1_645_557_742_000))
broken[8] &= 0x3F
others.append(broken.hex())
others.append(h(with_a_seven))  # 0.8.0's SQL order
for u in others:
    assert sql_version(bytes.fromhex(u)) == 0
    ts.append({"uuid": u, "layout": "sql_server", "version": 0, "unix_millis": None})
dump("timestamp.json", ts)

# ---- inspect
samples = [
    ("00000000000000000000000000000000", "nil"), ("ffffffffffffffffffffffffffffffff", "max"),
    ("c232ab00941411ecb3c89f6bdeced846", "v1, RFC A.1"), ("919108f752d143209bacf847db4148a8", "v4, RFC A.3"),
    (rows[0]["expect"], "v5, RFC A.4"), ("1ec9414c232a6b00b3c89f6bdeced846", "v6, RFC A.5"),
    ("017f22e279b07cc398c4dc0c0c07398f", "v7, RFC A.6"), ("320c3d4dcc00875b8ec932d5f69181c0", "v8, RFC B.1"),
    ("919108f752d1432019acf847db4148a8", "v4 nibble, NCS variant (0xxx)"),
    ("919108f752d14320d9acf847db4148a8", "v4 nibble, Microsoft variant (110x)"),
    ("919108f752d14320f9acf847db4148a8", "v4 nibble, Future variant (111x)"),
    ("017f22e279b07cc3b8c4dc0c0c07398f", "v7, variant 1011 (still RFC)"),
    ("017f22e279b07cc368c4dc0c0c07398f", "v7 nibble, NCS variant: not an RFC v7"),
]
inspect = []
for u, note in samples:
    b = bytes.fromhex(u)
    inspect.append({"uuid": u, "layout": "rfc9562", "version": b[6] >> 4, "variant": variant(b),
                    "is_rfc": (b[8] & 0xC0) == 0x80, "note": note})
for r in sql_rows[::4]:
    inspect.append({"uuid": r["sql"], "layout": "sql_server", "version": 7, "is_rfc": True,
                    "note": "SQL-ordered v7"})
for u in others:
    inspect.append({"uuid": u, "layout": "sql_server", "version": 0, "is_rfc": False, "note": "not a SQL-ordered v7"})
# RFC-ordered bytes can carry the 8 and the variant where SQL order puts them, as RFC A.4's v5
# does; the layout is the caller's to track, so this reads as a SQL-ordered v7.
assert sql_version(bytes.fromhex(rows[0]["expect"])) == 7
inspect.append({"uuid": rows[0]["expect"], "layout": "sql_server", "version": 7, "is_rfc": True,
                "note": "v5 RFC A.4, RFC-ordered, read as SQL order: its octets form a SQL-ordered v7"})
dump("inspect.json", inspect)
if STALE:
    sys.exit("corpus differs from the oracle: " + ", ".join(STALE))
print(("wrote" if WRITE else "ok:") + " v5 %d, v7_layout %d, sql_order %d, sql_order_refused %d, timestamp %d, inspect %d"
      % (len(rows), len(v7_rows), len(sql_rows), len(refused), len(ts), len(inspect)))
