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

runner.bench_func("hyperuuid.new_v7", lambda: hyperuuid.new_v7(RFC_TEST_VECTOR_MS))

if sys.version_info >= (3, 14):
    runner.bench_func("stdlib uuid.uuid7", uuid.uuid7)

# --- batch: hyperuuid batch call vs a loop of individual calls --------------


def v7_individual_loop() -> None:
    """Mints BATCH_SIZE v7 UUIDs one call at a time."""
    for _ in range(BATCH_SIZE):
        hyperuuid.new_v7(RFC_TEST_VECTOR_MS)


runner.bench_func(
    f"hyperuuid.new_v7_batch({BATCH_SIZE})",
    lambda: hyperuuid.new_v7_batch(BATCH_SIZE, RFC_TEST_VECTOR_MS),
)
runner.bench_func(f"hyperuuid.new_v7 x{BATCH_SIZE} (loop)", v7_individual_loop)

# --- extraction: hyperuuid's *_timestamp vs stdlib's .time property ---------
#
# Both generation-only microbenchmarks above measure minting a UUID; these
# measure the other direction — pulling the embedded timestamp back out of an
# already-minted one. stdlib's uuid.UUID.time property has real version-aware
# logic for this (confirmed by reading its actual source), not just a v1-only
# stub, so it's a fair comparison, not a strawman. Only meaningful on 3.14+,
# where stdlib has v7 at all — same guard as the generation benchmarks
# above. The UUID being measured is pre-generated once outside the timed
# closures in both directions, so only the extraction cost itself is timed.

if sys.version_info >= (3, 14):
    hyper_v7_for_extraction = hyperuuid.new_v7(RFC_TEST_VECTOR_MS)
    stdlib_v7_for_extraction = uuid.uuid7()

    runner.bench_func(
        "hyperuuid.v7_timestamp",
        lambda: hyperuuid.v7_timestamp(hyper_v7_for_extraction),
    )
    # Both Unix-ms-based here, so this one's directly comparable.
    runner.bench_func(
        "stdlib UUID.time (v7)",
        lambda: stdlib_v7_for_extraction.time,
    )

    # The integer forms: the same embedded value, with no datetime built around it.
    runner.bench_func(
        "hyperuuid.v7_unix_millis",
        lambda: hyperuuid.v7_unix_millis(hyper_v7_for_extraction),
    )
