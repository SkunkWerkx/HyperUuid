<?php

declare(strict_types=1);

namespace HyperUuid\Bench;

use HyperUuid\HyperUuid;
use PhpBench\Attributes as Bench;

/**
 * Batch generation vs. an individual-call loop of the same size — the same comparison
 * every other binding's benchmark suite makes.
 */
#[Bench\Iterations(5)]
#[Bench\Revs(1)]
#[Bench\OutputTimeUnit('milliseconds', precision: 3)]
final class UuidBatchBench
{
    private const RFC_TEST_VECTOR_MS = 1_645_557_742_000;

    public function benchNewV6Batch1000(): void
    {
        HyperUuid::newV6Batch(1000, self::RFC_TEST_VECTOR_MS);
    }

    public function benchNewV6Individualx1000(): void
    {
        for ($i = 0; $i < 1000; $i++) {
            HyperUuid::newV6(self::RFC_TEST_VECTOR_MS);
        }
    }

    public function benchNewV7Batch1000(): void
    {
        HyperUuid::newV7Batch(1000, self::RFC_TEST_VECTOR_MS);
    }

    public function benchNewV7Individualx1000(): void
    {
        for ($i = 0; $i < 1000; $i++) {
            HyperUuid::newV7(self::RFC_TEST_VECTOR_MS);
        }
    }
}
