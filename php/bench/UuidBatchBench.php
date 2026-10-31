<?php

declare(strict_types=1);

namespace HyperUuid\Bench;

use HyperUuid\HyperUuid;
use PhpBench\Attributes as Bench;

/**
 * Batch generation vs. an individual-call loop of the same size — the same comparison
 * every other binding's benchmark suite makes — and the byte form beside the object form.
 *
 * Each iteration is a fresh process, so its first call pays one-time costs (the random
 * source's first draw, the first allocations of each size); the warmup revolution keeps them
 * out of every figure.
 */
#[Bench\BeforeMethods('readClock')]
#[Bench\Iterations(5)]
#[Bench\Revs(100)]
#[Bench\Warmup(1)]
#[Bench\OutputTimeUnit('microseconds', precision: 1)]
final class UuidBatchBench
{
    /**
     * The present, read once per iteration process, outside the timed revolutions: every call
     * is then minted through the v7 stream. A fixed timestamp held long enough would split the
     * stream forward until it failed with CounterExhaustedException.
     */
    private int $nowMs = 0;

    public function readClock(): void
    {
        $this->nowMs = (int) (microtime(true) * 1000);
    }

    public function benchNewV7Batch1000(): void
    {
        HyperUuid::newV7Batch(1000, $this->nowMs);
    }

    public function benchNewV7Individualx1000(): void
    {
        for ($i = 0; $i < 1000; $i++) {
            HyperUuid::newV7($this->nowMs);
        }
    }

    public function benchNewV7BatchBytes1000(): void
    {
        HyperUuid::newV7BatchBytes(1000, $this->nowMs);
    }
}
