<?php

declare(strict_types=1);

namespace HyperUuid\Bench;

use HyperUuid\HyperUuid;
use HyperUuid\Uuid;
use PhpBench\Attributes as Bench;
use Ramsey\Uuid\Uuid as RamseyUuid;
use Ramsey\Uuid\UuidInterface;

/**
 * Timestamp *extraction* — how fast can each library read the embedded time back out of an
 * already-generated UUID — as opposed to UuidBench, which measures generation. ramsey/uuid is
 * the honest comparison point here (see the root README's "why not ramsey/uuid" framing): a
 * mature, pure-PHP implementation with its own real getDateTime() extraction logic, not a
 * naive stand-in.
 */
#[Bench\BeforeMethods('warmUp')]
#[Bench\Iterations(5)]
#[Bench\Revs(1000)]
#[Bench\OutputTimeUnit('microseconds', precision: 3)]
final class TimestampExtractionBench
{
    private const RFC_TEST_VECTOR_MS = 1_645_557_742_000;

    private Uuid $oursV6;
    private Uuid $oursV7;
    private UuidInterface $ramseyV6;
    private UuidInterface $ramseyV7;

    public function warmUp(): void
    {
        $this->oursV6 = HyperUuid::newV6(self::RFC_TEST_VECTOR_MS);
        $this->oursV7 = HyperUuid::newV7(self::RFC_TEST_VECTOR_MS);
        $this->ramseyV6 = RamseyUuid::uuid6();
        $this->ramseyV7 = RamseyUuid::uuid7();
    }

    public function benchOursV6Timestamp(): void
    {
        $this->oursV6->timestamp();
    }

    public function benchOursV7Timestamp(): void
    {
        $this->oursV7->timestamp();
    }

    public function benchRamseyV6GetDateTime(): void
    {
        $this->ramseyV6->getDateTime();
    }

    public function benchRamseyV7GetDateTime(): void
    {
        $this->ramseyV7->getDateTime();
    }
}
