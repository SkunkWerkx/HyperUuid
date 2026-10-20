<?php

declare(strict_types=1);

namespace HyperUuid;

/**
 * A parsed 16-byte RFC 9562 UUID value. Minimal by design — this package has no runtime
 * dependency on ramsey/uuid, the same "no extra dependency" positioning as the Go binding's
 * lone google/uuid requirement and the Python binding's dependency-free PyO3 wheels.
 * Casts to, and JSON-encodes as, the hyphenated hex string. Holds its 16 bytes exactly as
 * given: RFC 9562 order, or SQL Server order for a value from {@see toSqlOrder()}, which the
 * methods taking a {@see UuidLayout} read in place.
 */
final class Uuid implements \JsonSerializable, \Stringable
{
    private readonly string $bytes;

    private static ?bool $fastInstants = null;

    /**
     * Wraps a raw 16-byte RFC 9562 (big-endian) UUID value.
     *
     * @param string $bytes the raw 16-byte value
     * @throws \InvalidArgumentException If `$bytes` isn't exactly 16 bytes.
     */
    public function __construct(string $bytes)
    {
        if (\strlen($bytes) !== 16) {
            throw new \InvalidArgumentException('bytes must be exactly 16 bytes');
        }
        $this->bytes = $bytes;
    }

    /**
     * Parses an 8-4-4-4-12 hyphenated hex UUID string, in either letter case. Exactly that
     * shape and nothing else — no braces, no `urn:uuid:` prefix, no unhyphenated or
     * otherwise-hyphenated hex — the same rule the Rust core's own `Uuid::from_str` applies.
     *
     * @param string $string the UUID string to parse
     * @return self the parsed UUID
     * @throws \InvalidArgumentException If `$string` isn't a valid UUID string.
     */
    public static function parse(string $string): self
    {
        if (!preg_match('/\A[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\z/i', $string)) {
            throw new \InvalidArgumentException("invalid UUID string: {$string}");
        }
        return new self(hex2bin(str_replace('-', '', $string)));
    }

    /**
     * The UUID's 16 raw bytes in RFC 9562 (big-endian) order.
     *
     * @return string the raw 16-byte value
     */
    public function bytes(): string
    {
        return $this->bytes;
    }

    /**
     * The version of this UUID, read in `$layout`'s byte order by the native core.
     *
     * In RFC 9562 order (the default) this is the version nibble, 0 through 15: 0 for
     * {@see nil()}, 15 for {@see max()}. It says nothing about the variant; use
     * {@see isRfc()} when the question is "an RFC 9562 UUID of version N".
     *
     * With the default layout this reads the value as RFC 9562 order. For a value from
     * {@see toSqlOrder()}, or read back from a `uniqueidentifier` column, pass
     * {@see UuidLayout::SqlServer}: the RFC-order read uses the wrong bytes there.
     *
     * In {@see UuidLayout::SqlServer} this answers 7, the version the value was converted
     * from, or 0 for anything that isn't a SQL-ordered value (one straight from
     * {@see toSqlOrder()}). Every SQL-ordered value is an RFC 9562 version 8 with its marker
     * at octet 7 and the variant at octet 8; the core checks both.
     *
     * @param UuidLayout $layout the byte order this UUID is held in
     * @return int the version
     */
    public function version(UuidLayout $layout = UuidLayout::Rfc9562): int
    {
        return Runtime::version($this->bytes, $layout->value);
    }

    /**
     * The variant field of this RFC 9562-ordered UUID (RFC 9562 §4.1):
     * {@see UuidVariant::Ncs} for {@see nil()}, {@see UuidVariant::Future} for {@see max()},
     * and {@see UuidVariant::Rfc9562} for anything this library mints.
     *
     * RFC 9562 order only, and there is no layout parameter. A {@see toSqlOrder()} result or
     * a `uniqueidentifier` read-back carries the RFC variant at octet 8 too, so this answers
     * {@see UuidVariant::Rfc9562} for it, but that says nothing about whether it is a
     * SQL-ordered version 7; to validate one of those, use {@see isRfc()} with
     * {@see UuidLayout::SqlServer}.
     *
     * @return UuidVariant the variant
     */
    public function variant(): UuidVariant
    {
        return UuidVariant::from(Runtime::variant($this->bytes));
    }

