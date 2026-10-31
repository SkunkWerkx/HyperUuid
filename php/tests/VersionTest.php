<?php

declare(strict_types=1);

namespace HyperUuid\Tests;

use HyperUuid\HyperUuid;
use HyperUuid\UuidLayout;
use HyperUuid\UuidVariant;
use PHPUnit\Framework\TestCase;

final class VersionTest extends TestCase
{
    public function testPhpversionMatchesNativeVersion(): void
    {
        self::assertSame(phpversion('hyperuuid'), HyperUuid::nativeVersion());
        self::assertMatchesRegularExpression('/^\d+\.\d+\.\d+$/', (string) phpversion('hyperuuid'));
    }

    public function testEnumsCarryTheCoreCodes(): void
    {
        self::assertSame(2, UuidLayout::SqlServer->value);
        self::assertSame(3, UuidVariant::Microsoft->value);
    }

    public function testExceptionsAreFinalRuntimeExceptions(): void
    {
        foreach (['CounterExhausted', 'TimestampOutOfRange', 'RandomSource'] as $name) {
            $class = "HyperUuid\\{$name}Exception";
            self::assertTrue(is_subclass_of($class, \RuntimeException::class), $class);
            self::assertTrue((new \ReflectionClass($class))->isFinal(), $class);
        }
    }
}
