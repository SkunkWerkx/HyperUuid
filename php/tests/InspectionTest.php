<?php

declare(strict_types=1);

namespace HyperUuid\Tests;

use HyperUuid\HyperUuid;
use HyperUuid\Namespaces;
use HyperUuid\Uuid;
use HyperUuid\UuidLayout;
use HyperUuid\UuidVariant;
use PHPUnit\Framework\TestCase;

/**
 * Version/variant inspection and the layout-aware doors against values minted at run time,
 * by this library and by ramsey/uuid; {@see CorpusTest} pins the fixed vectors.
 */
final class InspectionTest extends TestCase
{
    private const MS = 1_645_557_742_000;

    public function testLibraryAndRamseyValuesReportTheirVersionAndTheRfcVariant(): void
    {
        $cases = [
            [new Uuid(\Ramsey\Uuid\Uuid::uuid4()->getBytes()), 4],
            [new Uuid(\Ramsey\Uuid\Uuid::uuid6()->getBytes()), 6],
            [new Uuid(\Ramsey\Uuid\Uuid::uuid7()->getBytes()), 7],
            [HyperUuid::newV4(), 4],
            [HyperUuid::newV5(Namespaces::dns(), 'x'), 5],
            [HyperUuid::newV6(self::MS), 6],
            [HyperUuid::newV7(self::MS), 7],
        ];
        foreach ($cases as [$id, $version]) {
            self::assertSame($version, $id->version(), (string) $id);
            self::assertSame(UuidVariant::Rfc9562, $id->variant(), (string) $id);
            self::assertTrue($id->isRfc($version), (string) $id);
            self::assertFalse($id->isRfc($version === 7 ? 6 : 7), (string) $id);
        }
    }

    public function testNilAndMaxAreClassifiedAsTheRfcSays(): void
    {
        self::assertSame(0, Uuid::nil()->version());
        self::assertSame(UuidVariant::Ncs, Uuid::nil()->variant());
        self::assertSame(15, Uuid::max()->version());
        self::assertSame(UuidVariant::Future, Uuid::max()->variant());
        self::assertFalse(Uuid::nil()->isRfc(0));
        self::assertFalse(Uuid::max()->isRfc(15));
    }

    public function testAVersionOutsideTheNibbleNeverMatches(): void
    {
        $id = HyperUuid::newV7(self::MS);
        self::assertFalse($id->isRfc(7 + 256));
        self::assertFalse($id->isRfc(-1));
        // Wider than the C ABI's uint32_t: must not wrap around onto 7.
        self::assertFalse($id->isRfc(0x1_0000_0007));
        self::assertFalse($id->isRfc(PHP_INT_MIN + 7));
    }

    /**
     * In SQL order a v6's random clock_seq sits where a v7's version nibble does and reads as
     * 7 one time in 16; enough draws that a confusion would surface.
     */
    public function testSqlOrderedV6AndV7AreValidatedAndReadInPlaceWithoutConfusion(): void
    {
        $sql = UuidLayout::SqlServer;
        for ($i = 0; $i < 2048; $i++) {
            $six = HyperUuid::newV6(self::MS)->toSqlOrder();
            $seven = HyperUuid::newV7(self::MS)->toSqlOrder();

            self::assertSame(6, $six->version($sql));
            self::assertSame(7, $seven->version($sql));
            self::assertTrue($six->isRfc(6, $sql));
            self::assertFalse($six->isRfc(7, $sql));
            self::assertTrue($seven->isRfc(7, $sql));
            self::assertFalse($seven->isRfc(6, $sql));

            self::assertSame(self::MS, $six->unixMillis($sql));
            self::assertSame(self::MS, $seven->unixMillis($sql));
            self::assertEquals($six->fromSqlOrder()->timestamp(), $six->timestamp(layout: $sql));
            self::assertEquals($seven->fromSqlOrder()->timestamp(), $seven->timestamp(layout: $sql));
            self::assertSame(6, $six->fromSqlOrder()->version());
            self::assertSame(7, $seven->fromSqlOrder()->version());
        }
    }

    /**
     * Fixed values only: a Uuid holds its bytes as given, so a random v4 (variant already in
     * octet 8) reads as a SQL-ordered v7 whenever its random octet 7 starts with 7 — one time
     * in 16, and genuinely indistinguishable from one. RFC 9562's A.3 v4 has 0x20 there.
     */
    public function testNonSqlValuesHaveNoSqlVersion(): void
    {
        $sql = UuidLayout::SqlServer;
        $v4 = Uuid::parse('919108f7-52d1-4320-9bac-f847db4148a8');
        foreach ([$v4, Uuid::nil(), Uuid::max(), HyperUuid::newV5(Namespaces::dns(), 'x')] as $id) {
            self::assertSame(0, $id->version($sql), (string) $id);
            self::assertNull($id->unixMillis($sql), (string) $id);
            self::assertNull($id->timestamp(false, $sql), (string) $id);
            try {
                $id->timestamp(layout: $sql);
                self::fail("timestamp() accepted {$id} in SQL Server order");
            } catch (\InvalidArgumentException $e) {
                self::assertStringContainsString('got version 0 in SqlServer order', $e->getMessage());
            }
            try {
                $id->fromSqlOrder();
                self::fail("fromSqlOrder() accepted {$id}");
            } catch (\InvalidArgumentException $e) {
                self::assertStringContainsString('not a SQL-ordered version 6 or 7 UUID', $e->getMessage());
            }
        }
    }

