"""Checks that every binding declares exactly the C ABI the core exports.

The export list is read from the core's source: every `pub extern "C" fn` in rust/src/ffi.rs.
Each binding's declarations are then read from its own source by the patterns in SITES, and
any difference fails the check: a missing export means a function nobody added to that
binding, and an extra one means the binding still declares a function the core dropped or
renamed. Both of those otherwise surface late, in one binding's suite on whichever leg
calls it.

The Python and Ruby extensions are not C ABI consumers: they link the core as a Rust crate
and register its functions under their language's spelling, so their sites map each export
to that spelling. The PHP extension (rust/src/php_ext.rs) is a benchmark spike that php/src
never calls, so it is not checked.

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


C_PROTOTYPE = r"(?m)^(?:u?int\d+_t|void) (\w+)\("

# Python's extension has conveniences past the C ABI: a buffer fill per time-ordered version
# and a datetime reading of the timestamp.
PYTHON_EXTRA = ("_bind", "fill_v6_bytes", "fill_v7_bytes", "v6_timestamp", "v7_timestamp")

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
        "Java (FFM)",
        [
            (
                "java/src/main/java/io/github/skunkwerkx/hyperuuid/UuidGenerator.java",
                r'export\("(\w+)"\)',
            )
        ],
        "all",
        (),
    ),
    (
        "Java (GraalWasm)",
        [
            (
                "java/src/main/java/io/github/skunkwerkx/hyperuuid/WasmBackend.java",
                r'= export\("(\w+)"\)',
            )
        ],
        "all",
        ("malloc", "free"),
    ),
    ("Go (cgo)", [("go/backend_static.go", C_PROTOTYPE)], "all", ()),
    ("Go (TinyGo)", [("go/backend_tinygo.go", C_PROTOTYPE)], "all", ()),
    (
        "Swift (hyperuuid.h)",
        [("swift/HyperUuidCore.artifactbundle/include/hyperuuid.h", C_PROTOTYPE)],
        "all",
        (),
    ),
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
    # The no-panic proof links every export but the version probe by symbol; that one it
    # calls through the crate.
    ("no-panic example", [("rust/examples/no_panic.rs", r"(?m)^    fn (\w+)\(")], "doors", ()),
    ("cdylib smoke", [(".github/scripts/cdylib_smoke.py", r'\("(\w+)", ')], "all", ()),
]


def declared(sources):
    names = set()
    for glob, pattern in sources:
        files = sorted(ROOT.glob(glob))
        if not files:
            sys.exit(f"check_exports: {glob} matches no file")
        for path in files:
            for match in re.finditer(pattern, path.read_text(encoding="utf-8"), re.MULTILINE):
                names.update(group for group in match.groups() if group)
    return names


def main():
    exports = core_exports()
    if VERSION not in exports or len(exports) < 2:
        sys.exit(f"check_exports: read {sorted(exports)} from {CORE}, which is not the core's ABI")
    print(f"core: {len(exports)} exports")
    failed = False
    for binding, sources, wants, extra in SITES:
        if wants == "all":
            expected = set(exports)
        elif wants == "doors":
            expected = exports - {VERSION}
        else:
            expected = {name for name in map(wants, exports) if name}
        expected |= set(extra)
        got = declared(sources)
        missing, stale = sorted(expected - got), sorted(got - expected)
        if missing or stale:
            failed = True
            print(f"FAIL {binding}")
            for name in missing:
                print(f"     missing {name}")
            for name in stale:
                print(f"     declares {name}, which the core does not export")
        else:
            print(f"ok   {binding}: {len(got)}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