    /**
     * Whether this is an RFC 9562 UUID of version `$version`, held in `$layout`'s byte order:
     * the RFC variant and that version, in one native call. The guard to run before trusting
     * a value's version-specific fields, such as a version 7's timestamp. In
     * {@see UuidLayout::SqlServer} only version 7 can match (see {@see version()}). A
     * `$version` outside 0-15 is simply never matched.
     *
     * With the default layout this reads the value as RFC 9562 order. A SQL-ordered value —
     * from {@see toSqlOrder()}, or read back from a `uniqueidentifier` column — needs
     * {@see UuidLayout::SqlServer}: the RFC-order read uses the wrong bytes there and answers
     * for whatever they happen to hold. There is deliberately no layout-agnostic check: the
     * caller holding the value knows its order, and only the layout it names says which bytes
     * to read.
     *
     * @param int $version the version to test for
     * @param UuidLayout $layout the byte order this UUID is held in
     * @return bool true if this is an RFC 9562 UUID of that version
     */
    public function isRfc(int $version, UuidLayout $layout = UuidLayout::Rfc9562): bool
    {
        return Runtime::isRfc($this->bytes, $version, $layout->value);
    }

    /**
     * The 8-4-4-4-12 hyphenated hex string representation.
     *
     * @return string the hyphenated hex string
     */
    public function __toString(): string
    {
        $hex = bin2hex($this->bytes);
        return sprintf(
            '%s-%s-%s-%s-%s',
            substr($hex, 0, 8),
            substr($hex, 8, 4),
            substr($hex, 12, 4),
            substr($hex, 16, 4),
            substr($hex, 20, 12)
        );
    }

    /**
     * The JSON representation — the same hyphenated hex string as {@see __toString()}, so
     * `json_encode(['id' => $uuid])` carries the UUID rather than an empty object.
     *
     * @return string the hyphenated hex string
     */
    public function jsonSerialize(): string
    {
        return $this->__toString();
    }

    /**
     * Whether `$other` wraps the same 16 raw bytes.
     *
     * @param Uuid $other the UUID to compare against
     * @return bool true if both wrap the same 16 raw bytes
     */
    public function equals(Uuid $other): bool
    {
        return $this->bytes === $other->bytes;
    }

    /**
     * The Unix-epoch millisecond timestamp embedded in an RFC 9562 version 7 UUID held in
     * `$layout`'s byte order, or null for anything else: another version (a version 6 or 1
     * from another generator included), or a 7 nibble under another variant (NCS, Microsoft,
     * Future), which has no RFC version and so no timestamp. One native call, which checks
     * the variant and version and reads the timestamp together. In
     * {@see UuidLayout::SqlServer} this reads a value straight from
     * {@see toSqlOrder()} (or a `uniqueidentifier` column) from its permuted bytes, with no
     * conversion back first.
     *
     * @param UuidLayout $layout the byte order this UUID is held in
     * @return int|null the embedded Unix-epoch milliseconds, or null if this isn't an RFC
     *     9562 version 7 UUID in that layout
     */
    public function unixMillis(UuidLayout $layout = UuidLayout::Rfc9562): ?int
    {
        return Runtime::getTimestamp($this->bytes, $layout->value);
    }

