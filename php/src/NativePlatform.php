<?php

declare(strict_types=1);

namespace HyperUuid;

/**
 * Maps the running process to the RID-style directory (matching the other bindings'
 * runtimes/{rid}/native/ / native/{rid}/ convention) and native library filename to load.
 *
 * @internal FFI plumbing, not part of the public API — PHP has no package-private visibility.
 */
final class NativePlatform
{
    /** Non-instantiable — static resolution only. */
    private function __construct()
    {
    }

    /** @return array{0: string, 1: string} [$rid, $libraryFileName] */
    public static function ridAndLibraryName(): array
    {
        return self::resolve(PHP_OS_FAMILY, php_uname('m'), PHP_INT_SIZE, PHP_OS_FAMILY === 'Linux' && self::isMusl());
    }

    /**
     * The mapping itself, as a pure function of what the process reports, so the whole table
     * is testable from any one platform.
     *
     * Windows is `win-x64` whatever the hardware: PHP has never shipped a native Windows
     * ARM64 build, so on ARM hardware it is an x64 process under emulation — while
     * php_uname('m') reports the *machine* ("ARM64"), which is not what a DLL has to match.
     * Everywhere else the machine string is matched exactly (case-insensitively), so an
     * architecture this package carries no library for is a clear error here rather than a
     * wrong-architecture dlopen failure later.
     *
     * @param string $osFamily PHP_OS_FAMILY
     * @param string $machine php_uname('m')
     * @param int $intSize PHP_INT_SIZE — 8 for the 64-bit process every bundled library needs
     * @param bool $musl whether the process runs on musl libc (Linux only)
     * @return array{0: string, 1: string} [$rid, $libraryFileName]
     */
    public static function resolve(string $osFamily, string $machine, int $intSize, bool $musl): array
    {
        if ($intSize !== 8) {
            throw new \RuntimeException('hyperuuid: unsupported platform — a 64-bit PHP is required');
        }
        if ($osFamily === 'Windows') {
            return ['win-x64', 'hyperuuid.dll'];
        }

        $arch = match (strtolower($machine)) {
            'x86_64', 'amd64' => 'x64',
            'aarch64', 'arm64' => 'arm64',
            default => throw new \RuntimeException(
                "hyperuuid: unsupported platform — no native library for architecture '{$machine}'"
            ),
        };

        return match ($osFamily) {
            'Darwin' => ["osx-{$arch}", 'libhyperuuid.dylib'],
            'Linux' => [($musl ? 'linux-musl-' : 'linux-') . $arch, 'libhyperuuid.so'],
            default => throw new \RuntimeException(
                "hyperuuid: unsupported platform PHP_OS_FAMILY={$osFamily}"
            ),
        };
    }

    /**
     * Whether this process runs on musl libc (Alpine) rather than glibc: true when the
     * process has a musl loader mapped. The same rule every binding in this repo uses; an
     * unreadable /proc/self/maps means glibc.
     */
    private static function isMusl(): bool
    {
        $maps = @file_get_contents('/proc/self/maps');
        return $maps !== false && (str_contains($maps, 'ld-musl-') || str_contains($maps, 'libc.musl-'));
    }
}
