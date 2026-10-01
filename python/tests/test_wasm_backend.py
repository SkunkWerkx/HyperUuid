"""Pins the wasm backend's selection and its agreement with the native one. The whole main
suite already runs under both (``HYPERUUID_WASM=1 pytest`` forces wasm); this file pins the
*agreement* between them by comparing deterministic outputs across a subprocess boundary —
the same shape as the Ruby binding's native_backend_spec. Skipped when the process under
test is not the wasm backend, so a plain ``pytest`` still exercises the native pin."""

import os
import subprocess
import sys
import threading
import uuid
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


def test_agrees_with_the_native_backend_on_the_version():
    assert native_eval("hyperuuid.native_version()") == hyperuuid.native_version()


def test_a_failed_regrow_leaves_no_dangling_buffer(monkeypatch):
    # The grow-only buffers free the old block before asking for a bigger one. If that
    # malloc fails, the pointer must not survive it: the next, smaller request would be
    # written into memory the guest has already taken back.
    from hyperuuid import _wasm

    guest = _wasm._get()
    hyperuuid.new_v7_batch(4, 1_700_000_000_000)
    hyperuuid.new_v5(uuid.NAMESPACE_DNS, "example.com")
    assert guest._batch_ptr and guest._name_ptr

    def refuse(size):
        raise MemoryError(f"hyperuuid: guest malloc({size}) failed")

    monkeypatch.setattr(guest, "_malloc", refuse)
    with pytest.raises(MemoryError):
        hyperuuid.new_v7_batch(guest._batch_cap // 16 + 1, 1_700_000_000_000)
    with pytest.raises(MemoryError):
        hyperuuid.new_v5(uuid.NAMESPACE_DNS, "x" * (guest._name_cap + 1))
    assert (guest._batch_ptr, guest._batch_cap) == (0, 0)
    assert (guest._name_ptr, guest._name_cap) == (0, 0)

    monkeypatch.undo()
    assert len(set(hyperuuid.new_v7_batch(4, 1_700_000_000_000))) == 4
    assert hyperuuid.new_v5(uuid.NAMESPACE_DNS, "python.org") == uuid.UUID("886313e1-3b8a-5372-9b90-0c9aee199e5d")


def test_a_batch_past_the_guests_address_space_is_a_memory_error():
    # 2**28 UUIDs is exactly 4 GiB: one byte more than a 32-bit size_t can say. The call slot
    # takes a size without a range check, so unguarded this wrapped to malloc(0) and the
    # batch overran it; it must be refused before the guest is asked.
    with pytest.raises(MemoryError):
        hyperuuid.new_v7_batch(2**28, 1_700_000_000_000)
    with pytest.raises(MemoryError):
        hyperuuid.new_v6_batch(2**32 - 1, 1_700_000_000_000)
    assert len(hyperuuid.new_v7_batch(3, 1_700_000_000_000)) == 3


def test_serializes_concurrent_callers_on_the_one_shared_instance():
    # One store, one pair of scratch buffers and one v7 counter, shared by every thread:
    # unserialized, a v5 would hash another thread's name, a v7 would come back as another
    # thread's bytes, and a batch would be read out of a buffer someone else just regrew.
    millis = 1_700_000_000_000
    minted: list[uuid.UUID] = []
    derived: list[tuple[str, uuid.UUID]] = []
    batches: list[list[uuid.UUID]] = []

    def work(n: int) -> None:
        for i in range(100):
            name = f"thread-{n}-name-{i}-" + "x" * (n * 7 + i)
            minted.append(hyperuuid.new_v7(millis))
            derived.append((name, hyperuuid.new_v5(uuid.NAMESPACE_URL, name)))
            if i % 10 == 0:
                batches.append(hyperuuid.new_v7_batch(8 * (n + 1), millis))

    threads = [threading.Thread(target=work, args=(n,)) for n in range(8)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    assert len(minted) == 800 and len(derived) == 800 and len(batches) == 80
    assert all(got == uuid.uuid5(uuid.NAMESPACE_URL, name) for name, got in derived)
    everything = minted + [id_ for batch in batches for id_ in batch]
    assert len(set(everything)) == len(everything)
    assert all(id_.version == 7 and id_.int >> 80 == millis for id_ in everything)
    # Each batch is one contiguous counter block, so it comes back already in order.
    assert all(batch == sorted(batch) for batch in batches)
