<?php

declare(strict_types=1);

namespace HyperUuid;

/** Well-known namespace UUIDs defined in RFC 9562 Section 6.6. */
final class Namespaces
{
    private function __construct()
    {
    }

    public static function dns(): Uuid
    {
        static $v = null;
        return $v ??= Uuid::parse('6ba7b810-9dad-11d1-80b4-00c04fd430c8');
    }

    public static function url(): Uuid
    {
        static $v = null;
        return $v ??= Uuid::parse('6ba7b811-9dad-11d1-80b4-00c04fd430c8');
    }

    public static function oid(): Uuid
    {
        static $v = null;
        return $v ??= Uuid::parse('6ba7b812-9dad-11d1-80b4-00c04fd430c8');
    }

    public static function x500(): Uuid
    {
        static $v = null;
        return $v ??= Uuid::parse('6ba7b814-9dad-11d1-80b4-00c04fd430c8');
    }
}