    /**
     * A 6 or 7 nibble under another variant has no RFC version, so it has no timestamp:
     * {@see Uuid::version()} still reads the nibble, but the timestamp doors refuse it.
     */
    public function testATimeBasedNibbleUnderAnotherVariantHasNoTimestamp(): void
    {
        $bytes = HyperUuid::newV7(self::MS)->bytes();
        foreach ([0x00, 0xC0, 0xE0] as $variantBits) { // NCS, Microsoft, Future
            $id = new Uuid(substr_replace($bytes, \chr((\ord($bytes[8]) & 0x1F) | $variantBits), 8, 1));
            self::assertSame(7, $id->version(), (string) $id);
            self::assertNotSame(UuidVariant::Rfc9562, $id->variant(), (string) $id);
            self::assertNull($id->unixMillis(), (string) $id);
            self::assertNull($id->timestamp(false), (string) $id);
            try {
                $id->timestamp();
                self::fail("timestamp() accepted {$id}");
            } catch (\InvalidArgumentException $e) {
                self::assertStringContainsString('got version 7 with a non-RFC 9562 variant', $e->getMessage());
            }
        }
    }

    /**
     * A layout is an enum, so there is no unset or out-of-range one to pass: the codes the
     * core doesn't define have no case, and anything that isn't a UuidLayout is a TypeError
     * before it gets near the core.
     */
    public function testAnUndefinedLayoutCannotBePassed(): void
    {
        self::assertNull(UuidLayout::tryFrom(0));
        self::assertNull(UuidLayout::tryFrom(3));
        self::assertSame([1, 2], array_map(static fn (UuidLayout $l): int => $l->value, UuidLayout::cases()));
        $id = HyperUuid::newV7(self::MS);
        $doors = [
            'version' => static fn () => $id->version(2),
            'isRfc' => static fn () => $id->isRfc(7, 2),
            'unixMillis' => static fn () => $id->unixMillis(2),
            'timestamp' => static fn () => $id->timestamp(true, 2),
        ];
        foreach ($doors as $door => $call) {
            try {
                $call();
                self::fail("{$door} accepted an int layout");
            } catch (\TypeError) {
                self::addToAssertionCount(1);
            }
        }
    }

    /**
     * The v7 batch limit is the counter space, enforced before the native buffer is allocated:
     * a refused count past it costs no memory.
     */
    public function testAV7BatchPastTheCounterSpaceIsRefusedOnEveryDoor(): void
    {
        self::assertSame(67_108_864, HyperUuid::MAX_V7_BATCH);
        foreach (['newV7Batch', 'newV7BatchBytes'] as $door) {
            memory_reset_peak_usage();
            $before = memory_get_peak_usage();
            try {
                HyperUuid::$door(HyperUuid::MAX_V7_BATCH + 1, self::MS);
                self::fail("{$door} accepted a batch past the counter space");
            } catch (\InvalidArgumentException $e) {
                self::assertStringContainsString((string) HyperUuid::MAX_V7_BATCH, $e->getMessage());
                self::assertStringContainsString('got ' . (HyperUuid::MAX_V7_BATCH + 1), $e->getMessage());
            }
            self::assertLessThan(1 << 20, memory_get_peak_usage() - $before, $door);
        }
        // Version 6 has no counter and no such limit (count only has to fit the uint32_t).
        self::assertSame('', HyperUuid::newV6BatchBytes(0, self::MS));
    }

    /**
     * Exactly the counter space: 1 GiB of bytes (2 GiB peak, the native buffer plus the PHP
     * string), in strictly increasing order end to end, and at most a millisecond past the
     * supplied timestamp (the roll-forward over the wrap). Opt-in through
     * HYPERUUID_HUGE_TESTS=1, run with `-d memory_limit=-1`.
     */
    public function testAV7BatchOfExactlyTheCounterSpaceIsStrictlyIncreasing(): void
    {
        if (getenv('HYPERUUID_HUGE_TESTS') !== '1') {
            self::markTestSkipped('needs 2 GiB; set HYPERUUID_HUGE_TESTS=1 and -d memory_limit=-1');
        }
        $bytes = HyperUuid::newV7BatchBytes(HyperUuid::MAX_V7_BATCH, self::MS);
        self::assertSame(HyperUuid::MAX_V7_BATCH * 16, \strlen($bytes));
        $previous = substr($bytes, 0, 16);
        $ordered = true;
        for ($i = 1; $i < HyperUuid::MAX_V7_BATCH; $i++) {
            $current = substr($bytes, $i * 16, 16);
            if ($current <= $previous) {
                $ordered = false;
                break;
            }
            $previous = $current;
        }
        self::assertTrue($ordered, "not strictly increasing at item {$i}");
        $last = (new Uuid($previous))->unixMillis();
        self::assertGreaterThanOrEqual(self::MS, $last);
        self::assertLessThanOrEqual(self::MS + 1, $last);
    }
}
