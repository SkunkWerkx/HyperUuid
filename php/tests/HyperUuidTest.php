<?php

declare(strict_types=1);

namespace HyperUuid\Tests;

use HyperUuid\CounterExhaustedException;
use HyperUuid\HyperUuid;
use HyperUuid\Namespaces;
use HyperUuid\TimestampOutOfRangeException;
use HyperUuid\Uuid;
use HyperUuid\UuidLayout;
use HyperUuid\UuidVariant;
use PHPUnit\Framework\Attributes\DataProvider;
use PHPUnit\Framework\Attributes\RunInSeparateProcess;
use PHPUnit\Framework\TestCase;

final class HyperUuidTest extends TestCase
{
    private const RFC_TEST_VECTOR_MS = 1_645_557_742_000;

    private const WRONG_VERSION = 'the value is not a UUID of the version this conversion takes';

    /** RFC 9562 Appendix A.5's version 6 test vector. */
    private const RFC_V6_TEST_VECTOR = '1ec9414c-232a-6b00-b3c8-9f6bdeced846';

    public function testV4HasVersionAndVariantBits(): void
    {
        $id = HyperUuid::newV4();
        self::assertSame(4, $id->version());
        self::assertSame(UuidVariant::Rfc9562, $id->variant());
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
        self::assertSame(UuidVariant::Rfc9562, $id->variant());
    }

    public function testV7OverflowTimestampThrows(): void
    {
        $this->expectException(TimestampOutOfRangeException::class);
        HyperUuid::newV7(0x0001_0000_0000_0000);
    }

