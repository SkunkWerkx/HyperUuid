<?php

declare(strict_types=1);

namespace HyperUuid;

/**
 * RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, calling
 * directly into the native libhyperuuid shared library via PHP's built-in FFI extension —
 * no runtime bridge, no extra Composer dependency. Bundles a native build for every
 * supported platform (see NativePlatform) and picks the right one at runtime, the same
 * trick the Go/Java bindings use since Composer has no per-platform native selection.
 */
final class HyperUuid
{
    private function __construct()
    {
    }

    /** Creates a random UUID version 4 (RFC 9562 §5.4). */
    public static function newV4(): Uuid
    {
        return new Uuid(Runtime::newV4());
    }

    /**
     * Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a UTF-8
     * name. The same (namespace, name) pair always produces the same UUID.
     */
    public static function newV5(Uuid $namespace, string $name): Uuid
    {
        return new Uuid(Runtime::newV5($namespace->bytes(), $name));
    }

    /**
     * Creates a time-sortable UUID version 6 (RFC 9562 §5.6), a field-compatible reordering
     * of version 1 for better sort/index locality. Defaults to the current time; pass an
     * explicit Unix-epoch millisecond timestamp to embed a specific time instead. `clock_seq`
     * and `node` are randomly generated on every call — unlike version 7, there is no
     * monotonic counter, so calls within the same millisecond are not guaranteed to sort in
     * creation order.
     */
    public static function newV6(?int $unixMillis = null): Uuid
    {
        $unixMillis ??= (int) round(microtime(true) * 1000);
        return new Uuid(Runtime::newV6($unixMillis));
    }

    /**
     * Creates a time-sortable UUID version 7 (RFC 9562 §6.2). Defaults to the current time;
     * pass an explicit Unix-epoch millisecond timestamp (non-negative, fitting in 48 bits)
     * to embed a specific time instead.
     */
    public static function newV7(?int $unixMillis = null): Uuid
    {
        $unixMillis ??= (int) round(microtime(true) * 1000);
        return new Uuid(Runtime::newV7($unixMillis));
    }
}
