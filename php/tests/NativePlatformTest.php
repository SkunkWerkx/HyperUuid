<?php

declare(strict_types=1);

namespace HyperUuid\Tests;

use HyperUuid\NativePlatform;
use PHPUnit\Framework\Attributes\DataProvider;
use PHPUnit\Framework\TestCase;

/**
 * The platform table as a pure function, so every row is checked from whichever one
 * platform the suite happens to run on.
 */
final class NativePlatformTest extends TestCase
{
    /** @return iterable<string, array{string, string, bool, string, string}> */
    public static function supported(): iterable
    {
        yield 'linux glibc x64' => ['Linux', 'x86_64', false, 'linux-x64', 'libhyperuuid.so'];
        yield 'linux glibc arm64' => ['Linux', 'aarch64', false, 'linux-arm64', 'libhyperuuid.so'];
        yield 'linux musl x64' => ['Linux', 'x86_64', true, 'linux-musl-x64', 'libhyperuuid.so'];
        yield 'linux musl arm64' => ['Linux', 'aarch64', true, 'linux-musl-arm64', 'libhyperuuid.so'];
        yield 'macOS intel' => ['Darwin', 'x86_64', false, 'osx-x64', 'libhyperuuid.dylib'];
        yield 'macOS apple silicon' => ['Darwin', 'arm64', false, 'osx-arm64', 'libhyperuuid.dylib'];
        yield 'windows x64' => ['Windows', 'AMD64', false, 'win-x64', 'hyperuuid.dll'];
        // PHP on Windows is an x64 process even on ARM hardware, where php_uname('m') still
        // reports the machine — the ARM64 DLL is never the one to load.
        yield 'windows on arm hardware' => ['Windows', 'ARM64', false, 'win-x64', 'hyperuuid.dll'];
        // The machine string is matched case-insensitively.
        yield 'uppercase machine' => ['Linux', 'AARCH64', false, 'linux-arm64', 'libhyperuuid.so'];
    }

    #[DataProvider('supported')]
    public function testResolvesTheRidAndLibraryName(
        string $osFamily,
        string $machine,
        bool $musl,
        string $rid,
        string $library
    ): void {
        self::assertSame([$rid, $library], NativePlatform::resolve($osFamily, $machine, 8, $musl));
    }

    /** @return iterable<string, array{string, string, int}> */
    public static function unsupported(): iterable
    {
        yield '32-bit arm' => ['Linux', 'armv7l', 8];
        yield 'riscv' => ['Linux', 'riscv64', 8];
        yield 'power' => ['Linux', 'ppc64le', 8];
        yield '32-bit x86' => ['Linux', 'i686', 8];
        yield 'an OS family with no library' => ['BSD', 'amd64', 8];
        yield '32-bit PHP on a 64-bit machine' => ['Linux', 'x86_64', 4];
        yield '32-bit PHP on Windows' => ['Windows', 'AMD64', 4];
    }

    #[DataProvider('unsupported')]
    public function testAnUnsupportedPlatformIsAClearError(string $osFamily, string $machine, int $intSize): void
    {
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionMessage('hyperuuid: unsupported platform');
        NativePlatform::resolve($osFamily, $machine, $intSize, false);
    }

    public function testTheRunningPlatformResolves(): void
    {
        [$rid, $library] = NativePlatform::ridAndLibraryName();
        self::assertMatchesRegularExpression('/\A(linux(-musl)?|osx|win)-(x64|arm64)\z/', $rid);
        self::assertStringContainsString('hyperuuid', $library);
    }
}
