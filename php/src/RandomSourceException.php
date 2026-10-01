<?php

declare(strict_types=1);

namespace HyperUuid;

/** Thrown when the native random source fails (any `uuid_new_*` generator's return code 1). */
final class RandomSourceException extends \RuntimeException
{
}
