"""Many threads at once: on a free-threaded CPython they really run in parallel, so these are
where a GIL assumption in the extension would show. A GIL build runs them too, serialized."""

import os
import sys
import sysconfig
import threading
import time

import pytest

import hyperuuid

pytestmark = pytest.mark.skipif(sys.platform == "emscripten", reason="Pyodide has no threads")


@pytest.mark.skipif(
    not sysconfig.get_config_var("Py_GIL_DISABLED"), reason="needs a free-threaded CPython"
)
@pytest.mark.skipif("PYTHON_GIL" in os.environ, reason="PYTHON_GIL overrides the module's choice")
def test_import_leaves_the_gil_off():
    """The extension declares it needs no GIL, so importing it doesn't switch the GIL back on."""
    assert not sys._is_gil_enabled()


def test_threads_minting_v7s_at_once_never_collide_or_reorder():
    """Eight threads minting v7s at once, singly, in batches and into buffers: every UUID is
    distinct, and each thread's own single v7s increase."""
    threads, per_thread = 8, 20_000
    results: list = [None] * threads
    barrier = threading.Barrier(threads)

    def work(i: int) -> None:
        barrier.wait()
        singles = [hyperuuid.new_v7() for _ in range(per_thread)]
        batch = hyperuuid.new_v7_batch(1000)
        buffer = bytearray(16 * 1000)
        hyperuuid.fill_v7(buffer)
        results[i] = (singles, batch, buffer)

    workers = [threading.Thread(target=work, args=(i,)) for i in range(threads)]
    for worker in workers:
        worker.start()
    for worker in workers:
        worker.join()

    everything = set()
    for singles, batch, buffer in results:
        assert all(a < b for a, b in zip(singles, singles[1:]))
        everything.update(id_.bytes for id_ in singles + batch)
        everything.update(bytes(buffer[i : i + 16]) for i in range(0, len(buffer), 16))
    assert len(everything) == threads * (per_thread + 2000)


def test_fill_v7_survives_another_thread_resizing_its_buffer():
    """While fill_v7 writes a bytearray, a resize from another thread waits for it or is
    refused with BufferError, rather than freeing the bytes being written. Without that, a
    free-threaded build crashes here."""
    buffer = bytearray(16 * 20_000)
    stop = threading.Event()

    def fill() -> None:
        while not stop.is_set():
            try:
                hyperuuid.fill_v7(buffer)
            except ValueError:
                pass  # caught at a length that isn't a multiple of 16

    def resize() -> None:
        # Capped in time as well as count: a resize waits out a fill holding the bytearray's
        # lock, which makes each one cost a fill. Code without the export or the lock gets
        # through all of them in well under a second, and crashes on the way.
        deadline = time.monotonic() + 3
        for _ in range(100_000):
            if time.monotonic() > deadline:
                break
            try:
                buffer[:] = bytearray(16 * 20_000 if len(buffer) == 16 else 16)
            except BufferError:
                pass  # refused while a fill holds the buffer: the point of the export

    fillers = [threading.Thread(target=fill) for _ in range(3)]
    resizer = threading.Thread(target=resize)
    for thread in [*fillers, resizer]:
        thread.start()
    resizer.join()
    stop.set()
    for filler in fillers:
        filler.join()
    assert len(buffer) % 16 == 0


def test_fill_v7_threads_sharing_one_buffer_survive():
    """Three threads filling the same bytearray take turns on its lock rather than writing it
    at once, and a thread writing it meanwhile through a memoryview, which takes no lock, races
    on its contents and nothing else. On Windows either used to abort the process: the OS random
    source fail-fasts when the memory it is filling is written underneath it."""
    buffer = bytearray(16 * 20_000)
    stop = threading.Event()

    def fill() -> None:
        for _ in range(500):
            hyperuuid.fill_v7(buffer)

    def overwrite() -> None:
        zeros = bytes(len(buffer))
        with memoryview(buffer) as view:
            while not stop.is_set():
                view[:] = zeros

    writer = threading.Thread(target=overwrite)
    fillers = [threading.Thread(target=fill) for _ in range(3)]
    writer.start()
    for filler in fillers:
        filler.start()
    for filler in fillers:
        filler.join()
    stop.set()
    writer.join()
    hyperuuid.fill_v7(buffer)
    ids = [bytes(buffer[i : i + 16]) for i in range(0, len(buffer), 16)]
    assert ids == sorted(ids) and len(set(ids)) == len(ids)
