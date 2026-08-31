<?php

declare(strict_types=1);

namespace HyperUuid;

/**
 * A parsed 16-byte RFC 9562 UUID value. Minimal by design — this package has no runtime
 * dependency on ramsey/uuid, the same "no extra dependency" positioning as the Go binding's
 * purego-only/no-cgo approach and the Python binding's dependency-free PyO3 wheels.
 */
final class Uuid
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
     * Parses an 8-4-4-4-12 hyphenated hex UUID string.
     *
     * @param string $string the UUID string to parse
     * @return self the parsed UUID
     * @throws \InvalidArgumentException If `$string` isn't a valid UUID string.
     */
    public static function parse(string $string): self
    {
        $hex = str_replace('-', '', $string);
        if (!preg_match('/\A[0-9a-fA-F]{32}\z/', $hex)) {
            throw new \InvalidArgumentException("invalid UUID string: {$string}");
        }
        return new self(hex2bin($hex));
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
     * The RFC 9562 version nibble (bits 48-51, the high nibble of octet 6).
     *
     * @return int the version nibble
     */
    public function version(): int
    {
        return (\ord($this->bytes[6]) >> 4) & 0x0F;
    }

    /**
     * The RFC 9562 variant bits (top two bits of octet 8). `0b10` means RFC 9562/4122.
     *
     * @return int the variant bits
     */
    public function variant(): int
    {
        return (\ord($this->bytes[8]) >> 6) & 0b11;
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
     * The UTC timestamp embedded in a version 6 or 7 UUID's timestamp field. Only meaningful
     * when `version()` is 6 or 7 — the RFC 9562 bit layout doesn't distinguish "not a
     * time-based UUID" from "time-based UUID with a very early timestamp", so the caller is
     * responsible for checking `version()` first if that matters.
     *
     * Throws by default for any other version; pass `throwOnMismatch: false` to get `null`
     * back instead — for a caller that doesn't already know (or want to separately check)
     * whether this UUID is time-based.
     *
     * @param bool $throwOnMismatch whether to throw (the default) or return null when
     *     `version()` isn't 6 or 7
     * @return \DateTimeImmutable|null the embedded UTC timestamp, or null if $throwOnMismatch
     *     is false and this isn't a version 6 or 7 UUID
     */
    public function timestamp(bool $throwOnMismatch = true): ?\DateTimeImmutable
    {
        $millis = match ($this->version()) {
            6 => Runtime::v6UnixMillis($this->bytes),
            7 => Runtime::v7UnixMillis($this->bytes),
            default => $throwOnMismatch ? throw new \InvalidArgumentException(
                "timestamp() is only defined for version 6 or 7 UUIDs, got version {$this->version()}"
            ) : null,
        };
        if ($millis === null) {
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
     * Unlike `version()`/`timestamp()`, a SQL-ordered blob's version nibble doesn't sit at a
     * fixed byte offset — it's octet 7 for a v7 value, octet 8 for a v6 one — so which inverse
     * to apply can't always be read off the bytes with certainty the way it can for an
     * RFC-ordered UUID (`GuidByteOrder`'s own documented "not detectable from the bits alone
     * by design" caveat, ported here). Pass `$version` explicitly (6 or 7) when you already
     * know it — the common case, since you typically just called {@see toSqlOrder()} on a
     * value whose version you knew. Left null, this tries the v7 inverse first and accepts it
     * if the result's own RFC-order version/variant bits actually read back as 7/RFC-4122,
     * then falls back to the v6 inverse under the same check — correct for any value this
     * binding's own `toSqlOrder()` produced, but not a cryptographic guarantee against
     * adversarial input, so prefer the explicit form where correctness matters most.
     *
     * @param int|null $version 6 or 7, or null to auto-detect
     * @return self this UUID reordered into RFC 9562 order
     * @throws \InvalidArgumentException If `$version` isn't 6 or 7, or (when null)
     *     neither inverse's result decodes to a valid version 6 or 7 UUID.
     */
    public function fromSqlOrder(?int $version = null): self
    {
        if ($version !== null) {
            return new self(match ($version) {
                6 => Runtime::v6ToRfcOrder($this->bytes),
                7 => Runtime::v7ToRfcOrder($this->bytes),
                default => throw new \InvalidArgumentException(
                    "fromSqlOrder() only supports version 6 or 7, got {$version}"
                ),
            });
        }

        $asV7 = new self(Runtime::v7ToRfcOrder($this->bytes));
        if ($asV7->version() === 7 && $asV7->variant() === 0b10) {
            return $asV7;
        }
        $asV6 = new self(Runtime::v6ToRfcOrder($this->bytes));
        if ($asV6->version() === 6 && $asV6->variant() === 0b10) {
            return $asV6;
        }
        throw new \InvalidArgumentException(
            'fromSqlOrder(): could not determine whether these bytes are version 6 or 7 SQL '
            . 'order; pass $version explicitly'
        );
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
