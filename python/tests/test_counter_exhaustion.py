"""v7 counter exhaustion, end to end through the public API.

Driving the real stream to its ceiling poisons the process-wide v7 state for everything
after it, so the whole script runs in a subprocess of its own.
"""

import os
import subprocess
import sys
from pathlib import Path

import pytest

# Pyodide cannot spawn a process, and run in-process the script would poison the stream for
# every test after it. The core's own wasm32-wasip1 suite runs the same exhaustion end to end.
pytestmark = pytest.mark.skipif(sys.platform == "emscripten", reason="Pyodide has no subprocesses")

SRC = str(Path(__file__).resolve().parent.parent / "src")

SCRIPT = r"""
import hyperuuid

M = 17592186044414  # 2**44 - 2, the last millisecond the stream holds
MESSAGE = (
    "version 7 counter exhausted: the stream cannot advance without running more than "
    "1000 ms ahead of the supplied timestamp or past its last millisecond"
)


def exhausted(call):
    try:
        call()
    except RuntimeError as e:
        assert str(e) == MESSAGE, repr(str(e))
        return True
    return False


# 1. One single v7 at M succeeds.
hyperuuid.new_v7(M)

# 2. Batches of 65,536 at M until the first error: the batch site.
for _ in range(1025):
    if exhausted(lambda: hyperuuid.new_v7_batch(65536, M)):
        break
else:
    raise AssertionError("no batch exhausted the stream")

# 3. Singles at M until the first error: the single site.
for _ in range(1024):
    if exhausted(lambda: hyperuuid.new_v7(M)):
        break
else:
    raise AssertionError("no single exhausted the stream")

# 4. Every other v7 door now fails at once.
assert exhausted(lambda: hyperuuid.new_v7(M)), "single"
assert exhausted(lambda: hyperuuid.new_v7_batch(1, M)), "batch"
assert exhausted(lambda: hyperuuid.fill_v7(bytearray(16), M)), "fill_v7"
"""


def test_counter_exhaustion_in_a_subprocess() -> None:
    """The real stream, driven to its ceiling, raises the exhaustion error at every v7 door."""
    env = dict(os.environ)
    env["PYTHONPATH"] = os.pathsep.join(filter(None, [SRC, env.get("PYTHONPATH")]))
    result = subprocess.run(
        [sys.executable, "-c", SCRIPT], check=False, env=env, capture_output=True, text=True
    )
    assert result.returncode == 0, result.stdout + result.stderr
