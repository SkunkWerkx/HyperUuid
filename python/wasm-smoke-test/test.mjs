// Proves the Rust core, built as an Emscripten SIDE_MODULE (not the cdylib the native
// bindings use, and not the staticlib the C# WASM build uses — a real, third artifact shape,
// see README.md in this directory), loads and runs correctly inside a genuine Pyodide
// (CPython-to-WASM) session via plain ctypes.CDLL — the exact mechanism hyperuuid's own
// _runtime.py already uses natively, unchanged.
//
// Run with: node test.mjs   (after building the artifact — see README.md)
import { loadPyodide } from "pyodide";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const wasmPath = path.join(
  here,
  "../../rust/target/wasm32-unknown-emscripten/release/hyperuuid.wasm"
);

const pyodide = await loadPyodide();
pyodide.FS.writeFile("/tmp/libhyperuuid.so", readFileSync(wasmPath));

const result = await pyodide.runPythonAsync(`
import ctypes
import time
import uuid as py_uuid

lib = ctypes.CDLL("/tmp/libhyperuuid.so")
lib.uuid_new_v4.argtypes = [ctypes.c_void_p]
lib.uuid_new_v4.restype = ctypes.c_int32
lib.uuid_new_v5.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint32, ctypes.c_void_p]
lib.uuid_new_v5.restype = ctypes.c_int32
lib.uuid_new_v7.argtypes = [ctypes.c_uint64, ctypes.c_void_p]
lib.uuid_new_v7.restype = ctypes.c_int32
lib.uuid_v7_unix_millis.argtypes = [ctypes.c_void_p]
lib.uuid_v7_unix_millis.restype = ctypes.c_uint64

lines = []
failed = False

def fail(msg):
    global failed
    failed = True
    lines.append(f"FAIL: {msg}")

# v4: proves randomness works inside Pyodide's sandbox.
a = ctypes.create_string_buffer(16)
b = ctypes.create_string_buffer(16)
lib.uuid_new_v4(ctypes.cast(a, ctypes.c_void_p))
lib.uuid_new_v4(ctypes.cast(b, ctypes.c_void_p))
if a.raw == b.raw:
    fail("two v4 calls produced identical output")
else:
    lines.append(f"v4: {py_uuid.UUID(bytes=a.raw)} {py_uuid.UUID(bytes=b.raw)}")

# v5: RFC 9562 Appendix A.4 vector (DNS namespace + www.example.com).
ns = py_uuid.UUID("6ba7b810-9dad-11d1-80b4-00c04fd430c8").bytes
name = b"www.example.com"
v5_out = ctypes.create_string_buffer(16)
lib.uuid_new_v5(
    ctypes.cast(ctypes.create_string_buffer(ns, 16), ctypes.c_void_p),
    name, len(name), ctypes.cast(v5_out, ctypes.c_void_p),
)
v5 = py_uuid.UUID(bytes=v5_out.raw)
expected_v5 = py_uuid.UUID("2ed6657d-e927-568b-95e1-2665a8aea6a2")
if v5 != expected_v5:
    fail(f"v5 mismatch, got {v5}")
else:
    lines.append(f"v5: {v5} matches RFC 9562 Appendix A.4 vector")

# v7, real clock: proves time.time() works correctly inside Pyodide too.
before_ms = int(time.time() * 1000)
v7_out = ctypes.create_string_buffer(16)
lib.uuid_new_v7(before_ms, ctypes.cast(v7_out, ctypes.c_void_p))
after_ms = int(time.time() * 1000)
embedded_ms = lib.uuid_v7_unix_millis(ctypes.cast(v7_out, ctypes.c_void_p))
v7 = py_uuid.UUID(bytes=v7_out.raw)
if not (before_ms <= embedded_ms <= after_ms):
    fail(f"v7 timestamp {embedded_ms} not within [{before_ms}, {after_ms}]")
else:
    lines.append(f"v7 (now): {v7} embeds real clock timestamp {embedded_ms}, wall clock was [{before_ms}, {after_ms}]")

lines.append("")
lines.append("SOME CHECKS FAILED" if failed else "ALL CHECKS PASSED (real Pyodide, ctypes.CDLL dynamic load)")
"\\n".join(lines)
`);

console.log(result);
if (result.includes("FAIL")) process.exit(1);
