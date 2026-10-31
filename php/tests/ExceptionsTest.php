<?php

declare(strict_types=1);

namespace HyperUuid\Tests;

use HyperUuid\CounterExhaustedException;
use HyperUuid\HyperUuid;
use HyperUuid\RandomSourceException;
use HyperUuid\TimestampOutOfRangeException;
use PHPUnit\Framework\TestCase;

/**
 * The three exceptions behave as the 1.0 binding's `final class X extends \RuntimeException {}`
 * did: built by the engine's exception machinery, so they carry the call site and a trace,
 * and constructible from PHP with RuntimeException's own constructor.
 */
final class ExceptionsTest extends TestCase
{
    public function testAThrownExceptionPointsAtTheCallSite(): void
    {
        try {
            $line = __LINE__ + 1;
            HyperUuid::newV7(1 << 48);
            self::fail('newV7 accepted a timestamp past 48 bits');
        } catch (TimestampOutOfRangeException $e) {
            self::assertSame(__FILE__, $e->getFile());
            self::assertSame($line, $e->getLine());
            self::assertNotEmpty($e->getTrace());
            self::assertSame('newV7', $e->getTrace()[0]['function'] ?? null);
            self::assertStringContainsString('HyperUuid\\HyperUuid::newV7(', $e->getTraceAsString());
        }
    }

    /** @return iterable<string, array{class-string<\RuntimeException>}> */
    public static function exceptionClasses(): iterable
    {
        yield 'CounterExhausted' => [CounterExhaustedException::class];
        yield 'TimestampOutOfRange' => [TimestampOutOfRangeException::class];
        yield 'RandomSource' => [RandomSourceException::class];
    }

    #[\PHPUnit\Framework\Attributes\DataProvider('exceptionClasses')]
    public function testConstructibleFromPhpWithRuntimeExceptionsConstructor(string $class): void
    {
        $previous = new \LogicException('cause');
        $line = __LINE__ + 1;
        $e = new $class('m', 7, $previous);
        self::assertInstanceOf(\RuntimeException::class, $e);
        self::assertSame('m', $e->getMessage());
        self::assertSame(7, $e->getCode());
        self::assertSame($previous, $e->getPrevious());
        self::assertSame(__FILE__, $e->getFile());
        self::assertSame($line, $e->getLine());

        $bare = new $class();
        self::assertSame('', $bare->getMessage());
        self::assertSame(0, $bare->getCode());
        self::assertNull($bare->getPrevious());
        self::assertTrue((new \ReflectionClass($class))->isFinal());
    }
}
