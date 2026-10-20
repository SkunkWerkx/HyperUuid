<?php

declare(strict_types=1);

namespace HyperUuid;

/**
 * Thrown by the version 7 generators, single and batch, when the unix millisecond timestamp
 * doesn't fit RFC 9562's 48-bit unix_ts_ms field (native return code 2).
 */
final class TimestampOutOfRangeException extends \RuntimeException
{
}
