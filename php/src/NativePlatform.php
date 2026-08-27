<?php

declare(strict_types=1);

namespace HyperUuid;

/**
 * Maps the running PHP_OS_FAMILY / php_uname('m') to the RID-style directory (matching the
 * other bindings' runtimes/{rid}/native/ / native/{rid}/ convention) and native library
 * filename to load.
 */
final class NativePlatform
{
    /** @return array{0: string, 1: string} [$rid, $libraryFileName] */
    public static function ridAndLibraryName(): array
    {
        // php_uname('m') mirrors the OS's own reported string verbatim — Windows reports
        // "ARM64" (uppercase, matching PROCESSOR_ARCHITECTURE), unlike Linux/Darwin's
        // lowercase "aarch64"/"arm64" — so this must be case-insensitive. Confirmed by
        // hitting exactly this bug on a real windows-11-arm GHA runner: it silently fell
        // through to the win-x64 branch and tried to dlopen the wrong .dll.
        $machine = strtolower(php_uname('m'));
        $isArm = str_contains($machine, 'arm64') || str_contains($machine, 'aarch64');

        return match (PHP_OS_FAMILY) {
            'Windows' => $isArm ? ['win-arm64', 'hyperuuid.dll'] : ['win-x64', 'hyperuuid.dll'],
            'Darwin' => $isArm ? ['osx-arm64', 'libhyperuuid.dylib'] : ['osx-x64', 'libhyperuuid.dylib'],
            'Linux' => $isArm ? ['linux-arm64', 'libhyperuuid.so'] : ['linux-x64', 'libhyperuuid.so'],
            default => throw new \RuntimeException(
                'hyperuuid: unsupported platform PHP_OS_FAMILY=' . PHP_OS_FAMILY
            ),
        };
    }
}
