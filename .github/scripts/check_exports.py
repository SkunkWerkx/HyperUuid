"""Checks that every binding declares exactly the C ABI the core exports.

The export list is read from the core's source: every `pub extern "C" fn` in rust/src/ffi.rs.
Each binding's declarations are then read from its own source by the patterns in SITES, and
any difference fails the check: a missing export means a function nobody added to that
binding, and an extra one means the binding still declares a function the core dropped or
renamed. Both of those otherwise surface late, in one binding's suite on whichever leg
calls it.

The Python and Ruby extensions and the TypeScript addon are not C ABI consumers: they link
the core as a Rust crate and register its functions under their language's spelling, so their
sites map each export to that spelling. The PHP extension (rust/src/php_ext.rs) is a benchmark
spike that php/src never calls, so it is not checked.

No dependencies and no build: CI runs it in a job of its own, and so can a dev loop.

usage: python .github/scripts/check_exports.py
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

CORE = ["rust/src/ffi.rs"]
VERSION = "hyperuuid_version"


def core_exports():
    names = set()
    for path in CORE:
        text = (ROOT / path).read_text(encoding="utf-8")
        names |= set(re.findall(r'pub extern "C" fn (\w+)', text))
    return names


def python_spelling(export):
    """python_ext.rs's name for an export: no uuid_ prefix (the module is the namespace),
    and the way back from SQL Server order is named for where it comes from."""
    if export == VERSION:
        return "native_version"
    return export.removeprefix("uuid_").replace("_to_rfc_order", "_from_sql_order")


def ruby_spelling(export):
    """ruby_ext.rs's name: Fiddle's, without the uuid_ prefix."""
    if export == VERSION:
        return "packed_version"
    return export.removeprefix("uuid_")


TS_EXTRA = ("new_v4_bytes", "new_v7_bytes", "new_v7_batch_bytes", "fill_v7", "v7_timestamp")


def ts_spelling(export):
    """The TypeScript addon's snake_case name: Python's, except that the layout pair is one
    function there (`v7_unix_millis` takes the optional layout), so its second half has no
    name of its own."""
    if export == "uuid_v7_unix_millis_in":
        return None
    return python_spelling(export)


def camel(name):
    """napi-rs exposes a Rust `snake_case` function as `camelCase` in the generated .d.ts."""
    head, *rest = name.split("_")
    return head + "".join(part[:1].upper() + part[1:] for part in rest)


def ts_camel(export):
    """binding.d.ts's name for an export, or None for the merged half of the layout pair."""
    name = ts_spelling(export)
    return camel(name) if name else None


def napi_catch_unwind(path):
    """Every napi function must carry `catch_unwind`: without it a panic in the core unwinds
    into Node-API and aborts the process instead of throwing. Attributes above consts and
    enums are not functions and are skipped."""
    path = Path(path)
    if not path.is_absolute():
        path = ROOT / path
    lines = path.read_text(encoding="utf-8").splitlines()
    bad = []
    for number, line in enumerate(lines):
        if not re.match(r"#\[napi(?:\(.*\))?\]\s*$", line.strip()) or "catch_unwind" in line:
            continue
        following = next((l for l in lines[number + 1 :] if l.strip()), "")
        match = re.match(r"pub fn (\w+)", following)
        if match:
            bad.append(f"{match.group(1)} (line {number + 1})")
    if bad:
        sys.exit(f"check_exports: {path.name} has napi functions without catch_unwind: " + ", ".join(bad))


def jni_no_panic(path):
    """Every JNI entry point must carry the no-panic attribute: the example's link-time proof
    covers only the fns that have it, so one dropped attribute would pass every site."""
    path = Path(path)
    if not path.is_absolute():
        path = ROOT / path
    lines = path.read_text(encoding="utf-8").splitlines()
    bad = []
    for number, line in enumerate(lines):
        match = re.match(r'pub unsafe extern "system" fn (\w+)', line)
        if not match:
            continue
        attrs, above = [], number - 1
        while above >= 0 and lines[above].startswith("#["):
            attrs.append(lines[above])
            above -= 1
        if not any("no_panic::no_panic" in attr for attr in attrs):
            bad.append(f"{match.group(1)} (line {number + 1})")
    if bad:
        sys.exit(f"check_exports: {path.name} has JNI functions without no_panic: " + ", ".join(bad))


C_PROTOTYPE = r"(?m)^(?:u?int\d+_t|void) (\w+)\("

# Python's extension has conveniences past the C ABI: a v7 buffer fill
# and a datetime reading of the timestamp.
PYTHON_EXTRA = ("_bind", "fill_v7_bytes", "v7_timestamp")

