<?php

/**
 * Stubs for the hyperuuid extension, for IDEs and static analysis. Generated: don't edit.
 * Regenerate from php/ with `php tools/generate-stubs.php`, which runs
 * `cargo php stubs --stdout` (cargo-php 0.2.1) on the debug build of the extension and
 * completes its output (the tool says how).
 */

namespace HyperUuid {
    /**
     * Static entry points of the binding.
     */
    final class HyperUuid {
        /**
         * The most UUIDs one version 7 batch (`newV7Batch()`, `newV7BatchBytes()`) mints:
         * 67,108,864, which is 2^20 steps' worth: a batch of n consumes ⌈n/64⌉ steps, so the
         * largest batch takes exactly one millisecond of the stream.
         */
        public const MAX_V7_BATCH = 67108864;

        /**
         * Not instantiable, as the 1.0 class's private constructor made it.
         */
        private function __construct() {}

        /**
         * The loaded library's own version, "major.minor.patch".
         *
         * @return string
         */
        public static function nativeVersion(): string {}

        /**
         * Creates a random UUID version 4 (RFC 9562 §5.4).
         *
         * @return \HyperUuid\Uuid
         */
        public static function newV4(): \HyperUuid\Uuid {}

        /**
         * Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a name,
         * taken as the string's raw bytes. The same (namespace, name) pair always produces the
         * same UUID.
         *
         * @throws \InvalidArgumentException If `$name` is 4 GiB or longer.
         *
         * @param \HyperUuid\Uuid $namespace
         * @param string $name
         * @return \HyperUuid\Uuid
         */
        public static function newV5(\HyperUuid\Uuid $namespace, string $name): \HyperUuid\Uuid {}

        /**
         * Creates a time-sortable UUID version 7 (RFC 9562 §6.2). Defaults to the current time;
         * pass an explicit DateTimeInterface or Unix-epoch millisecond timestamp (non-negative,
         * fitting in 48 bits) to embed a specific time instead. The ordering contract is in the
         * README, "v7 ordering, precisely".
         *
         * @throws TimestampOutOfRangeException If the timestamp doesn't fit 48 bits.
         * @throws CounterExhaustedException If the stream can't advance.
         *
         * @param \DateTimeInterface|int|null $unixMillis
         * @return \HyperUuid\Uuid
         */
        public static function newV7(\DateTimeInterface|int|null $unixMillis = null): \HyperUuid\Uuid {}

        /**
         * Creates `count` time-sortable version 7 UUIDs sharing one timestamp capture and one
         * contiguous block of the v7 stream, in strictly increasing order. Defaults to the
         * current time.
         *
         * @throws \InvalidArgumentException If `$count` is negative or above `MAX_V7_BATCH`.
         * @throws TimestampOutOfRangeException If the timestamp doesn't fit 48 bits.
         * @throws CounterExhaustedException If the stream can't advance.
         *
         * @param int $count
         * @param \DateTimeInterface|int|null $unixMillis
         * @return array
         */
        public static function newV7Batch(int $count, \DateTimeInterface|int|null $unixMillis = null): array {}

        /**
         * Returns `$count` version 7 UUIDs as one binary string of raw RFC 9562-ordered bytes,
         * 16 per UUID, instead of an array of Uuid objects. Same ordering guarantee and limit as
         * `newV7Batch()`.
         *
         * @throws \InvalidArgumentException If `$count` is negative or above `MAX_V7_BATCH`.
         * @throws TimestampOutOfRangeException If the timestamp doesn't fit 48 bits.
         * @throws CounterExhaustedException If the stream can't advance.
         *
         * @param int $count
         * @param \DateTimeInterface|int|null $unixMillis
         * @return string
         */
        public static function newV7BatchBytes(int $count, \DateTimeInterface|int|null $unixMillis = null): string {}
    }

    /**
     * Well-known namespace UUIDs defined in RFC 9562 Section 6.6. Each accessor returns one
     * instance per request, as the 1.0 class's function-static cache did.
     */
    final class Namespaces {
        /**
         * Not instantiable, as the 1.0 class's private constructor made it.
         */
        private function __construct() {}

        /**
         * The DNS namespace UUID.
         *
         * @return \HyperUuid\Uuid
         */
        public static function dns(): \HyperUuid\Uuid {}

        /**
         * The ISO OID namespace UUID.
         *
         * @return \HyperUuid\Uuid
         */
        public static function oid(): \HyperUuid\Uuid {}

        /**
         * The URL namespace UUID.
         *
         * @return \HyperUuid\Uuid
         */
        public static function url(): \HyperUuid\Uuid {}

        /**
         * The X.500 DN namespace UUID.
         *
         * @return \HyperUuid\Uuid
         */
        public static function x500(): \HyperUuid\Uuid {}
    }

    /**
     * A parsed 16-byte RFC 9562 UUID value, held exactly as given: RFC 9562 order, or SQL
     * Server order for a value from `toSqlOrder()`, which the methods taking a `UuidLayout`
     * read in place. Casts to, and JSON-encodes as, the hyphenated hex string.
     */
    final class Uuid implements \JsonSerializable {
        private string $bytes;

        /**
         * Wraps a raw 16-byte RFC 9562 (big-endian) UUID value.
         *
         * @throws \InvalidArgumentException If `$bytes` isn't exactly 16 bytes.
         *
         * @param string $bytes
         */
        public function __construct(string $bytes) {}

