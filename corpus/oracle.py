"""The independent oracle behind corpus/*.json. It shares no code with the Rust core.

- v5 comes from hashlib.
- The v7 SQL Server permutation is a line-for-line port of Norse Svartalfheim's
  SequentialGuidBytes, which works in .NET's mixed-endian Guid layout, so it is a
  different derivation from the core's.
- The v6 permutation and both layouts come from RFC 9562's prose.

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

GREG = 0x01B21DD213814000

def v6_layout(ms, clock_seq, node):
    ticks = ms * 10_000 + GREG
    b = bytearray(16)
    b[0:4] = (ticks >> 28).to_bytes(4, "big")
    b[4:6] = ((ticks >> 12) & 0xFFFF).to_bytes(2, "big")
    b[6:8] = (0x6000 | (ticks & 0x0FFF)).to_bytes(2, "big")
    b[8:10] = (0x8000 | (clock_seq & 0x3FFF)).to_bytes(2, "big")
    b[10:16] = node
    b[10] |= 0x01  # multicast bit: random node
    return bytes(b)

def native_of(rfc):
    """System.Guid(rfc, bigEndian: true).TryWriteBytes(native) — .NET's mixed-endian layout."""
    return bytes([rfc[3], rfc[2], rfc[1], rfc[0], rfc[5], rfc[4], rfc[7], rfc[6]]) + rfc[8:]

def norse_v7_to_sql(rfc):
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

def v6_to_sql(rfc):
    # SqlGuid significance order 10..15, 8,9, 6,7, 4,5, 0..3 (most significant first):
    # the timestamp span rfc[0..8) fills the most significant slots, rfc[8..16) the rest.
    sql = bytearray(16)
    for dst, src in zip([10, 11, 12, 13, 14, 15, 8, 9, 6, 7, 4, 5, 0, 1, 2, 3], range(16)):
        sql[dst] = rfc[src]
    return bytes(sql)

SIGNIFICANCE = [10, 11, 12, 13, 14, 15, 8, 9, 6, 7, 4, 5, 0, 1, 2, 3]
def sql_key(b):
    return bytes(b[i] for i in SIGNIFICANCE)

def v7_millis(b): return int.from_bytes(b[0:6], "big")
def v6_millis(b):
    th = int.from_bytes(b[0:4], "big"); tm = int.from_bytes(b[4:6], "big")
    tl = int.from_bytes(b[6:8], "big") & 0x0FFF
    return max(((th << 28) | (tm << 12) | tl) - GREG, 0) // 10_000

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

# ---- v6 layout
v6_ms = [0, 1, 1_645_557_742_000, 1_750_000_000_123, 1_800_000_000_000]
clock_seqs = [0, 0x33C8, 0x3FFF, 0x1234, 0x0177]  # 0x0177: SQL octet 7 reads 0x77, a v7 decoy
nodes = [bytes(6), bytes.fromhex("9f6bdeced846"), b"\xff" * 6, bytes([1, 2, 3, 4, 5, 6])]
v6_rows = []
for i, ms in enumerate(v6_ms):
    for j, cs in enumerate(clock_seqs):
        n = nodes[(i + j) % len(nodes)]
        v6_rows.append({"unix_millis": ms, "clock_seq": cs, "node": h(n), "expect": h(v6_layout(ms, cs, n))})
dump("v6_layout.json", v6_rows)

# ---- SQL order
sql_rows = []
for r in v7_rows:
    rfc = bytes.fromhex(r["expect"])
    sql_rows.append({"version": 7, "rfc": h(rfc), "sql": h(norse_v7_to_sql(rfc))})
for r in v6_rows:
    rfc = bytes.fromhex(r["expect"])
    sql_rows.append({"version": 6, "rfc": h(rfc), "sql": h(v6_to_sql(rfc))})
# Sanity: SQL order sorts by (millis, counter) for v7 and by millis for v6.
v7s = sorted(v7_rows, key=lambda r: (r["unix_millis"], r["counter"]))
keys = [sql_key(norse_v7_to_sql(bytes.fromhex(r["expect"]))) for r in v7s]
assert keys == sorted(keys)
dump("sql_order.json", sql_rows)

# ---- timestamp (RFC order; layout-aware rows join once the core has them)
ts = []
ts.append({"uuid": "1ec9414c232a6b00b3c89f6bdeced846", "version": 6, "unix_millis": 1_645_557_742_000})  # RFC A.5
ts.append({"uuid": "017f22e279b07cc398c4dc0c0c07398f", "version": 7, "unix_millis": 1_645_557_742_000})  # RFC A.6
for r in v7_rows[::5]:
    ts.append({"uuid": r["expect"], "version": 7, "unix_millis": r["unix_millis"]})
for r in v6_rows[::3]:
    ts.append({"uuid": r["expect"], "version": 6, "unix_millis": r["unix_millis"]})