# (binding, [(file glob, pattern)], what it must declare, names it may declare besides).
# `all` is every export, `doors` every export but the version probe, and a function maps
# each export to the binding's own name.
SITES = [
    (
        "C# (P/Invoke)",
        [("csharp/HyperUuid/*.cs", r'LibraryImport\("hyperuuid", EntryPoint = "(\w+)"\)')],
        "all",
        (),
    ),
    (
        "C# (Blazor WebAssembly)",
        [("csharp/HyperUuid/*.cs", r'LibraryImport\("\*", EntryPoint = "(\w+)"\)')],
        "all",
        (),
    ),
    (
        "C# (iOS and Mac Catalyst)",
        [("csharp/HyperUuid/*.cs", r'LibraryImport\("__Internal", EntryPoint = "(\w+)"\)')],
        "all",
        (),
    ),
    (
        "Java (FFM)",
        [
            (
                "java/src/main/java25/io/github/skunkwerkx/hyperuuid/NativeCore.java",
                r'export\("(\w+)"\)',
            )
        ],
        "all",
        (),
    ),
    # The JDK 21 base: the Java declarations, the shim's registration table, the shim's no-panic
    # proof (snake case, so not `camel`) and the shim's own C imports of the core.
    (
        "Java (JNI natives)",
        [
            (
                "java/src/main/java/io/github/skunkwerkx/hyperuuid/NativeCore.java",
                r"static native \w+ (\w+)\(",
            )
        ],
        camel,
        ("jniNatives",),
    ),
    (
        "Java (JNI table)",
        [("java/jni/src/lib.rs", r'(?m)^\s*native!\(\s*"(\w+)",')],
        camel,
        ("jniNatives",),
    ),
    (
        "Java (JNI no-panic)",
        [("java/jni/examples/no_panic.rs", r"hyperuuid_jni::(?:native_(\w+)|(JNI_OnLoad))\(")],
        "all",
        ("jni_natives", "JNI_OnLoad"),
    ),
    (
        "Java (JNI imports)",
        [("java/jni/src/lib.rs", r"(?m)^\s*fn ((?:uuid|hyperuuid)_\w+)\(")],
        "all",
        (),
    ),
    ("Go (cgo)", [("go/backend_static.go", C_PROTOTYPE)], "all", ()),
    ("Go (TinyGo)", [("go/backend_tinygo.go", C_PROTOTYPE)], "all", ()),
    (
        # Generated by cbindgen (rust/tests/header.rs); the Swift bundle's copy is checked
        # byte-identical to it there.
        "C (include/hyperuuid.h)",
        [("include/hyperuuid.h", C_PROTOTYPE)],
        "all",
        (),
    ),
    (
        # Hand-written over the C header; every call into it is spelled ::uuid_* or
        # ::hyperuuid_*, which is what this reads.
        "C++ (include/hyperuuid.hpp)",
        [("include/hyperuuid.hpp", r"::((?:uuid|hyperuuid)_\w+)\(")],
        "all",
        (),
    ),
    ("R (core.h)", [("r/src/core.h", C_PROTOTYPE)], "all", ()),
    (
        "PHP (FFI cdef)",
        [("php/src/Runtime.php", r"'(?:int|u?int\d+_t|void) (\w+)\(")],
        "all",
        (),
    ),
    ("Ruby (Fiddle)", [("ruby/lib/hyperuuid/runtime.rb", r'handle\["(\w+)"\]')], "all", ()),
    (
        "Ruby (Magnus)",
        [("rust/src/ruby_ext.rs", r'define_singleton_method\(\s*"(\w+)"')],
        ruby_spelling,
        (),
    ),
    (
        "Python (PyO3)",
        [("rust/src/python_ext.rs", r"wrap_pyfunction!\((\w+), ")],
        python_spelling,
        PYTHON_EXTRA,
    ),
    (
        "Python (stubs)",
        [("python/src/hyperuuid/_native.pyi", r"(?m)^def (\w+)\(")],
        python_spelling,
        PYTHON_EXTRA,
    ),
    # The addon is checked at both ends, as Python's is: the Rust source is what registers a
    # function, binding.d.ts is what TypeScript callers see, and either can drift alone.
    (
        "TypeScript (napi)",
        [("typescript/src/lib.rs", r'(?m)^#\[napi\((?:[^)\n]*)\)\]\s*\npub fn (\w+)(?:<[^>]*>)?\(')],
        ts_spelling,
        TS_EXTRA,
    ),
    (
        "TypeScript (binding.d.ts)",
        [("typescript/binding.d.ts", r"(?m)^export declare function (\w+)\(")],
        ts_camel,
        tuple(camel(name) for name in TS_EXTRA),
    ),
    # The no-panic proof links every export but the version probe by symbol; that one it
    # calls through the crate.
    ("no-panic example", [("rust/examples/no_panic.rs", r"(?m)^    fn (\w+)\(")], "doors", ()),
    ("cdylib smoke", [(".github/scripts/cdylib_smoke.py", r'\("(\w+)", ')], "all", ()),
]


# Sites whose names must each appear once: a table that registers a name twice is a bug a
# set comparison cannot see.
UNIQUE_SITES = ("Java (JNI table)",)


def declared_list(sources):
    names = []
    for glob, pattern in sources:
        files = sorted(ROOT.glob(glob))
        if not files:
            sys.exit(f"check_exports: {glob} matches no file")
        for path in files:
            for match in re.finditer(pattern, path.read_text(encoding="utf-8"), re.MULTILINE):
                names.extend(group for group in match.groups() if group)
    return names


def declared(sources):
    return set(declared_list(sources))


def main():
    exports = core_exports()
    if VERSION not in exports or len(exports) < 2:
        sys.exit(f"check_exports: read {sorted(exports)} from {CORE}, which is not the core's ABI")
    print(f"core: {len(exports)} exports")
    napi_catch_unwind("typescript/src/lib.rs")
    jni_no_panic("java/jni/src/lib.rs")
    failed = False
    for binding, sources, wants, extra in SITES:
        if wants == "all":
            expected = set(exports)
        elif wants == "doors":
            expected = exports - {VERSION}
        else:
            expected = {name for name in map(wants, exports) if name}
        expected |= set(extra)
        found = declared_list(sources)
        got = set(found)
        repeats = sorted({name for name in found if found.count(name) > 1}) if binding in UNIQUE_SITES else []
        missing, stale = sorted(expected - got), sorted(got - expected)
        if missing or stale or repeats:
            failed = True
            print(f"FAIL {binding}")
            for name in repeats:
                print(f"     repeats {name}")
            for name in missing:
                print(f"     missing {name}")
            for name in stale:
                print(f"     declares {name}, which the core does not export")
        else:
            print(f"ok   {binding}: {len(got)}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
