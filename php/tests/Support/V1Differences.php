<?php

declare(strict_types=1);

namespace HyperUuid\Tests\Support;

use HyperUuid\TimestampOutOfRangeException;

/**
 * The fixtures under tests/fixtures/v1 are 1.0's own recordings and are never edited. Where
 * this extension answers differently by design, the difference is listed in php/README.md
 * under "Differences from 1.0" and applied here, so a recording compares against what the
 * extension is meant to say and any other difference still fails.
 */
final class V1Differences
{
    /** 1.0 accepted every value of the 48-bit field; the core now stops at 9999-12-31T23:59:59.999Z. */
    public const MAX_UNIX_MILLIS = 253_402_300_799_999;

    public const V1_RANGE_MESSAGE = 'unix_millis must fit within the RFC 9562 48-bit field';

    public const RANGE_MESSAGE = 'unix_millis must be between 0 and 253402300799999 (9999-12-31T23:59:59.999Z)';

    /**
     * A recorded outcome as this extension gives it: the range message is reworded, and a
     * millisecond count 1.0 accepted past the cap is now the out-of-range exception. Takes
     * both recordings' shapes (`millis`/`exception_class` from datetimes.json,
     * `result`/`exception` from coercion.json).
     *
     * @param array<string, mixed> $outcome
     * @return array<string, mixed>
     */
    public static function expected(array $outcome): array
    {
        if (($outcome['message'] ?? null) === self::V1_RANGE_MESSAGE) {
            $outcome['message'] = self::RANGE_MESSAGE;
        }
        foreach (['millis' => 'exception_class', 'result' => 'exception'] as $value => $class) {
            if (\is_int($outcome[$value] ?? null) && $outcome[$value] > self::MAX_UNIX_MILLIS) {
                unset($outcome[$value]);
                $outcome[$class] = TimestampOutOfRangeException::class;
                $outcome['message'] = self::RANGE_MESSAGE;
            }
        }
        return $outcome;
    }
}