    public function testV7SameMillisecondBatchIsMonotonicallyOrdered(): void
    {
        $now = self::nowMillis();
        $ids = [];
        for ($i = 0; $i < 100; $i++) {
            $ids[] = (string) HyperUuid::newV7($now);
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
        $now = self::nowMillis();
        $before = HyperUuid::newV7($now);
        $batch = HyperUuid::newV7Batch(10, $now);
        $after = HyperUuid::newV7($now);

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
        // The stream may stamp a later ms that an earlier test moved it to.
        self::assertLessThanOrEqual($after + 1000, $embedded);
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

    public function testTimestampThrowsForNonTimeBasedVersionByDefault(): void
    {
        $this->expectException(\InvalidArgumentException::class);
        HyperUuid::newV4()->timestamp();
    }

    public function testTimestampReturnsNullForNonTimeBasedVersionWhenThrowOnMismatchIsFalse(): void
    {
        self::assertNull(HyperUuid::newV4()->timestamp(throwOnMismatch: false));
    }

    public function testTimestampStillReturnsTheRealTimestampWhenThrowOnMismatchIsFalse(): void
    {
        $id = HyperUuid::newV7(self::RFC_TEST_VECTOR_MS);
        self::assertEquals($id->timestamp(), $id->timestamp(throwOnMismatch: false));
    }

    public function testNewV7AcceptsADateTimeInterfaceInPlaceOfAMillisecondInt(): void
    {
        $dt = \DateTimeImmutable::createFromFormat('U', (string) intdiv(self::RFC_TEST_VECTOR_MS, 1000));
        $id = HyperUuid::newV7($dt);
        self::assertSame(intdiv(self::RFC_TEST_VECTOR_MS, 1000) * 1000, self::millis($id->timestamp()));
    }

    public function testV7ToSqlOrderRoundTripsThroughFromSqlOrder(): void
    {
        $id = HyperUuid::newV7(self::RFC_TEST_VECTOR_MS);
        $sqlOrdered = $id->toSqlOrder();
        self::assertFalse($id->equals($sqlOrdered));
        self::assertTrue($id->equals($sqlOrdered->fromSqlOrder()));
    }

    public function testV7ToSqlOrderWritesTheVersion8MarkerAtOctet7AndTheVariantAtOctet8(): void
    {
        $bytes = HyperUuid::newV7(self::RFC_TEST_VECTOR_MS)->toSqlOrder()->bytes();
        self::assertSame(0x80, \ord($bytes[7]) & 0xF0);
        self::assertSame(0x80, \ord($bytes[8]) & 0xC0);
    }

    public function testToSqlOrderThrowsForUnsupportedVersion(): void
    {
        $this->expectException(\InvalidArgumentException::class);
        $this->expectExceptionMessage(self::WRONG_VERSION);
        HyperUuid::newV4()->toSqlOrder();
    }

    public function testEveryV7ConvertsToSqlOrderUpToTheLargestTimestamp(): void
    {
        // The layout holds all 122 free bits of a version 7, so no timestamp is out of range.
        foreach ([0, (1 << 47) - 1, 1 << 47, 0x0000_FFFF_FFFF_FFFF] as $millis) {
            $id = HyperUuid::newV7($millis);
            $sql = $id->toSqlOrder();
            self::assertSame($millis, $sql->unixMillis(UuidLayout::SqlServer), (string) $id);
            self::assertTrue($id->equals($sql->fromSqlOrder()), (string) $id);
        }
    }

    public function testToSqlOrderRefusesAVersion6(): void
    {
        $id = Uuid::parse(self::RFC_V6_TEST_VECTOR);
        try {
            $id->toSqlOrder();
            self::fail('toSqlOrder() accepted a version 6');
        } catch (\InvalidArgumentException $e) {
            self::assertSame(self::WRONG_VERSION, $e->getMessage());
        }
        self::assertSame(self::RFC_V6_TEST_VECTOR, (string) $id);
    }

    /**
     * Proves timestamp() isn't just reading back what our own newV7 wrote — it's a plain
     * RFC 9562 bit-layout read, so it recovers the real embedded timestamp from a version 7
     * UUID minted by ramsey/uuid's own independent implementation too.
     */
    public function testTimestampExtractsFromRamseyUuidsNativeV7Generator(): void
    {
        $ramsey = \Ramsey\Uuid\Uuid::uuid7();
        $ours = new Uuid($ramsey->getBytes());

        self::assertSame(7, $ours->version());
        // Millisecond precision only — ramsey's getDateTime() carries microseconds for v7,
        // but the RFC 9562 field itself (and this binding's timestamp()) is millisecond-only,
        // so anything finer isn't part of what's actually embedded in the UUID to compare.
        self::assertSame(self::millis($ramsey->getDateTime()), self::millis($ours->timestamp()));
    }

    /**
     * A version 6 from another generator still reports its version nibble, but this library
     * reads no version 6 timestamp: unixMillis() answers null and timestamp() treats it like
     * any other version without one.
     */
    public function testAVersion6FromAnotherGeneratorHasNoTimestamp(): void
    {
        foreach ([Uuid::parse(self::RFC_V6_TEST_VECTOR), new Uuid(\Ramsey\Uuid\Uuid::uuid6()->getBytes())] as $id) {
            self::assertSame(6, $id->version(), (string) $id);
            self::assertNull($id->unixMillis(), (string) $id);
            self::assertNull($id->timestamp(throwOnMismatch: false), (string) $id);
            try {
                $id->timestamp();
                self::fail("timestamp() accepted {$id}");
            } catch (\InvalidArgumentException $e) {
                self::assertSame(
                    'timestamp() is only defined for RFC 9562 version 7 UUIDs, got version 6',
                    $e->getMessage()
                );
            }
        }
    }

    private static function millis(\DateTimeInterface $dt): int
    {
        // Integer arithmetic throughout — float parsing of 'U.u' is lossy right at millisecond
        // boundaries and produced real off-by-one failures here during development.
        return ((int) $dt->format('U')) * 1000 + intdiv((int) $dt->format('u'), 1000);
    }

    private static function nowMillis(): int
    {
        return (int) round(microtime(true) * 1000);
    }

    /**
     * Replicates System.Data.SqlTypes.SqlGuid.CompareTo's fixed byte significance order — the
     * correctness oracle this project's C# test suite checks directly against the real type;
     * no PHP equivalent exists to test against here, so this stands in for it.
     */
    private static function sqlGuidCompare(string $a, string $b): int
    {
        foreach ([10, 11, 12, 13, 14, 15, 8, 9, 6, 7, 4, 5, 0, 1, 2, 3] as $i) {
            $cmp = \ord($a[$i]) <=> \ord($b[$i]);
            if ($cmp !== 0) {
                return $cmp;
            }
        }
        return 0;
    }

    public function testV7ToSqlOrderSortsByCreationOrderUnderSqlGuidComparison(): void
    {
        // Current-time stamps, 500 ms ahead at most: a fixed old timestamp is minted outside the
        // v7 stream, which orders nothing, and one far ahead would drag the stream away from the clock.
        // No test supplies more than now + 500 ms, and current-time bounds allow + 1000 ms.
        $now = self::nowMillis();
        $ids = [];
        for ($i = 0; $i < 200; $i++) {
            $ids[] = HyperUuid::newV7($now + $i);
        }
        // Same-millisecond run, so the counter (not just the timestamp) has to sort correctly too.
        for ($i = 0; $i < 200; $i++) {
            $ids[] = HyperUuid::newV7($now + 500);
        }

        $sqlOrdered = array_map(static fn (Uuid $id): string => $id->toSqlOrder()->bytes(), $ids);
        $sorted = $sqlOrdered;
        usort($sorted, [self::class, 'sqlGuidCompare']);

        self::assertSame($sqlOrdered, $sorted);
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

    public function testNewV7BatchBytesReturnsRawBytes(): void
    {
        $bytes = HyperUuid::newV7BatchBytes(64, self::RFC_TEST_VECTOR_MS);
        self::assertIsString($bytes);
        self::assertSame(64 * 16, strlen($bytes));
    }

    public function testNewV7BatchBytesDecodesToVersion7(): void
    {
        $bytes = HyperUuid::newV7BatchBytes(32, self::RFC_TEST_VECTOR_MS);
        for ($i = 0; $i < 32; $i++) {
            $id = new Uuid(substr($bytes, $i * 16, 16));
            self::assertSame(7, $id->version(), "item {$i}");
        }
    }

    public function testNewV7BatchBytesIsStrictlyIncreasing(): void
    {
        $bytes = HyperUuid::newV7BatchBytes(256, self::RFC_TEST_VECTOR_MS);
        $raw = [];
        for ($i = 0; $i < 256; $i++) {
            $raw[] = substr($bytes, $i * 16, 16);
        }
        $sorted = $raw;
        sort($sorted);
        self::assertSame($sorted, $raw, 'batch is not in creation order');
        self::assertCount(256, array_unique($raw), 'batch contains duplicates');
    }

    public function testNewV7BatchBytesAgreesWithTheObjectForm(): void
    {
        $now = self::nowMillis();
        // Two separate batches, so the values differ — what must agree is everything the
        // two doors share: the layout, the embedded timestamp, and one counter sequence
        // (the object batch, minted second, continues above the byte batch).
        $bytes = HyperUuid::newV7BatchBytes(16, $now);
        $fromObjects = HyperUuid::newV7Batch(16, $now);
        $previous = '';
        for ($i = 0; $i < 16; $i++) {
            $fromBytes = new Uuid(substr($bytes, $i * 16, 16));
            self::assertSame($fromObjects[$i]->version(), $fromBytes->version(), "item {$i}");
            self::assertSame($fromObjects[$i]->variant(), $fromBytes->variant(), "item {$i}");
            self::assertSame(self::embeddedMillis($fromObjects[$i]), self::embeddedMillis($fromBytes), "item {$i}");
            $previous = $fromBytes->bytes();
        }
        self::assertGreaterThan(0, strcmp($fromObjects[0]->bytes(), $previous));
    }

    public function testBatchBytesDefaultsToNow(): void
    {
        $before = (int) (microtime(true) * 1000);
        $bytes = HyperUuid::newV7BatchBytes(1);
        $after = (int) (microtime(true) * 1000);
        $ts = (new Uuid(substr($bytes, 0, 16)))->timestamp();
        $ms = (int) ((float) $ts->format('U.u') * 1000);
        self::assertGreaterThanOrEqual($before - 1000, $ms);
        self::assertLessThanOrEqual($after + 1000, $ms);
    }

    /** @return iterable<string, array{string}> */
    public static function batchDoors(): iterable
    {
        yield 'newV7Batch' => ['newV7Batch'];
        yield 'newV7BatchBytes' => ['newV7BatchBytes'];
    }

    #[DataProvider('batchDoors')]
    public function testANegativeBatchCountIsACallerBug(string $door): void
    {
        $this->expectException(\InvalidArgumentException::class);
        // The message names the counter-space limit (HyperUuid::MAX_V7_BATCH).
        $this->expectExceptionMessageMatches('/^count must be between 0 and \\d+.*, got -1$/');
        HyperUuid::$door(-1, self::RFC_TEST_VECTOR_MS);
    }

    #[DataProvider('batchDoors')]
    public function testABatchCountWiderThan32BitsIsACallerBug(string $door): void
    {
        // The C ABI's count is a uint32_t — without the guard this would truncate to 0.
        $this->expectException(\InvalidArgumentException::class);
        HyperUuid::$door(0x1_0000_0000, self::RFC_TEST_VECTOR_MS);
    }

    public function testBatchBytesCountZeroReturnsAnEmptyString(): void
    {
        self::assertSame('', HyperUuid::newV7BatchBytes(0, self::RFC_TEST_VECTOR_MS));
    }

    public function testParseAcceptsTheHyphenatedFormInEitherCase(): void
    {
        $lower = Uuid::parse('6ba7b810-9dad-11d1-80b4-00c04fd430c8');
        $upper = Uuid::parse('6BA7B810-9DAD-11D1-80B4-00C04FD430C8');
        self::assertTrue($lower->equals($upper));
        self::assertSame('6ba7b810-9dad-11d1-80b4-00c04fd430c8', (string) $upper);
        self::assertSame(hex2bin('6ba7b8109dad11d180b400c04fd430c8'), $lower->bytes());
    }

    /** @return iterable<string, array{string}> */
    public static function malformedUuidStrings(): iterable
    {
        yield 'empty' => [''];
        yield 'unhyphenated' => ['6ba7b8109dad11d180b400c04fd430c8'];
        yield 'hyphens in the wrong places' => ['6ba7b81-09dad-11d1-80b4-00c04fd430c8'];
        yield 'extra hyphens' => ['6ba7-b810-9dad-11d1-80b4-00c04fd430c8'];
        yield 'braced' => ['{6ba7b810-9dad-11d1-80b4-00c04fd430c8}'];
        yield 'urn prefix' => ['urn:uuid:6ba7b810-9dad-11d1-80b4-00c04fd430c8'];
        yield 'non-hex digit' => ['6ba7b810-9dad-11d1-80b4-00c04fd430cg'];
        yield 'too short' => ['6ba7b810-9dad-11d1-80b4-00c04fd430c'];
        yield 'too long' => ['6ba7b810-9dad-11d1-80b4-00c04fd430c80'];
        yield 'trailing newline' => ["6ba7b810-9dad-11d1-80b4-00c04fd430c8\n"];
        yield 'surrounding whitespace' => [' 6ba7b810-9dad-11d1-80b4-00c04fd430c8 '];
    }

    #[DataProvider('malformedUuidStrings')]
    public function testParseRejectsEverythingButTheHyphenatedForm(string $string): void
    {
        $this->expectException(\InvalidArgumentException::class);
        Uuid::parse($string);
    }

    public function testConstructorRejectsAnythingButSixteenBytes(): void
    {
        foreach (['', str_repeat("\x00", 15), str_repeat("\x00", 17)] as $bytes) {
            try {
                new Uuid($bytes);
                self::fail('a ' . \strlen($bytes) . '-byte value should have been rejected');
            } catch (\InvalidArgumentException) {
                $this->addToAssertionCount(1);
            }
        }
    }

    public function testJsonEncodesAsTheHyphenatedString(): void
    {
        $id = Uuid::parse('6ba7b810-9dad-11d1-80b4-00c04fd430c8');
        self::assertSame('"6ba7b810-9dad-11d1-80b4-00c04fd430c8"', json_encode($id));
        self::assertSame('{"id":"6ba7b810-9dad-11d1-80b4-00c04fd430c8"}', json_encode(['id' => $id]));
        self::assertInstanceOf(\Stringable::class, $id);
    }

    public function testFromSqlOrderThrowsWhenNeitherInverseDecodes(): void
    {
        // Nil has no SQL layout version 8 and variant, so it is no SQL-ordered value.
        $this->expectException(\InvalidArgumentException::class);
        Uuid::nil()->fromSqlOrder();
    }

    public function testNewV7EmbedsADateTimesMilliseconds(): void
    {
        // Sub-second precision survives to the millisecond, and no further.
        $dt = new \DateTimeImmutable('2022-02-22T19:22:22.123456Z');
        $expected = $dt->getTimestamp() * 1000 + 123;
        self::assertSame($expected, self::embeddedMillis(HyperUuid::newV7($dt)));
    }

    /**
     * Drives the real stream to its ceiling, which poisons the process-wide v7 state for
     * everything after it, so the whole script runs in a process of its own.
     */
    #[RunInSeparateProcess]
    public function testCounterExhaustion(): void
    {
        $m = 17_592_186_044_414; // 2**44 - 2, the last millisecond the stream holds
        $message = 'version 7 counter exhausted: the stream cannot advance without running more than '
            . '1000 ms ahead of the supplied timestamp or past its last millisecond';
        $exhausted = static function (callable $call) use ($message): bool {
            try {
                $call();
            } catch (CounterExhaustedException $e) {
                self::assertSame($message, $e->getMessage());
                return true;
            }
            return false;
        };

        // 1. One single v7 at M succeeds.
        HyperUuid::newV7($m);

        // 2. Batches of 65,536 at M until the first error: the batch site.
        $batchExhausted = false;
        for ($i = 0; $i < 1025 && !$batchExhausted; $i++) {
            $batchExhausted = $exhausted(static fn () => HyperUuid::newV7BatchBytes(65536, $m));
        }
        self::assertTrue($batchExhausted, 'no batch exhausted the stream');

        // 3. Singles at M until the first error: the single site.
        $singleExhausted = false;
        for ($i = 0; $i < 1024 && !$singleExhausted; $i++) {
            $singleExhausted = $exhausted(static fn () => HyperUuid::newV7($m));
        }
        self::assertTrue($singleExhausted, 'no single exhausted the stream');

        // 4. Every other v7 door now fails at once.
        self::assertTrue($exhausted(static fn () => HyperUuid::newV7($m)), 'newV7');
        self::assertTrue($exhausted(static fn () => HyperUuid::newV7Batch(1, $m)), 'newV7Batch');
        self::assertTrue($exhausted(static fn () => HyperUuid::newV7BatchBytes(1, $m)), 'newV7BatchBytes');
    }

    public function testNativeVersionNamesTheLoadedLibrary(): void
    {
        self::assertSame(self::crateVersion(), HyperUuid::nativeVersion());
    }

    public function testIsAvailableIsTheNonThrowingProbe(): void
    {
        self::assertTrue(HyperUuid::isAvailable());
        // Cached and idempotent — the second answer is the first, no reload.
        self::assertTrue(HyperUuid::isAvailable());
    }

    public function testIsAvailableAnswersFalseWhenFfiIsDisabled(): void
    {
        // ffi.enable=0 refuses the FFI API even on the CLI, which is exactly what a
        // restricted web SAPI looks like from inside the binding.
        self::assertSame('unavailable ffi', self::probe(\dirname(__DIR__) . '/src', '-d', 'ffi.enable=0'));
    }

    public function testIsAvailableAnswersFalseWhenTheExtensionIsMissing(): void
    {
        // -n drops every ini file, and with them a shared ext-ffi. A PHP with ext-ffi
        // compiled in statically has nothing to drop, so there is nothing to prove there.
        $answer = self::probe(\dirname(__DIR__) . '/src', '-n');
        if (str_ends_with($answer, ' ffi')) {
            self::markTestSkipped('ext-ffi is compiled into this PHP and cannot be unloaded');
        }
        self::assertSame('unavailable no-ffi', $answer);
    }

    public function testIsAvailableAnswersFalseWhenTheLibraryIsMissing(): void
    {
        // A copy of the binding with no native/ directory beside it, far from any cargo
        // build the development fallback could find.
        $root = sys_get_temp_dir() . '/hyperuuid-probe-' . bin2hex(random_bytes(6));
        $src = $root . '/php/src';
        mkdir($src, 0o777, true);
        try {
            foreach (glob(\dirname(__DIR__) . '/src/*.php') as $file) {
                copy($file, $src . '/' . basename($file));
            }
            self::assertSame('unavailable ffi', self::probe($src));
        } finally {
            array_map('unlink', glob($src . '/*.php'));
            rmdir($src);
            rmdir($root . '/php');
            rmdir($root);
        }
    }

    /**
     * Runs tests/fixtures/probe.php in a child PHP — the only way to observe a process in
     * which the binding cannot load — and returns what it printed.
     */
    private static function probe(string $src, string ...$phpFlags): string
    {
        // Without the dev-loop override, or the probe would load the library it names.
        $env = array_diff_key(getenv(), ['HYPERUUID_NATIVE_LIBRARY' => true]);
        $process = proc_open(
            [PHP_BINARY, ...$phpFlags, __DIR__ . '/fixtures/probe.php', $src],
            [1 => ['pipe', 'w'], 2 => ['pipe', 'w']],
            $pipes,
            null,
            $env
        );
        self::assertIsResource($process);
        $stdout = stream_get_contents($pipes[1]);
        $stderr = stream_get_contents($pipes[2]);
        self::assertSame(0, proc_close($process), "probe failed: {$stdout}{$stderr}");
        return $stdout;
    }

    /** The crate's own manifest version, walked up to from here. */
    private static function crateVersion(): string
    {
        $dir = __DIR__;
        // Stop when dirname() stops moving, not at '/': a Windows root is 'C:\\', never '/'.
        for ($parent = \dirname($dir); $parent !== $dir; $dir = $parent, $parent = \dirname($dir)) {
            $candidate = $dir . '/rust/Cargo.toml';
            if (is_file($candidate)) {
                self::assertSame(
                    1,
                    preg_match('/^version\s*=\s*"([^"]+)"/m', file_get_contents($candidate), $match),
                    'rust/Cargo.toml carries no package version'
                );
                return $match[1];
            }
        }
        self::fail('rust/Cargo.toml not found');
    }
}
