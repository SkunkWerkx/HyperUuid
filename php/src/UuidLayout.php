<?php

declare(strict_types=1);

namespace HyperUuid;

/**
 * The byte order a {@see Uuid} is held in, for the inspection and timestamp methods that take
 * one. The case values are the native core's layout codes. There is no "unspecified" case: a
 * PHP enum parameter can't be left unset, so every value a method receives is a real layout.
 */
enum UuidLayout: int
{
    /** RFC 9562 order, what every other method in this library takes and returns. */
    case Rfc9562 = 1;

    /**
     * The order {@see Uuid::toSqlOrder()} returns, which SQL Server's `uniqueidentifier` sorts
     * by creation order. Defined for versions 6 and 7 only.
     */
    case SqlServer = 2;
}
