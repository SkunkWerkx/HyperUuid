<?php

declare(strict_types=1);

namespace HyperUuid;

/**
 * Thrown by the version 6 and 7 generators, single and batch, when the unix millisecond
 * timestamp doesn't fit the version's timestamp field — the 60-bit Gregorian field for
 * version 6, RFC 9562's 48-bit unix_ts_ms for version 7 (native return code 2).
 */
final class TimestampOutOfRangeException extends \RuntimeException
{
}
