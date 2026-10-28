<?php

declare(strict_types=1);

namespace HyperUuid;

/**
 * Thrown by the version 7 generators, single and batch, when the stream cannot advance
 * without running more than 1000 ms ahead of the supplied timestamp or past its last
 * millisecond, 2^44 - 2 (native return code 6). It is a state error, not an argument error:
 * a call whose timestamp is past the stream's millisecond succeeds.
 */
final class CounterExhaustedException extends \RuntimeException
{
}
