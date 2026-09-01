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
    /** Non-instantiable — static factory methods only. */
    private function __construct()
    {
    }

    /**
     * Creates a random UUID version 4 (RFC 9562 §5.4).
     *
     * @return Uuid a new random version 4 UUID
     */
    public static function newV4(): Uuid
    {
        return new Uuid(Runtime::newV4());
    }

    /**
     * Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a UTF-8
     * name. The same (namespace, name) pair always produces the same UUID.
     *
     * @param Uuid $namespace the namespace UUID, e.g. one of {@see Namespaces}
     * @param string $name the UTF-8-encoded name
     * @return Uuid the deterministic version 5 UUID for this (namespace, name) pair
     */
    public static function newV5(Uuid $namespace, string $name): Uuid
    {
        return new Uuid(Runtime::newV5($namespace->bytes(), $name));
    }

    /**
     * Converts $value to a Unix-epoch millisecond integer: null becomes the current time, a
     * DateTimeInterface is converted exactly (whole seconds plus microseconds, no float
     * rounding), and an int (a raw millisecond count) passes through unchanged. Shared by
     * every newV6/newV7/batch door below so a caller can pass either a DateTimeInterface or a
     * raw millisecond count interchangeably.
     *
     * @param \DateTimeInterface|int|null $value the timestamp to convert, or null for the
     *     current time
     * @return int the equivalent Unix-epoch millisecond count
     */
    private static function unixMillisFrom(\DateTimeInterface|int|null $value): int
    {
        if ($value === null) {
            return (int) round(microtime(true) * 1000);
        }
        if ($value instanceof \DateTimeInterface) {
            return $value->getTimestamp() * 1000 + intdiv((int) $value->format('u'), 1000);
        }
        return $value;
    }

    /**
     * Creates a time-sortable UUID version 6 (RFC 9562 §5.6), a field-compatible reordering
     * of version 1 for better sort/index locality. Defaults to the current time; pass an
     * explicit DateTimeInterface or Unix-epoch millisecond timestamp to embed a specific time
     * instead. `clock_seq` and `node` are randomly generated on every call — unlike version 7,
     * there is no monotonic counter, so calls within the same millisecond are not guaranteed
     * to sort in creation order.
     *
     * @param \DateTimeInterface|int|null $unixMillis the timestamp to embed, or null for the
     *     current time
     * @return Uuid a new version 6 UUID
     */
    public static function newV6(\DateTimeInterface|int|null $unixMillis = null): Uuid
    {
        return new Uuid(Runtime::newV6(self::unixMillisFrom($unixMillis)));
    }

    /**
     * Creates `count` time-sortable version 6 UUIDs sharing one timestamp capture — one FFI
     * call and one random-bytes fetch instead of `count` of each. Defaults to the current time.
     *
     * @param int $count how many UUIDs to create
     * @param \DateTimeInterface|int|null $unixMillis the shared timestamp to embed in each, or
     *     null for the current time
     * @return list<Uuid> `count` new version 6 UUIDs
     */
    public static function newV6Batch(int $count, \DateTimeInterface|int|null $unixMillis = null): array
    {
        $bytes = Runtime::newV6Batch($count, self::unixMillisFrom($unixMillis));
        $ids = [];
        for ($i = 0; $i < $count; $i++) {
            $ids[] = new Uuid(substr($bytes, $i * 16, 16));
        }
        return $ids;
    }

    /**
     * Creates a time-sortable UUID version 7 (RFC 9562 §6.2). Defaults to the current time;
     * pass an explicit DateTimeInterface or Unix-epoch millisecond timestamp (non-negative,
     * fitting in 48 bits) to embed a specific time instead.
     *
     * @param \DateTimeInterface|int|null $unixMillis the timestamp to embed, or null for the
     *     current time
     * @return Uuid a new version 7 UUID
     */
    public static function newV7(\DateTimeInterface|int|null $unixMillis = null): Uuid
    {
        return new Uuid(Runtime::newV7(self::unixMillisFrom($unixMillis)));
    }

    /**
     * Creates `count` time-sortable version 7 UUIDs sharing one timestamp capture and one
     * contiguous block of the monotonic counter — one FFI call and one random-bytes fetch
     * instead of `count` of each. Defaults to the current time.
     *
     * @param int $count how many UUIDs to create
     * @param \DateTimeInterface|int|null $unixMillis the shared timestamp to embed in each, or
     *     null for the current time
     * @return list<Uuid> `count` new version 7 UUIDs
     */
    public static function newV7Batch(int $count, \DateTimeInterface|int|null $unixMillis = null): array
    {
        $bytes = Runtime::newV7Batch($count, self::unixMillisFrom($unixMillis));
        $ids = [];
        for ($i = 0; $i < $count; $i++) {
            $ids[] = new Uuid(substr($bytes, $i * 16, 16));
        }
        return $ids;
    }

    /**
     * Returns `$count` version 7 UUIDs as one binary string of raw RFC 9562-ordered bytes,
     * 16 per UUID, instead of an array of Uuid objects.
     *
     * Substantially faster than {@see newV7Batch} for large batches. The native call is
     * identical; the difference is that newV7Batch then allocates `$count` Uuid objects and
     * `$count` substrings on top of it. This hands back the bytes the native core already
     * produced, untouched.
     *
     * Use it when bytes are the destination — a BINARY(16) bind parameter, a wire format, a
     * bulk insert. If you need Uuid objects, keep using {@see newV7Batch}: slicing this
     * string yourself only moves the same allocations into your own code.
     *
     * Slice it with `substr($bytes, $i * 16, 16)`, which is what newV7Batch does internally.
     */
    public static function newV7BatchBytes(int $count, \DateTimeInterface|int|null $unixMillis = null): string
    {
        return Runtime::newV7Batch($count, self::unixMillisFrom($unixMillis));
    }

    /**
     * Returns `$count` version 6 UUIDs as one binary string of raw RFC 9562-ordered bytes,
     * 16 per UUID. The version 6 counterpart to {@see newV7BatchBytes}, with the same
     * rationale and the same guidance about when it is the right call.
     *
     * `clock_seq` and `node` are independently random per item; unlike version 7 there is no
     * monotonic counter, so items minted in the same millisecond are not guaranteed to sort
     * in creation order.
     */
    public static function newV6BatchBytes(int $count, \DateTimeInterface|int|null $unixMillis = null): string
    {
        return Runtime::newV6Batch($count, self::unixMillisFrom($unixMillis));
    }
}
