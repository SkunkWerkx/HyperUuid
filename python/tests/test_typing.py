"""Pins the typed surface: ``py.typed`` ships, ``_native.pyi`` describes what the loaded
extension actually has, and a consumer's code type-checks against the package as shipped.
The last one needs mypy and is skipped without it."""

import ast
import inspect
import os
from pathlib import Path

import pytest

import hyperuuid
from hyperuuid import _native

PACKAGE = Path(hyperuuid.__file__).resolve().parent
SAMPLES = Path(__file__).resolve().parent / "typecheck"


def _stubbed() -> dict[str, ast.FunctionDef]:
    tree = ast.parse((PACKAGE / "_native.pyi").read_text(encoding="utf-8"))
    return {node.name: node for node in tree.body if isinstance(node, ast.FunctionDef)}


def test_the_package_is_marked_typed():
    """The package ships py.typed."""
    assert (PACKAGE / "py.typed").is_file()


def test_the_stub_and_the_loaded_backend_name_the_same_functions():
    """_native.pyi names every function the package calls on the extension."""
    stubbed = set(_stubbed())
    # What the package actually calls is the surface; the extension may keep helpers beside it.
    consumed = {name for name in stubbed if callable(getattr(_native, name, None))}
    assert consumed == stubbed, "in _native.pyi but not on the extension"
    public = {name for name in hyperuuid.__all__ if callable(getattr(hyperuuid, name))}
    assert {"native_version", "new_v4", "new_v5", "new_v6", "new_v7"} <= public & stubbed


def test_the_stub_and_the_loaded_backend_agree_on_parameter_names():
    """Parameter names are part of the surface — a keyword call must work."""
    for name, node in _stubbed().items():
        expected = [arg.arg for arg in node.args.args]
        actual = list(inspect.signature(getattr(_native, name)).parameters)
        assert actual == expected, f"{name} on the extension"


def test_a_consumer_type_checks_against_the_package(monkeypatch):
    """mypy --strict accepts a consumer of the package."""
    api = pytest.importorskip("mypy.api")
    # The package as this checkout has it, the way conftest.py puts it on sys.path. --strict
    # follows the import, so hyperuuid's own annotations are checked against the stub too.
    monkeypatch.setenv("MYPYPATH", str(PACKAGE.parent))
    out, err, status = api.run(
        ["--strict", "--cache-dir", os.devnull, str(SAMPLES / "consumer.py")]
    )
    assert status == 0, out + err
