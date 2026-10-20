"""Many threads at once: on a free-threaded CPython they really run in parallel, so these are
where a GIL assumption in the extension would show. A GIL build runs them too, serialized."""

import os
import sys
import sysconfig
import threading

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
    """While fill_v7 writes a bytearray, a resize from another thread is refused with
    BufferError rather than freeing the bytes being written. Without that, a free-threaded
    build crashes here."""
    buffer = bytearray(16 * 100_000)
    stop = threading.Event()

    def fill() -> None:
        while not stop.is_set():
            try:
                hyperuuid.fill_v7(buffer)
            except ValueError:
                pass  # caught at a length that isn't a multiple of 16

    def resize() -> None:
        for _ in range(100_000):
            try:
                buffer[:] = bytearray(16 * 100_000 if len(buffer) == 16 else 16)
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
