<?php

declare(strict_types=1);

namespace HyperUuid;

use FFI;

/**
 * FFI plumbing for the native libhyperuuid shared library — dlopen/dlsym plus a raw C-ABI
 * call, no runtime bridge (the same "no shim" positioning as the Swift binding's dlopen
 * approach). PHP's built-in `ext-ffi` needs no Composer package for this — the same
 * "no extra dependency" stance as the Go binding's lone google/uuid requirement.
 *
 * Performance shape, measured not assumed (the HyperCast lesson, applied here): PHP's raw
 * ext-ffi call floor is ~105 ns — already extension-class — so every avoidable nanosecond
 * in this file was wrapper, and the wrapper is written accordingly. Input byte strings are
 * declared `const char *` in the cdef so PHP strings cross zero-copy (no per-call CData
 * allocation, no memcpy in); the 16-byte out buffer is one static scratch allocated once
 * (PHP's request model makes static scratch safe — arrays auto-decay to pointers, so no
 * pre-taken address is needed); only the in-place byte-order rewrites still copy, because
 * the native call genuinely mutates the buffer.
 *
 * @internal FFI plumbing, not part of the public API — PHP has no package-private visibility.
 *     Callers go through {@see HyperUuid} and {@see Uuid}, which guarantee the 16-byte
 *     buffers these methods hand to native code unchecked.
 */
final class Runtime
{
    /** The widest count or length the C ABI's `uint32_t` parameters can carry. */
    private const UINT32_MAX = 0xFFFFFFFF;

    /** Batch return code 3's meaning: `count * 16` overflows the platform's address space. */
    private const UNADDRESSABLE_BATCH = 'the batch is too large to address on this platform';

    private static ?FFI $ffi = null;
    private static ?FFI\CData $out16 = null;
    private static ?FFI\CData $millis = null;

    /** Non-instantiable — static calls only. */
    private function __construct()
    {
    }

    /**
     * The loaded library's own version, packed `major << 16 | minor << 8 | patch` by the
     * core's zero-argument probe, rendered as "major.minor.patch".
     */
    public static function nativeVersion(): string
    {
        $ffi = self::$ffi ?? self::load();
        $packed = $ffi->hyperuuid_version();
        return sprintf('%d.%d.%d', $packed >> 16, ($packed >> 8) & 0xFF, $packed & 0xFF);
    }