# Sub-millisecond ticks truncate rather than round.
sub = bytearray(v6_layout(1_750_000_000_123, 0, bytes(6)))
ticks = (1_750_000_000_123 * 10_000 + GREG) + 9_999
sub[0:4] = (ticks >> 28).to_bytes(4, "big"); sub[4:6] = ((ticks >> 12) & 0xFFFF).to_bytes(2, "big")
sub[6:8] = (0x6000 | (ticks & 0x0FFF)).to_bytes(2, "big")
ts.append({"uuid": h(sub), "version": 6, "unix_millis": 1_750_000_000_123})
# A pre-1970 Gregorian timestamp is RFC-valid and saturates to 0.
pre = bytearray(16); pre[6] = 0x60; pre[8] = 0x80
ts.append({"uuid": h(pre), "version": 6, "unix_millis": 0})
for u, v in [("00000000000000000000000000000000", 0), ("ffffffffffffffffffffffffffffffff", 15),
             ("919108f752d143209bacf847db4148a8", 4), (rows[0]["expect"], 5),
             ("c232ab00941411ecb3c89f6bdeced846", 1),
             # A 6 or 7 nibble under another variant is no RFC version, so no timestamp.
             ("017f22e279b07cc368c4dc0c0c07398f", 7),   # NCS (0xxx)
             ("017f22e279b07cc3d8c4dc0c0c07398f", 7),   # Microsoft (110x)
             ("1ec9414c232a6b00f3c89f6bdeced846", 6)]:  # Future (111x)
    ts.append({"uuid": u, "version": v, "unix_millis": None})
for r in ts:
    r["layout"] = "rfc9562"
for r in ts:
    b = bytes.fromhex(r["uuid"])
    assert b[6] >> 4 == r["version"], r
    if r["unix_millis"] is not None:
        assert (v6_millis(b) if r["version"] == 6 else v7_millis(b)) == r["unix_millis"], r

def norse_v7_to_rfc(sql):
    top14 = ((sql[8] & 0x3F) << 8) | sql[9]
    bottom12 = (sql[6] << 4) | (sql[7] & 0x0F)
    counter = (top14 << 12) | bottom12
    counter_hi = (counter >> 14) & 0xFFF; counter_lo = counter & 0x3FFF
    native = bytearray(16)
    native[3], native[2], native[1], native[0] = sql[10], sql[11], sql[12], sql[13]
    native[5], native[4] = sql[14], sql[15]
    native[6] = counter_hi & 0xFF
    native[7] = (sql[7] & 0xF0) | ((counter_hi >> 8) & 0x0F)
    native[8] = (sql[8] & 0xC0) | ((counter_lo >> 8) & 0x3F)
    native[9] = counter_lo & 0xFF
    native[10], native[11] = sql[4], sql[5]
    native[12:16] = sql[0:4]
    n = bytes(native)  # back from .NET's mixed-endian layout to RFC order
    return bytes([n[3], n[2], n[1], n[0], n[5], n[4], n[7], n[6]]) + n[8:]

def v6_to_rfc(sql):
    rfc = bytearray(16)
    for dst, src in zip([10, 11, 12, 13, 14, 15, 8, 9, 6, 7, 4, 5, 0, 1, 2, 3], range(16)):
        rfc[src] = sql[dst]
    return bytes(rfc)

def is_rfc(b, v): return (b[8] & 0xC0) == 0x80 and b[6] >> 4 == v

def sql_version(sql):
    """Try each inverse; accept it only if what comes back is an RFC UUID of that version."""
    hits = [v for v, inv in ((7, norse_v7_to_rfc), (6, v6_to_rfc)) if is_rfc(inv(sql), v)]
    assert len(hits) <= 1, sql.hex()
    return hits[0] if hits else 0

def variant(b):
    t = b[8] >> 5
    return "ncs" if t <= 3 else "rfc9562" if t <= 5 else "microsoft" if t == 6 else "future"

assert any(bytes.fromhex(r["sql"])[7] >> 4 == 7 for r in sql_rows if r["version"] == 6)
for r in sql_rows:
    sql = bytes.fromhex(r["sql"])
    assert sql_version(sql) == r["version"], r
    millis = v7_millis(bytes.fromhex(r["rfc"])) if r["version"] == 7 else v6_millis(bytes.fromhex(r["rfc"]))
    ts.append({"uuid": r["sql"], "layout": "sql_server", "version": r["version"], "unix_millis": millis})
others = ["00000000000000000000000000000000", "ffffffffffffffffffffffffffffffff",
          "919108f752d143209bacf847db4148a8", rows[0]["expect"]]
# A SQL-ordered v7 with its variant bits cleared: no longer a SQL-ordered RFC v7.
broken = bytearray.fromhex(next(r["sql"] for r in sql_rows if r["version"] == 7 and r["rfc"].startswith("0000017f")
                                or r["version"] == 7 and int(r["rfc"][:12], 16) == 1_645_557_742_000))
broken[8] &= 0x3F
others.append(broken.hex())
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
for r in sql_rows[::4] + [r for r in sql_rows if r["version"] == 6 and bytes.fromhex(r["sql"])[7] >> 4 == 7]:
    inspect.append({"uuid": r["sql"], "layout": "sql_server", "version": r["version"], "is_rfc": True,
                    "note": "SQL-ordered v%d" % r["version"]})
for u in others:
    inspect.append({"uuid": u, "layout": "sql_server", "version": 0, "is_rfc": False, "note": "not a SQL-ordered v6/v7"})
dump("inspect.json", inspect)
if STALE:
    sys.exit("corpus differs from the oracle: " + ", ".join(STALE))
print(("wrote" if WRITE else "ok:") + " v5 %d, v7_layout %d, v6_layout %d, sql_order %d, timestamp %d, inspect %d"
      % (len(rows), len(v7_rows), len(v6_rows), len(sql_rows), len(ts), len(inspect)))
