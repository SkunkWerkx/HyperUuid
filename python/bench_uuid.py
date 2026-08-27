"""pyperf benchmarks: hyperuuid vs stdlib uuid.

Run with: python bench_uuid.py --fast -o /tmp/hyperuuid_bench.json
(--fast trims pyperf's default process/value counts so this finishes in a
couple of minutes instead of ~20 — still statistically valid, just fewer
samples).
"""

from __future__ import annotations

import sys
import uuid

import pyperf

import hyperuuid

RFC_TEST_VECTOR_MS = 1_645_557_742_000
BATCH_SIZE = 1000

runner = pyperf.Runner()

# --- single-item: hyperuuid vs stdlib ---------------------------------------

runner.bench_func("hyperuuid.new_v4", hyperuuid.new_v4)
runner.bench_func("stdlib uuid.uuid4", uuid.uuid4)

runner.bench_func(
    "hyperuuid.new_v5",
    lambda: hyperuuid.new_v5(uuid.NAMESPACE_DNS, "example.com"),
)
runner.bench_func(
    "stdlib uuid.uuid5",
    lambda: uuid.uuid5(uuid.NAMESPACE_DNS, "example.com"),
)

runner.bench_func("hyperuuid.new_v6", lambda: hyperuuid.new_v6(RFC_TEST_VECTOR_MS))

runner.bench_func("hyperuuid.new_v7", lambda: hyperuuid.new_v7(RFC_TEST_VECTOR_MS))

if sys.version_info >= (3, 14):
    runner.bench_func("stdlib uuid.uuid7", uuid.uuid7)
    runner.bench_func("stdlib uuid.uuid6", uuid.uuid6)

# --- batch: hyperuuid batch call vs a loop of individual calls --------------


def v6_individual_loop() -> None:
    for _ in range(BATCH_SIZE):
        hyperuuid.new_v6(RFC_TEST_VECTOR_MS)


def v7_individual_loop() -> None:
    for _ in range(BATCH_SIZE):
        hyperuuid.new_v7(RFC_TEST_VECTOR_MS)


runner.bench_func(
    f"hyperuuid.new_v6_batch({BATCH_SIZE})",
    lambda: hyperuuid.new_v6_batch(BATCH_SIZE, RFC_TEST_VECTOR_MS),
)
runner.bench_func(f"hyperuuid.new_v6 x{BATCH_SIZE} (loop)", v6_individual_loop)

runner.bench_func(
    f"hyperuuid.new_v7_batch({BATCH_SIZE})",
    lambda: hyperuuid.new_v7_batch(BATCH_SIZE, RFC_TEST_VECTOR_MS),
)
runner.bench_func(f"hyperuuid.new_v7 x{BATCH_SIZE} (loop)", v7_individual_loop)
