<?php

declare(strict_types=1);

namespace HyperUuid;

use FFI;

/**
 * FFI plumbing for the native libhyperuuid shared library — dlopen/dlsym plus a raw C-ABI
 * call, no runtime bridge (the same "no shim" positioning as the Go/Swift bindings' purego/
 * dlopen approach). PHP's built-in `ext-ffi` needs no Composer package for this — the same
 * "no extra dependency" stance as Go's purego-only/no-cgo approach.
 */
final class Runtime
{
    private static ?FFI $ffi = null;

    public static function newV4(): string
    {
        $out = self::ffi()->new('uint8_t[16]');
        $rc = self::ffi()->uuid_new_v4($out);
        if ($rc !== 0) {
            throw new RandomSourceException("uuid_new_v4 failed with code {$rc}");
        }
        return FFI::string($out, 16);
    }

    public static function newV5(string $namespaceBytes, string $nameBytes): string
    {
        $ns = self::ffi()->new('uint8_t[16]');
        FFI::memcpy($ns, $namespaceBytes, 16);

        $nameLen = \strlen($nameBytes);
        $namePtr = null;
        if ($nameLen > 0) {
            $namePtr = self::ffi()->new("uint8_t[{$nameLen}]");
            FFI::memcpy($namePtr, $nameBytes, $nameLen);
        }

        $out = self::ffi()->new('uint8_t[16]');
        $rc = self::ffi()->uuid_new_v5($ns, $namePtr, $nameLen, $out);
        if ($rc !== 0) {
            throw new RandomSourceException("uuid_new_v5 failed with code {$rc}");
        }
        return FFI::string($out, 16);
    }

    public static function newV6(int $unixMillis): string
    {
        $out = self::ffi()->new('uint8_t[16]');
        $rc = self::ffi()->uuid_new_v6($unixMillis, $out);
        if ($rc === 2) {
            throw new TimestampOutOfRangeException(
                'unix_millis does not fit the 60-bit v6 timestamp field'
            );
        }
        if ($rc !== 0) {
            throw new RandomSourceException("uuid_new_v6 failed with code {$rc}");
        }
        return FFI::string($out, 16);
    }

    public static function v6UnixMillis(string $bytes): int
    {
        $ptr = self::ffi()->new('uint8_t[16]');
        FFI::memcpy($ptr, $bytes, 16);
        return self::ffi()->uuid_v6_unix_millis($ptr);
    }

    public static function newV7(int $unixMillis): string
    {
        $out = self::ffi()->new('uint8_t[16]');
        $rc = self::ffi()->uuid_new_v7($unixMillis, $out);
        if ($rc === 2) {
            throw new TimestampOutOfRangeException(
                'unix_millis must fit within the RFC 9562 48-bit field'
            );
        }
        if ($rc !== 0) {
            throw new RandomSourceException("uuid_new_v7 failed with code {$rc}");
        }
        return FFI::string($out, 16);
    }

    public static function v7UnixMillis(string $bytes): int
    {
        $ptr = self::ffi()->new('uint8_t[16]');
        FFI::memcpy($ptr, $bytes, 16);
        return self::ffi()->uuid_v7_unix_millis($ptr);
    }

    /**
     * Loaded lazily and exactly once, mirroring the Go binding's sync.Once / Swift's lazy
     * static let — the native library and its function pointers live for the process's
     * lifetime, same as every other binding (never unloaded).
     */
    private static function ffi(): FFI
    {
        if (self::$ffi !== null) {
            return self::$ffi;
        }

        [$rid, $libName] = NativePlatform::ridAndLibraryName();
        $path = __DIR__ . "/native/{$rid}/{$libName}";
        if (!is_file($path)) {
            throw new \RuntimeException(
                "hyperuuid: {$path} not found (unsupported platform, or this package was built "
                . 'without a native library for it)'
            );
        }

        self::$ffi = FFI::cdef(
            'int uuid_new_v4(void *out_ptr);'
            . 'int uuid_new_v5(const void *ns_ptr, const void *name_ptr, uint32_t name_len, void *out_ptr);'
            . 'int uuid_new_v6(uint64_t unix_millis, void *out_ptr);'
            . 'uint64_t uuid_v6_unix_millis(const void *uuid_ptr);'
            . 'int uuid_new_v7(uint64_t unix_millis, void *out_ptr);'
            . 'uint64_t uuid_v7_unix_millis(const void *uuid_ptr);',
            $path
        );

        return self::$ffi;
    }
}
