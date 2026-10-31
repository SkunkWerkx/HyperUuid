<?php

declare(strict_types=1);

namespace HyperUuid\Tests;

use HyperUuid\Tests\Support\ChildProcess;
use PHPUnit\Framework\TestCase;

final class AvailabilityTest extends TestCase
{
    use ChildProcess;

    private const PROBE = 'echo (int) extension_loaded("hyperuuid"), " ", '
        . '(int) class_exists("HyperUuid\\\\HyperUuid");';

    public function testLoadedExtensionDefinesTheApi(): void
    {
        self::assertSame('1 1', self::child(['-d', 'extension=' . self::ext()], self::PROBE));
    }

    public function testWithoutTheExtensionNothingIsDefined(): void
    {
        self::assertSame('0 0', self::child(['-n'], self::PROBE));
    }
}