    public static function newV4(): string
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->uuid_new_v4(self::$out16);
        if ($rc !== 0) {
            throw new RandomSourceException("uuid_new_v4 failed with code {$rc}");
        }
        return FFI::string(self::$out16, 16);
    }

    public static function newV5(string $namespaceBytes, string $nameBytes): string
    {
        if (\strlen($nameBytes) > self::UINT32_MAX) {
            // name_len is a uint32_t; ext-ffi would silently truncate a longer length.
            throw new \InvalidArgumentException('name must be shorter than 4 GiB');
        }
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->uuid_new_v5(
            $namespaceBytes,
            $nameBytes === '' ? null : $nameBytes,
            \strlen($nameBytes),
            self::$out16
        );
        if ($rc !== 0) {
            throw new RandomSourceException("uuid_new_v5 failed with code {$rc}");
        }
        return FFI::string(self::$out16, 16);
    }

    public static function newV6(int $unixMillis): string
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->uuid_new_v6($unixMillis, self::$out16);
        if ($rc === 2) {
            throw new TimestampOutOfRangeException(
                'unix_millis does not fit the 60-bit v6 timestamp field'
            );
        }
        if ($rc !== 0) {
            throw new RandomSourceException("uuid_new_v6 failed with code {$rc}");
        }
        return FFI::string(self::$out16, 16);
    }

    public static function newV6Batch(int $count, int $unixMillis): string
    {
        self::checkCount($count);
        if ($count === 0) {
            return '';
        }
        $ffi = self::$ffi ?? self::load();
        $out = $ffi->new('uint8_t[' . ($count * 16) . ']');
        $rc = $ffi->uuid_new_v6_batch($unixMillis, $count, $out);
        if ($rc === 2) {
            throw new TimestampOutOfRangeException(
                'unix_millis does not fit the 60-bit v6 timestamp field'
            );
        }
        if ($rc === 3) {
            throw new \InvalidArgumentException(self::UNADDRESSABLE_BATCH);
        }
        if ($rc !== 0) {
            throw new RandomSourceException("uuid_new_v6_batch failed with code {$rc}");
        }
        return FFI::string($out, $count * 16);
    }

    public static function newV7(int $unixMillis): string
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->uuid_new_v7($unixMillis, self::$out16);
        if ($rc === 2) {
            throw new TimestampOutOfRangeException(
                'unix_millis must fit within the RFC 9562 48-bit field'
            );
        }
        if ($rc !== 0) {
            throw new RandomSourceException("uuid_new_v7 failed with code {$rc}");
        }
        return FFI::string(self::$out16, 16);
    }

    /**
     * The Unix-epoch milliseconds of `$bytes`, held in the order of the core layout code
     * `$layout`, when it is an RFC 9562 version 6 or 7 UUID there (variant checked); null
     * for anything else. One native call.
     */
    public static function getTimestamp(string $bytes, int $layout): ?int
    {
        $ffi = self::$ffi ?? self::load();
        return $ffi->uuid_get_timestamp($bytes, $layout, FFI::addr(self::$millis)) === 0
            ? null
            : self::$millis->cdata;
    }

    /**
     * The version of `$bytes` held in the order of the core layout code `$layout`: the RFC
     * nibble (0-15) in RFC 9562 order; 6, 7, or 0 for "not a SQL-ordered v6/v7" in SQL Server
     * order.
     */
    public static function version(string $bytes, int $layout): int
    {
        $ffi = self::$ffi ?? self::load();
        return $ffi->uuid_version($bytes, $layout);
    }

    /** The core's variant code for RFC-ordered `$bytes`: 1 Ncs, 2 Rfc9562, 3 Microsoft, 4 Future. */
    public static function variant(string $bytes): int
    {
        $ffi = self::$ffi ?? self::load();
        return $ffi->uuid_variant($bytes);
    }

    /** Whether `$bytes`, held in layout `$layout`, is an RFC 9562 UUID of version `$version`. */
    public static function isRfc(string $bytes, int $version, int $layout): bool
    {
        if ($version < 0 || $version > self::UINT32_MAX) {
            // version is a uint32_t; ext-ffi would wrap a negative or over-wide one into
            // range, possibly onto a real version. The core answers false past 15 itself.
            return false;
        }
        $ffi = self::$ffi ?? self::load();
        return $ffi->uuid_is_rfc($bytes, $version, $layout) !== 0;
    }

    public static function newV7Batch(int $count, int $unixMillis): string
    {
        if ($count < 0 || $count > HyperUuid::MAX_V7_BATCH) {
            // Checked before the buffer is allocated: the core would refuse it (code 4)
            // without writing anything, but only after a pointless allocation here.
            throw new \InvalidArgumentException(self::v7BatchLimitMessage($count));
        }
        if ($count === 0) {
            return '';
        }
        $ffi = self::$ffi ?? self::load();
        $out = $ffi->new('uint8_t[' . ($count * 16) . ']');
        $rc = $ffi->uuid_new_v7_batch($unixMillis, $count, $out);
        if ($rc === 2) {
            throw new TimestampOutOfRangeException(
                'unix_millis must fit within the RFC 9562 48-bit field'
            );
        }
        if ($rc === 3) {
            throw new \InvalidArgumentException(self::UNADDRESSABLE_BATCH);
        }
        if ($rc === 4) {
            throw new \InvalidArgumentException(self::v7BatchLimitMessage($count));
        }
        if ($rc !== 0) {
            throw new RandomSourceException("uuid_new_v7_batch failed with code {$rc}");
        }
        return FFI::string($out, $count * 16);
    }

    /**
     * Rewrites `$bytes` from RFC 9562 order to the byte order SQL Server's `uniqueidentifier`
     * needs on the wire to sort a version 7 UUID by creation order. Meaningful only for a
     * genuine version 7 UUID.
     */
    public static function v7ToSqlOrder(string $bytes): string
    {
        $ffi = self::$ffi ?? self::load();
        FFI::memcpy(self::$out16, $bytes, 16);
        $ffi->uuid_v7_to_sql_order(self::$out16);
        return FFI::string(self::$out16, 16);
    }

    /** Inverse of {@see v7ToSqlOrder} — rewrites `$bytes` from SQL Server order back to RFC 9562 order. */
    public static function v7ToRfcOrder(string $bytes): string
    {
        $ffi = self::$ffi ?? self::load();
        FFI::memcpy(self::$out16, $bytes, 16);
        $ffi->uuid_v7_to_rfc_order(self::$out16);
        return FFI::string(self::$out16, 16);
    }

    /**
     * Rewrites `$bytes` from RFC 9562 order to the byte order SQL Server's `uniqueidentifier`
     * needs on the wire to sort a version 6 UUID by creation order. Meaningful only for a
     * genuine version 6 UUID.
     */
    public static function v6ToSqlOrder(string $bytes): string
    {
        $ffi = self::$ffi ?? self::load();
        FFI::memcpy(self::$out16, $bytes, 16);
        $ffi->uuid_v6_to_sql_order(self::$out16);
        return FFI::string(self::$out16, 16);
    }

    /** Inverse of {@see v6ToSqlOrder} — rewrites `$bytes` from SQL Server order back to RFC 9562 order. */
    public static function v6ToRfcOrder(string $bytes): string
    {
        $ffi = self::$ffi ?? self::load();
        FFI::memcpy(self::$out16, $bytes, 16);
        $ffi->uuid_v6_to_rfc_order(self::$out16);
        return FFI::string(self::$out16, 16);
    }

    /**
     * A batch count crosses the ABI as a `uint32_t`: a negative or over-wide count is a
     * caller bug, reported as one here instead of as an FFI allocation error or a silently
     * truncated batch.
     */
    private static function checkCount(int $count): void
    {
        if ($count < 0 || $count > self::UINT32_MAX) {
            throw new \InvalidArgumentException(
                "count must be between 0 and 4294967295, got {$count}"
            );
        }
    }

    private static function v7BatchLimitMessage(int $count): string
    {
        return 'count must be between 0 and ' . HyperUuid::MAX_V7_BATCH
            . " (a single version 7 batch takes at most the 26-bit counter space), got {$count}";
    }

    /**
     * Loaded lazily, once per request: PHP's statics reset between requests, so under a web
     * SAPI the declarations are bound again on each request's first call (the OS keeps the
     * library itself mapped for the worker's lifetime). The CLI's single long request is the
     * one case that matches the other bindings' load-once-per-process (Java's class
     * initializer, Swift's lazy static let).
     */
    private static function load(): FFI
    {
        [$rid, $libName] = NativePlatform::ridAndLibraryName();
        $path = __DIR__ . "/native/{$rid}/{$libName}";
        // Development loop: HYPERUUID_NATIVE_LIBRARY names a library to load instead of the
        // staged one, so the suite runs against a core built from the checkout without
        // replacing committed files (.github/scripts/local-core.sh builds one and prints it).
        $override = getenv('HYPERUUID_NATIVE_LIBRARY');
        if (\is_string($override) && $override !== '') {
            $path = $override;
        } elseif (!is_file($path)) {
            // Development loop: fall back to the in-repo cargo build, exactly what the
            // other bindings' local staging does.
            $repoBuild = \dirname(__DIR__, 2) . "/rust/target/release/{$libName}";
            if (is_file($repoBuild)) {
                $path = $repoBuild;
            }
        }
        if (!is_file($path)) {
            throw new \RuntimeException(
                "hyperuuid: {$path} not found (unsupported platform, or this package was built "
                . 'without a native library for it)'
            );
        }

        self::$ffi = FFI::cdef(
            'uint32_t hyperuuid_version(void);'
            . 'int uuid_new_v4(void *out_ptr);'
            . 'int uuid_new_v5(const char *ns_ptr, const char *name_ptr, uint32_t name_len, void *out_ptr);'
            . 'int uuid_new_v6(uint64_t unix_millis, void *out_ptr);'
            . 'uint64_t uuid_v6_unix_millis(const char *uuid_ptr);'
            . 'int uuid_new_v6_batch(uint64_t unix_millis, uint32_t count, void *out_ptr);'
            . 'int uuid_new_v7(uint64_t unix_millis, void *out_ptr);'
            . 'uint64_t uuid_v7_unix_millis(const char *uuid_ptr);'
            . 'int uuid_new_v7_batch(uint64_t unix_millis, uint32_t count, void *out_ptr);'
            . 'void uuid_v7_to_sql_order(void *uuid_ptr);'
            . 'void uuid_v7_to_rfc_order(void *uuid_ptr);'
            . 'void uuid_v6_to_sql_order(void *uuid_ptr);'
            . 'void uuid_v6_to_rfc_order(void *uuid_ptr);'
            . 'uint32_t uuid_version(const char *uuid_ptr, uint32_t layout);'
            . 'uint32_t uuid_variant(const char *uuid_ptr);'
            . 'uint32_t uuid_is_rfc(const char *uuid_ptr, uint32_t version, uint32_t layout);'
            . 'uint64_t uuid_v6_unix_millis_in(const char *uuid_ptr, uint32_t layout);'
            . 'uint64_t uuid_v7_unix_millis_in(const char *uuid_ptr, uint32_t layout);'
            . 'uint32_t uuid_get_timestamp(const char *uuid_ptr, uint32_t layout, uint64_t *millis_out);',
            $path
        );
        self::$out16 = self::$ffi->new('uint8_t[16]');
        self::$millis = self::$ffi->new('uint64_t');
        return self::$ffi;
    }
}
