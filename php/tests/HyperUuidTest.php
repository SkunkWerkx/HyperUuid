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

    public function testV6EmbedsTheTimestamp(): void
    {
        $id = HyperUuid::newV6(self::RFC_TEST_VECTOR_MS);
        $expected = \DateTimeImmutable::createFromFormat(
            'U.v',
            sprintf('%d.%03d', intdiv(self::RFC_TEST_VECTOR_MS, 1000), self::RFC_TEST_VECTOR_MS % 1000),
            new \DateTimeZone('UTC')
        );
        self::assertEquals($expected, $id->timestamp());
    }

    public function testV6HasVersionAndVariantBits(): void
    {
        $id = HyperUuid::newV6(self::RFC_TEST_VECTOR_MS);
        self::assertSame(6, $id->version());
        self::assertSame(0b10, $id->variant());
    }

    public function testV6SetsTheNodeIdMulticastBit(): void
    {
        $id = HyperUuid::newV6(self::RFC_TEST_VECTOR_MS);
        self::assertSame(1, \ord($id->bytes()[10]) & 0x01);
    }

    public function testV6IsNonDeterministicWithinTheSameMillisecond(): void
    {
        $seen = [];
        for ($i = 0; $i < 100; $i++) {
            $seen[(string) HyperUuid::newV6(self::RFC_TEST_VECTOR_MS)] = true;
        }
        self::assertCount(100, $seen);
    }

    public function testV6CurrentTimestampIsEmbedded(): void
    {
        $before = (int) round(microtime(true) * 1000);
        $id = HyperUuid::newV6();
        $after = (int) round(microtime(true) * 1000);

        $embedded = $id->timestamp()->getTimestamp() * 1000 + (int) $id->timestamp()->format('v');
        self::assertGreaterThanOrEqual($before, $embedded);
        self::assertLessThanOrEqual($after, $embedded);
    }

    public function testV6BatchReturnsCountUuidsSharingTheTimestamp(): void
    {
        $ids = HyperUuid::newV6Batch(10, self::RFC_TEST_VECTOR_MS);
        self::assertCount(10, $ids);
        $expected = \DateTimeImmutable::createFromFormat(
            'U.v',
            sprintf('%d.%03d', intdiv(self::RFC_TEST_VECTOR_MS, 1000), self::RFC_TEST_VECTOR_MS % 1000),
            new \DateTimeZone('UTC')
        );
        foreach ($ids as $id) {
            self::assertSame(6, $id->version());
            self::assertEquals($expected, $id->timestamp());
        }
    }

    public function testV6BatchProducesPairwiseDistinctUuids(): void
    {
        $ids = HyperUuid::newV6Batch(100, self::RFC_TEST_VECTOR_MS);
        $seen = array_unique(array_map(fn ($id) => (string) $id, $ids));
        self::assertCount(100, $seen);
    }

    public function testV6BatchCountZeroReturnsEmptyArray(): void
    {
        self::assertSame([], HyperUuid::newV6Batch(0, self::RFC_TEST_VECTOR_MS));
    }

    public function testV6BatchOverflowTimestampThrows(): void
    {
        $this->expectException(TimestampOutOfRangeException::class);
        HyperUuid::newV6Batch(1, PHP_INT_MAX);
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

    public function testV7BatchReturnsCountUuidsSortedAndSharingTheTimestamp(): void
    {
        $ids = HyperUuid::newV7Batch(1000, self::RFC_TEST_VECTOR_MS);
        self::assertCount(1000, $ids);
        $strings = array_map(fn ($id) => (string) $id, $ids);
        $sorted = $strings;
        sort($sorted, SORT_STRING);
        self::assertSame($sorted, $strings);
        foreach ($ids as $id) {
            self::assertSame(self::RFC_TEST_VECTOR_MS, self::embeddedMillis($id));
        }
    }

    public function testV7BatchContinuesTheSameCounterSequenceAsIndividualCalls(): void
    {
        $before = HyperUuid::newV7(self::RFC_TEST_VECTOR_MS);
        $batch = HyperUuid::newV7Batch(10, self::RFC_TEST_VECTOR_MS);
        $after = HyperUuid::newV7(self::RFC_TEST_VECTOR_MS);

        $ids = [(string) $before, ...array_map(fn ($id) => (string) $id, $batch), (string) $after];
        $sorted = $ids;
        sort($sorted, SORT_STRING);
        self::assertSame($sorted, $ids);
    }

    public function testV7BatchCountZeroReturnsEmptyArray(): void
    {
        self::assertSame([], HyperUuid::newV7Batch(0, self::RFC_TEST_VECTOR_MS));
    }

    public function testV7BatchOverflowTimestampThrows(): void
    {
        $this->expectException(TimestampOutOfRangeException::class);
        HyperUuid::newV7Batch(1, 0x0001_0000_0000_0000);
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

    public function testNilIsAllZeroBytes(): void
    {
        self::assertSame(str_repeat("\x00", 16), Uuid::nil()->bytes());
        self::assertSame('00000000-0000-0000-0000-000000000000', (string) Uuid::nil());
    }

    public function testMaxIsAllOneBytes(): void
    {
        self::assertSame(str_repeat("\xFF", 16), Uuid::max()->bytes());
        self::assertSame('ffffffff-ffff-ffff-ffff-ffffffffffff', (string) Uuid::max());
    }

    public function testNilAndMaxRoundTripThroughParse(): void
    {
        self::assertTrue(Uuid::parse((string) Uuid::nil())->equals(Uuid::nil()));
        self::assertTrue(Uuid::parse((string) Uuid::max())->equals(Uuid::max()));
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
