<?php

declare(strict_types=1);

namespace HyperUuid;

/**
 * Thrown by HyperUuid::newV7() when the unix millisecond timestamp doesn't fit the RFC 9562
 * 48-bit unix_ts_ms field (uuid_new_v7 return code 2).
 */
final class TimestampOutOfRangeException extends \RuntimeException
{
}
