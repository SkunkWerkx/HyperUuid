<?php

declare(strict_types=1);

namespace HyperUuid\Tests;

use HyperUuid\HyperUuid;
use HyperUuid\Namespaces;
use HyperUuid\TimestampOutOfRangeException;
use HyperUuid\Uuid;
use PHPUnit\Framework\TestCase;

final class HyperUuidTest extends TestCase
{
    private const RFC_TEST_VECTOR_MS = 1_645_557_742_000;

    public function testV4HasVersionAndVariantBits(): void
    {
        $id = HyperUuid::newV4();
        self::assertSame(4, $id->version());
        self::assertSame(0b10, $id->variant());
    }

    public function testV4IsNonDeterministic(): void
    {
        $seen = [];
        for ($i = 0; $i < 100; $i++) {
            $seen[(string) HyperUuid::newV4()] = true;
        }
        self::assertCount(100, $seen);
    }

    public function testV5MatchesRfcTestVector(): void
    {
        $id = HyperUuid::newV5(Namespaces::dns(), 'www.example.com');
        self::assertTrue($id->equals(Uuid::parse('2ed6657d-e927-568b-95e1-2665a8aea6a2')));
    }

    public function testV5MatchesPythonDocsVector(): void
    {
        $id = HyperUuid::newV5(Namespaces::dns(), 'python.org');
        self::assertTrue($id->equals(Uuid::parse('886313e1-3b8a-5372-9b90-0c9aee199e5d')));
    }

    public function testV5IsDeterministic(): void
    {
        $a = HyperUuid::newV5(Namespaces::dns(), 'same-name');
        $b = HyperUuid::newV5(Namespaces::dns(), 'same-name');
        self::assertTrue($a->equals($b));
    }

    public function testV5DifferentNamespacesDiffer(): void
    {
        $dns = HyperUuid::newV5(Namespaces::dns(), 'test');
        $url = HyperUuid::newV5(Namespaces::url(), 'test');
        self::assertFalse($dns->equals($url));
    }

    public function testV5HandlesMultiByteUtf8Names(): void
    {
        $a = HyperUuid::newV5(Namespaces::url(), 'café — 日本語');
        $b = HyperUuid::newV5(Namespaces::url(), 'café — 日本語');
        self::assertTrue($a->equals($b));
    }

    public function testV5EmptyNameDoesNotError(): void
    {
        $id = HyperUuid::newV5(Namespaces::dns(), '');
        self::assertSame(5, $id->version());
    }

    public function testV7EmbedsTheTimestamp(): void
    {
        $id = HyperUuid::newV7(self::RFC_TEST_VECTOR_MS);
        self::assertSame(self::RFC_TEST_VECTOR_MS, self::embeddedMillis($id));
    }

    public function testV7HasVersionAndVariantBits(): void
    {
        $id = HyperUuid::newV7(self::RFC_TEST_VECTOR_MS);
        self::assertSame(7, $id->version());
        self::assertSame(0b10, $id->variant());
    }

    public function testV7OverflowTimestampThrows(): void
    {
        $this->expectException(TimestampOutOfRangeException::class);
        HyperUuid::newV7(0x0001_0000_0000_0000);
    }

    public function testV7SameMillisecondBatchIsMonotonicallyOrdered(): void
    {
        $ids = [];
        for ($i = 0; $i < 100; $i++) {
            $ids[] = (string) HyperUuid::newV7(self::RFC_TEST_VECTOR_MS);
        }
        $sorted = $ids;
        sort($sorted, SORT_STRING);
        self::assertSame($sorted, $ids);
    }

    public function testV7CurrentTimestampIsEmbedded(): void
    {
        $before = (int) round(microtime(true) * 1000);
        $id = HyperUuid::newV7();
        $after = (int) round(microtime(true) * 1000);

        $embedded = self::embeddedMillis($id);
        self::assertGreaterThanOrEqual($before, $embedded);
        self::assertLessThanOrEqual($after, $embedded);
    }

    public function testV7TimestampRecoversTheExactMillisecond(): void
    {
        $id = HyperUuid::newV7(self::RFC_TEST_VECTOR_MS);
        $expected = \DateTimeImmutable::createFromFormat(
            'U.v',
            sprintf('%d.%03d', intdiv(self::RFC_TEST_VECTOR_MS, 1000), self::RFC_TEST_VECTOR_MS % 1000),
            new \DateTimeZone('UTC')
        );
        self::assertEquals($expected, $id->timestamp());
    }

    public function testV7TimestampRoundTripsZeroAndMax(): void
    {
        self::assertSame(0, HyperUuid::newV7(0)->timestamp()->getTimestamp());

        $maxMs = 0x0000_FFFF_FFFF_FFFF;
        $recovered = HyperUuid::newV7($maxMs)->timestamp();
        self::assertSame($maxMs, $recovered->getTimestamp() * 1000 + (int) $recovered->format('v'));
    }

    private static function embeddedMillis(Uuid $id): int
    {
        $bytes = $id->bytes();
        $ms = 0;
        for ($i = 0; $i < 6; $i++) {
            $ms = ($ms << 8) | \ord($bytes[$i]);
        }
        return $ms;
    }
}
