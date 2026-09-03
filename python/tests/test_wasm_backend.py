"""Pins the wasm backend's selection and its agreement with the native one. The whole main
suite already runs under both (``HYPERUUID_WASM=1 pytest`` forces wasm); this file pins the
*agreement* between them by comparing deterministic outputs across a subprocess boundary —
the same shape as the Ruby binding's native_backend_spec. Skipped when the process under
test is not the wasm backend, so a plain ``pytest`` still exercises the native pin."""

import os
import subprocess
import sys
from pathlib import Path

import pytest

import hyperuuid

pytestmark = pytest.mark.skipif(
    hyperuuid.BACKEND != "wasm", reason=f"wasm backend not loaded (BACKEND={hyperuuid.BACKEND})"
)


def native_eval(expression: str) -> str:
    src = str(Path(__file__).resolve().parent.parent / "src")
    env = {k: v for k, v in os.environ.items() if k != "HYPERUUID_WASM"}
    env["PYTHONPATH"] = src
    out = subprocess.run(
        [sys.executable, "-c", f"import hyperuuid; print({expression}, end='')"],
        env=env,
        capture_output=True,
        text=True,
        check=True,
    )
    return out.stdout


def test_reports_the_wasm_backend():
    assert hyperuuid.BACKEND == "wasm"
    assert native_eval("hyperuuid.BACKEND") == "native"


def test_agrees_with_the_native_backend_on_deterministic_v5():
    import uuid

    wasm = hyperuuid.new_v5(uuid.NAMESPACE_DNS, "example.com")
    assert native_eval('str(hyperuuid.new_v5(__import__("uuid").NAMESPACE_DNS, "example.com"))') == str(wasm)


def test_agrees_with_the_native_backend_on_v7_timestamp_extraction():
    minted = hyperuuid.new_v7(1_645_557_742_123)
    recovered = hyperuuid.v7_timestamp(minted)
    native = native_eval(f'hyperuuid.v7_timestamp(__import__("uuid").UUID("{minted}")).isoformat()')
    assert native == recovered.isoformat()


def test_agrees_with_the_native_backend_on_sql_order():
    minted = hyperuuid.new_v7(1_645_557_742_123)
    sql = hyperuuid.v7_to_sql_order(minted)
    assert native_eval(f'str(hyperuuid.v7_to_sql_order(__import__("uuid").UUID("{minted}")))') == str(sql)
    assert hyperuuid.v7_from_sql_order(sql) == minted


def test_batch_and_fill_share_one_guest_buffer_safely():
    # Grow-only guest buffers: a larger batch after a smaller one, then a fill of the larger
    # size again, must all come back intact and distinct.
    small = hyperuuid.new_v7_batch(10, 1_700_000_000_000)
    big = hyperuuid.new_v7_batch(1000, 1_700_000_000_000)
    buf = bytearray(16 * 1000)
    hyperuuid.fill_v7(buf, 1_700_000_000_000)
    assert len({*small, *big}) == 1010
    assert all(buf[i + 6] >> 4 == 7 for i in range(0, len(buf), 16))