        /**
         * The serialized form: the 16 bytes under the property's mangled name, the array PHP
         * serialized for the 1.0 class.
         *
         * @return array
         */
        public function __serialize(): array {}

        /**
         * The 8-4-4-4-12 hyphenated hex string representation.
         *
         * @return string
         */
        public function __toString(): string {}

        /**
         * @param array $data
         * @return void
         */
        public function __unserialize(array $data): void {}

        /**
         * The UUID's 16 raw bytes, as held.
         *
         * @return string
         */
        public function bytes(): string {}

        /**
         * Whether `$other` wraps the same 16 raw bytes.
         *
         * @param \HyperUuid\Uuid $other
         * @return bool
         */
        public function equals(\HyperUuid\Uuid $other): bool {}

        /**
         * Inverse of `toSqlOrder()`: converts a SQL-ordered UUID back to the RFC 9562 version
         * 7 it came from.
         *
         * @throws \InvalidArgumentException If these bytes aren't a SQL-ordered value converted
         *     from version 7.
         *
         * @return \HyperUuid\Uuid
         */
        public function fromSqlOrder(): \HyperUuid\Uuid {}

        /**
         * Whether this is an RFC 9562 UUID of version `$version`, held in `$layout`'s byte
         * order. A `$version` outside 0-15 is simply never matched.
         *
         * @param int $version
         * @param \HyperUuid\UuidLayout $layout
         * @return bool
         */
        public function isRfc(int $version, \HyperUuid\UuidLayout $layout = \HyperUuid\UuidLayout::Rfc9562): bool {}

        /**
         * The JSON representation: the same hyphenated hex string as `__toString()`.
         *
         * @return string
         */
        public function jsonSerialize(): string {}

        /**
         * The RFC 9562 §5.10 Max UUID: all 128 bits one.
         *
         * @return \HyperUuid\Uuid
         */
        public static function max(): \HyperUuid\Uuid {}

        /**
         * The RFC 9562 §5.9 Nil UUID: all 128 bits zero.
         *
         * @return \HyperUuid\Uuid
         */
        public static function nil(): \HyperUuid\Uuid {}

        /**
         * Parses an 8-4-4-4-12 hyphenated hex UUID string, in either letter case. Exactly that
         * shape and nothing else: no braces, no `urn:uuid:` prefix, no unhyphenated or otherwise
         * hyphenated hex.
         *
         * @throws \InvalidArgumentException If `$string` isn't a valid UUID string.
         *
         * @param string $string
         * @return \HyperUuid\Uuid
         */
        public static function parse(string $string): \HyperUuid\Uuid {}

        /**
         * The UTC timestamp embedded in an RFC 9562 version 7 UUID held in `$layout`'s byte
         * order. Throws for anything else, or returns null when `$throwOnMismatch` is false.
         *
         * @throws \InvalidArgumentException If `$throwOnMismatch` is true and this isn't an RFC
         *     9562 version 7 UUID in that layout.
         *
         * @param bool $throwOnMismatch
         * @param \HyperUuid\UuidLayout $layout
         * @return \DateTimeImmutable|null
         */
        public function timestamp(bool $throwOnMismatch = true, \HyperUuid\UuidLayout $layout = \HyperUuid\UuidLayout::Rfc9562): ?\DateTimeImmutable {}

        /**
         * Converts an RFC 9562-ordered version 7 UUID to SQL Server order, the byte order a
         * `uniqueidentifier` sorts by creation order.
         *
         * @throws \InvalidArgumentException If this isn't an RFC 9562 version 7 UUID.
         *
         * @return \HyperUuid\Uuid
         */
        public function toSqlOrder(): \HyperUuid\Uuid {}

        /**
         * The Unix-epoch millisecond timestamp embedded in an RFC 9562 version 7 UUID held in
         * `$layout`'s byte order, or null for anything else.
         *
         * @param \HyperUuid\UuidLayout $layout
         * @return int|null
         */
        public function unixMillis(\HyperUuid\UuidLayout $layout = \HyperUuid\UuidLayout::Rfc9562): ?int {}

        /**
         * The variant field of this RFC 9562-ordered UUID (RFC 9562 §4.1).
         *
         * @return \HyperUuid\UuidVariant
         */
        public function variant(): \HyperUuid\UuidVariant {}

        /**
         * The version of this UUID, read in `$layout`'s byte order: the version nibble, 0
         * through 15, in RFC 9562 order; in SQL Server order 7 for a value converted from
         * version 7, or 0 for anything else.
         *
         * @param \HyperUuid\UuidLayout $layout
         * @return int
         */
        public function version(\HyperUuid\UuidLayout $layout = \HyperUuid\UuidLayout::Rfc9562): int {}
    }

    /**
     * Byte order a UUID is held in.
     */
    enum UuidLayout: int {
      case Rfc9562 = 1;
      case SqlServer = 2;
    }

    /**
     * The RFC 9562 variant field.
     */
    enum UuidVariant: int {
      case Ncs = 1;
      case Rfc9562 = 2;
      case Microsoft = 3;
      case Future = 4;
    }
}

namespace HyperUuid {
    /**
     * The version 7 stream cannot advance without running more than 1000 ms ahead of the
     * supplied timestamp or past its last millisecond.
     */
    final class CounterExhaustedException extends \RuntimeException {}

    /**
     * The operating system's random source failed.
     */
    final class RandomSourceException extends \RuntimeException {}

    /**
     * A timestamp doesn't fit the RFC 9562 48-bit field.
     */
    final class TimestampOutOfRangeException extends \RuntimeException {}
}
