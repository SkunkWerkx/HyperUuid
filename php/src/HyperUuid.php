<?php

declare(strict_types=1);

namespace HyperUuid;

/**
 * RFC 9562 UUID v4 (random), v5 (deterministic) and v7 (time-sortable) generation, calling
 * directly into the native libhyperuuid shared library via PHP's built-in FFI extension —
 * no runtime bridge, no extra Composer dependency. Bundles a native build for every
 * supported platform (see NativePlatform) and picks the right one at runtime, the same
 * trick the Go/Java bindings use since Composer has no per-platform native selection.
 * {@see isAvailable()} is the probe a consumer with a fallback gates on, and
 * {@see nativeVersion()} names the library that actually loaded.
 */
final class HyperUuid
{
    /**
     * The most UUIDs one version 7 batch ({@see newV7Batch()}, {@see newV7BatchBytes()})
     * mints: 67,108,864, which is 2^20 steps' worth: a batch of n consumes ⌈n/64⌉ steps, so
     * the largest batch takes exactly one millisecond of the stream.
     *
     * Every batch up to this size is in strictly increasing order. A larger one is refused
     * with an InvalidArgumentException before anything is allocated. The ordering contract,
     * including what happens when the stream runs out, is on {@see newV7()}.
     */
    public const MAX_V7_BATCH = 1 << 26;

    private static ?bool $available = null;

    /** Non-instantiable — static factory methods only. */
    private function __construct()
    {
    }

    /**
     * Whether libhyperuuid resolved for this platform and exports the ABI this binding was
     * built against — the probe a consumer with a fallback gates on. Attempts the same load
     * every generator makes, but never throws: a missing ext-ffi, an `ffi.enable` setting
     * that restricts FFI for this SAPI, a missing or unloadable library, an unsupported
     * platform, or a stale library that lacks a symbol this binding declares all answer
     * false. The answer is cached for the request; true exactly when
     * {@see nativeVersion()} succeeds.
     *
     * @return bool true when every generator can be called; false when the first one would throw
     */
    public static function isAvailable(): bool
    {
        if (self::$available !== null) {
            return self::$available;
        }
        try {
            Runtime::nativeVersion();
            return self::$available = true;
        } catch (\Throwable) {
            return self::$available = false;
        }
    }

    /**
     * The version of the native library that actually loaded, as "major.minor.patch" —
     * the core's own manifest version, read through the zero-argument probe it exports, so
     * a host can prove the library it resolved is the one this binding was written against
     * before minting the first UUID. Throws when no library resolves; {@see isAvailable()}
     * is the non-throwing form.
     *
     * @return string the native library's semantic version as "major.minor.patch"
     */
    public static function nativeVersion(): string
    {
        return Runtime::nativeVersion();
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
     * newV7 and both batch doors below so a caller can pass either a DateTimeInterface or a
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
     * Creates a time-sortable UUID version 7 (RFC 9562 §6.2). Defaults to the current time;
     * pass an explicit DateTimeInterface or Unix-epoch millisecond timestamp (non-negative,
     * fitting in 48 bits) to embed a specific time instead.
     *
     * Every v7 call, single or batch, reserves from one process-wide stream that is re-seeded
     * at random whenever a timestamp moves it forward. A single call consumes one of a
     * millisecond's 2^20 steps and a batch of n consumes ceil(n/64). For timestamps up to
     * 2^44 - 2 (the year 2527):
     *
     * - Ordering: a call whose timestamp is within 10 s of the newest stream timestamp either
     *   throws {@see CounterExhaustedException} or reserves from the stream, and its UUIDs
     *   then sort strictly after every UUID from a reservation that completed before it began.
     *   Overlapping reservations are ordered by reservation, which need not be completion order.
     * - Embedded timestamp: at most 11 s ahead of the one supplied. A timestamp up to 11 s
     *   behind the stream is clamped onto the stream's millisecond, and a millisecond whose
     *   steps run out splits into the next (at most 1000 ms ahead of the supplied timestamp).
     * - Backfill and future-dated calls: a timestamp more than 11 s behind the stream, or past
     *   2^44 - 2, is minted at itself, outside the stream: it succeeds, leaves the stream alone
     *   and is ordered against nothing. One call far in the future advances the stream, and
     *   until the clock catches up, real-time calls are minted out of stream, unordered among
     *   themselves.
     * - Exhaustion: a split that would take the stream more than 1000 ms ahead of the supplied
     *   timestamp, or past 2^44 - 2, throws {@see CounterExhaustedException} before anything is
     *   reserved.
     *
     * @param \DateTimeInterface|int|null $unixMillis the timestamp to embed, or null for the
     *     current time
     * @return Uuid a new version 7 UUID
     * @throws TimestampOutOfRangeException If the timestamp doesn't fit 48 bits.
     * @throws CounterExhaustedException If the stream can't advance (see above).
     */
    public static function newV7(\DateTimeInterface|int|null $unixMillis = null): Uuid
    {
        return new Uuid(Runtime::newV7(self::unixMillisFrom($unixMillis)));
    }

    /**
     * Creates `count` time-sortable version 7 UUIDs sharing one timestamp capture and one
     * contiguous block of the v7 stream — one FFI call and one random-bytes fetch
     * instead of `count` of each. Defaults to the current time.
     *
     * The batch is in strictly increasing order, under the same stream contract as
     * {@see newV7()}: a batch of n consumes ceil(n/64) of a millisecond's steps, and may
     * carry timestamps up to 1000 ms past the one supplied.
     *
     * @param int $count how many UUIDs to create, 0 to {@see MAX_V7_BATCH}
     * @param \DateTimeInterface|int|null $unixMillis the shared timestamp to embed in each, or
     *     null for the current time
     * @return list<Uuid> `count` new version 7 UUIDs
     * @throws \InvalidArgumentException If `$count` is negative or above {@see MAX_V7_BATCH}.
     * @throws CounterExhaustedException If the stream can't advance (see {@see newV7()}).
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
     * Same ordering guarantee and limit as {@see newV7Batch}.
     *
     * @param int $count how many UUIDs to create, 0 to {@see MAX_V7_BATCH}
     * @param \DateTimeInterface|int|null $unixMillis the shared timestamp to embed in each, or
     *     null for the current time
     * @return string `$count * 16` bytes: `$count` version 7 UUIDs in RFC 9562 order
     * @throws \InvalidArgumentException If `$count` is negative or above {@see MAX_V7_BATCH}.
     * @throws CounterExhaustedException If the stream can't advance (see {@see newV7()}).
     */
    public static function newV7BatchBytes(int $count, \DateTimeInterface|int|null $unixMillis = null): string
    {
        return Runtime::newV7Batch($count, self::unixMillisFrom($unixMillis));
    }
}
