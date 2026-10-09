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
     * {@see UuidLayout::SqlServer} is defined only for the two versions that have a SQL Server
     * order, and answers 6, 7, or 0 for anything that isn't a SQL-ordered version 6 or 7 RFC
     * 9562 UUID (a value straight from {@see toSqlOrder()}). The version nibble lands at a
     * different byte for each, and the other version's random bits can mimic it there, so the
     * core checks the variant bits too, where each version puts them, and the answer never
     * confuses the two.
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
     * {@see UuidLayout::SqlServer} only versions 6 and 7 can match (see {@see version()}). A
     * `$version` outside 0-15 is simply never matched.
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
     * The Unix-epoch millisecond timestamp embedded in an RFC 9562 version 6 or 7 UUID held
     * in `$layout`'s byte order, or null for anything else: another version, or a 6 or 7
     * nibble under another variant (NCS, Microsoft, Future), which has no RFC version and so
     * no timestamp. One native call, which checks the variant and version and reads the
     * timestamp together. In {@see UuidLayout::SqlServer} this reads a value straight from
     * {@see toSqlOrder()} (or a `uniqueidentifier` column) from its permuted bytes, with no
     * conversion back first.
     *
     * @param UuidLayout $layout the byte order this UUID is held in
     * @return int|null the embedded Unix-epoch milliseconds, or null if this isn't an RFC
     *     9562 version 6 or 7 UUID in that layout
     */
    public function unixMillis(UuidLayout $layout = UuidLayout::Rfc9562): ?int
    {
        return Runtime::getTimestamp($this->bytes, $layout->value);
    }

    /**
     * The UTC timestamp embedded in an RFC 9562 version 6 or 7 UUID's timestamp field, held
     * in `$layout`'s byte order — {@see unixMillis()} as a DateTimeImmutable, with the same
     * variant check. A version 6
     * timestamp before 1970 reads back as the Unix epoch.
     *
     * Throws by default for anything else; pass `throwOnMismatch: false` to get `null`
     * back instead — for a caller that doesn't already know (or want to separately check)
     * whether this UUID is time-based.
     *
     * @param bool $throwOnMismatch whether to throw (the default) or return null when
     *     this isn't an RFC 9562 version 6 or 7 UUID in that layout
     * @param UuidLayout $layout the byte order this UUID is held in
     * @return \DateTimeImmutable|null the embedded UTC timestamp, or null if $throwOnMismatch
     *     is false and this isn't an RFC 9562 version 6 or 7 UUID in that layout
     * @throws \InvalidArgumentException If `$throwOnMismatch` is true and this isn't an
     *     RFC 9562 version 6 or 7 UUID in that layout.
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
                $variant = $version === 6 || $version === 7 ? ' with a non-RFC 9562 variant' : '';
                throw new \InvalidArgumentException(
                    'timestamp() is only defined for RFC 9562 version 6 or 7 UUIDs, got version '
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
     * Converts an RFC 9562-ordered version 6 or 7 UUID to the byte order SQL Server's
     * `uniqueidentifier` needs on the wire to sort by creation order. Dispatches on
     * `version()` the same way {@see timestamp()} does.
     *
     * `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a
     * `uniqueidentifier` column — doesn't compare a GUID's 16 bytes left to right; it uses a
     * fixed, non-sequential byte significance order (`10,11,12,13,14,15,8,9,6,7,4,5,0,1,2,3`,
     * most significant first). Both permutations are computed once in the native Rust core and
     * verified there — the v7 one independently against the real `System.Data.SqlTypes.SqlGuid`
     * comparator in this project's C# test suite too; this binding calls the same native
     * functions rather than reimplementing the math.
     *
     * For **v7**: this moves the timestamp and counter — the two fields that determine
     * creation order — into the comparison's most-significant bytes, and moves the trailing
     * entropy, which carries no ordering information, into the least-significant ones as one
     * intact block.
     *
     * For **v6**: v6 has no monotonic counter the way v7 does, so the only field determining
     * its creation order is the 60-bit timestamp itself — this moves that whole timestamp
     * (most significant chunk first) into the comparison's most significant bytes. Everything
     * after it — `variant`, `clock_seq`, and `node` (octets 8-15, already one contiguous run
     * with no ordering value of its own — `clock_seq`/`node` are independently random per call
     * here, not a counter, and `variant` is a fixed constant either way) — moves as that single
     * 8-byte span into the remaining bytes, in the same relative order, not individually
     * reshuffled. Version and variant end up at different byte offsets than v7's result (octet
     * 8's top nibble / octet 6's top two bits, not 7/8) — fine, since `fromSqlOrder()` already
     * knows how to tell the two apart. **Caveat unlike v7:** two v6 UUIDs minted at the same
     * millisecond have identical timestamp bits — `clock_seq`/`node` being random rather than
     * a counter means their
     * relative order isn't guaranteed to match creation order, the same limitation plain RFC
     * order already has for v6, not something this transform introduces.
     *
     * Meaningful only for a genuine version 6 or 7 UUID — same convention as {@see timestamp()}.
     *
     * @return self this UUID reordered into SQL Server wire order
     */
    public function toSqlOrder(): self
    {
        return new self(match ($this->version()) {
            6 => Runtime::v6ToSqlOrder($this->bytes),
            7 => Runtime::v7ToSqlOrder($this->bytes),
            default => throw new \InvalidArgumentException(
                "toSqlOrder() is only defined for version 6 or 7 UUIDs, got version {$this->version()}"
            ),
        });
    }

    /**
     * Inverse of {@see toSqlOrder()} — converts a SQL-Server-ordered UUID back to RFC 9562
     * order.
     *
     * Pass `$version` explicitly (6 or 7) when you already know it — the common case, since
     * you typically just called {@see toSqlOrder()} on a value whose version you knew. Left
     * null, the native core reads it in place, as {@see version()} with
     * {@see UuidLayout::SqlServer} does: a SQL-ordered blob's version nibble sits at octet 7
     * for a v7 value and octet 8 for a v6 one, and the core checks the variant bits where each
     * version puts them too, so the two are never confused.
     *
     * @param int|null $version 6 or 7, or null to read it from the bytes
     * @return self this UUID reordered into RFC 9562 order
     * @throws \InvalidArgumentException If `$version` isn't 6 or 7, or (when null) these
     *     bytes aren't a SQL-ordered version 6 or 7 UUID.
     */
    public function fromSqlOrder(?int $version = null): self
    {
        if ($version === null) {
            $version = Runtime::version($this->bytes, UuidLayout::SqlServer->value);
            if ($version === 0) {
                throw new \InvalidArgumentException(
                    'fromSqlOrder(): these bytes are not a SQL-ordered version 6 or 7 UUID; pass '
                    . '$version explicitly to convert them anyway'
                );
            }
        }
        return new self(match ($version) {
            6 => Runtime::v6ToRfcOrder($this->bytes),
            7 => Runtime::v7ToRfcOrder($this->bytes),
            default => throw new \InvalidArgumentException(
                "fromSqlOrder() only supports version 6 or 7, got {$version}"
            ),
        });
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
