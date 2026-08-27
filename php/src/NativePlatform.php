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
        $isArm = str_contains(php_uname('m'), 'arm64') || str_contains(php_uname('m'), 'aarch64');

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
