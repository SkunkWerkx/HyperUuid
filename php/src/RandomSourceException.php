<?php

declare(strict_types=1);

namespace HyperUuid;

/** Thrown when the native random source fails (uuid_new_v4/v5/v7 return code 1). */
final class RandomSourceException extends \RuntimeException
{
}
