<?php

declare(strict_types=1);

namespace HyperUuid\Bench;

use HyperUuid\HyperUuid;
use HyperUuid\Namespaces;
use PhpBench\Attributes as Bench;

/**
 * Single-item generation, one FFI call per iteration — the same shape as the
 * single-item benchmarks in rust/benches/uuid_benchmarks.rs, csharp/HyperUuid.Benchmarks,
 * and go/uuidgen_bench_test.go.
 */
#[Bench\BeforeMethods('warmUp')]
#[Bench\Iterations(5)]
#[Bench\Revs(1000)]
#[Bench\OutputTimeUnit('microseconds', precision: 3)]
final class UuidBench
{
    private const RFC_TEST_VECTOR_MS = 1_645_557_742_000;

    public function warmUp(): void
    {
        HyperUuid::newV4();
    }

    public function benchNewV4(): void
    {
        HyperUuid::newV4();
    }

    public function benchNewV5(): void
    {
        HyperUuid::newV5(Namespaces::dns(), 'example.com');
    }

    public function benchNewV6(): void
    {
        HyperUuid::newV6(self::RFC_TEST_VECTOR_MS);
    }

    public function benchNewV7(): void
    {
        HyperUuid::newV7(self::RFC_TEST_VECTOR_MS);
    }

    /** Naive inline reference point, not RFC-complete — PHP core has no UUID facility to compare against. */
    public function benchNaiveInlineV4(): void
    {
        $bytes = random_bytes(16);
        $bytes[6] = \chr((\ord($bytes[6]) & 0x0f) | 0x40);
        $bytes[8] = \chr((\ord($bytes[8]) & 0x3f) | 0x80);
    }
}