    /**
     * The UTC timestamp embedded in an RFC 9562 version 7 UUID's timestamp field, held in
     * `$layout`'s byte order — {@see unixMillis()} as a DateTimeImmutable, with the same
     * variant check.
     *
     * Throws by default for anything else; pass `throwOnMismatch: false` to get `null`
     * back instead — for a caller that doesn't already know (or want to separately check)
     * whether this UUID is time-based.
     *
     * @param bool $throwOnMismatch whether to throw (the default) or return null when
     *     this isn't an RFC 9562 version 7 UUID in that layout
     * @param UuidLayout $layout the byte order this UUID is held in
     * @return \DateTimeImmutable|null the embedded UTC timestamp, or null if $throwOnMismatch
     *     is false and this isn't an RFC 9562 version 7 UUID in that layout
     * @throws \InvalidArgumentException If `$throwOnMismatch` is true and this isn't an
     *     RFC 9562 version 7 UUID in that layout.
     */
    public function timestamp(
        bool $throwOnMismatch = true,
        UuidLayout $layout = UuidLayout::Rfc9562
    ): ?\DateTimeImmutable {
        $millis = $this->unixMillis($layout);
        if ($millis === null) {
            if ($throwOnMismatch) {
                $version = $this->version($layout);
                $where = $layout === UuidLayout::Rfc9562 ? '' : " in {$layout->name} order";
                $variant = $version === 7 ? ' with a non-RFC 9562 variant' : '';
                throw new \InvalidArgumentException(
                    'timestamp() is only defined for RFC 9562 version 7 UUIDs, got version '
                    . "{$version}{$variant}{$where}"
                );
            }
            return null;
        }
        // PHP 8.4+: build from exact integers instead of a date-string format parse (the
        // HyperCast instant lesson); older PHP keeps the createFromFormat path.
        self::$fastInstants ??= method_exists(\DateTimeImmutable::class, 'createFromTimestamp')
            && method_exists(\DateTimeImmutable::class, 'setMicrosecond');
        if (self::$fastInstants) {
            $instant = \DateTimeImmutable::createFromTimestamp(intdiv($millis, 1000));
            $micros = ($millis % 1000) * 1000;
            return $micros === 0 ? $instant : $instant->setMicrosecond($micros);
        }
        $dt = \DateTimeImmutable::createFromFormat(
            'U.v',
            sprintf('%d.%03d', intdiv($millis, 1000), $millis % 1000),
            new \DateTimeZone('UTC')
        );
        if ($dt === false) {
            throw new \RuntimeException('hyperuuid: failed to construct timestamp from UUID');
        }
        return $dt;
    }

    /**
     * Converts an RFC 9562-ordered version 7 UUID to SQL Server order, the byte order a
     * `uniqueidentifier` sorts by creation order.
     *
     * `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a
     * `uniqueidentifier` column — doesn't compare a GUID's 16 bytes left to right; it uses a
     * fixed significance order (`10,11,12,13,14,15,8,9,6,7,4,5,0,1,2,3`, most significant
     * first). The result is an RFC 9562 version 8 UUID (§5.8, the version for custom
     * layouts): the version nibble 8 at octet 7's top nibble and the RFC variant at octet 8's
     * top bits — where SQL Server's text form and a .NET `Guid` show them, so both read
     * version 8 — and around them the version 7's 122 free bits filling those compare slots
     * most significant first: the 48-bit millisecond timestamp, the 26-bit counter, then the
     * 48 random bits, so values sort by timestamp and then counter. That is SequentialGuid's
     * permutation with an 8 where it keeps the 7. The layout is computed in the native Rust
     * core and checked there (and against the real `SqlGuid` comparator in this project's C#
     * suite); this binding calls it rather than reimplementing the math.
     *
     * A version 7 and a version 8 have the same 122 free bits, so every version 7 converts
     * and the conversion is the exact inverse of {@see fromSqlOrder()}.
     *
     * @return self this UUID in SQL Server order
     * @throws \InvalidArgumentException If this isn't an RFC 9562 version 7 UUID.
     */
    public function toSqlOrder(): self
    {
        return new self(Runtime::v7ToSqlOrder($this->bytes));
    }

    /**
     * Inverse of {@see toSqlOrder()} — converts a SQL-ordered UUID back to the RFC 9562
     * version 7 it came from.
     *
     * The version 8 marker and the variant are checked, as {@see version()} with
     * {@see UuidLayout::SqlServer} does; anything that isn't a SQL-ordered value is refused,
     * an RFC-ordered value included.
     *
     * @return self this UUID in RFC 9562 order
     * @throws \InvalidArgumentException If these bytes aren't a SQL-ordered value converted
     *     from version 7.
     */
    public function fromSqlOrder(): self
    {
        return new self(Runtime::v7ToRfcOrder($this->bytes));
    }

    /**
     * The RFC 9562 §5.9 Nil UUID — all 128 bits zero.
     *
     * @return self the Nil UUID
     */
    public static function nil(): self
    {
        static $v = null;
        return $v ??= new self(str_repeat("\x00", 16));
    }

    /**
     * The RFC 9562 §5.10 Max UUID — all 128 bits one.
     *
     * @return self the Max UUID
     */
    public static function max(): self
    {
        static $v = null;
        return $v ??= new self(str_repeat("\xFF", 16));
    }
}
