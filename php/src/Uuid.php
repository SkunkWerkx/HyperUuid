<?php

declare(strict_types=1);

namespace HyperUuid;

/**
 * A parsed 16-byte RFC 9562 UUID value. Minimal by design — this package has no runtime
 * dependency on ramsey/uuid, the same "no extra dependency" positioning as the Go binding's
 * purego-only/no-cgo approach and the Python binding's stdlib-ctypes-only one.
 */
final class Uuid
{
    private readonly string $bytes;

    public function __construct(string $bytes)
    {
        if (\strlen($bytes) !== 16) {
            throw new \InvalidArgumentException('bytes must be exactly 16 bytes');
        }
        $this->bytes = $bytes;
    }

    public static function parse(string $string): self
    {
        $hex = str_replace('-', '', $string);
        if (!preg_match('/\A[0-9a-fA-F]{32}\z/', $hex)) {
            throw new \InvalidArgumentException("invalid UUID string: {$string}");
        }
        return new self(hex2bin($hex));
    }

    public function bytes(): string
    {
        return $this->bytes;
    }

    public function version(): int
    {
        return (\ord($this->bytes[6]) >> 4) & 0x0F;
    }

    public function variant(): int
    {
        return (\ord($this->bytes[8]) >> 6) & 0b11;
    }

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

    public function equals(Uuid $other): bool
    {
        return $this->bytes === $other->bytes;
    }

    /**
     * The UTC timestamp embedded in a version 6 or 7 UUID's timestamp field. Only meaningful
     * when `version()` is 6 or 7 — the RFC 9562 bit layout doesn't distinguish "not a
     * time-based UUID" from "time-based UUID with a very early timestamp", so the caller is
     * responsible for checking `version()` first if that matters.
     */
    public function timestamp(): \DateTimeImmutable
    {
        $millis = match ($this->version()) {
            6 => Runtime::v6UnixMillis($this->bytes),
            7 => Runtime::v7UnixMillis($this->bytes),
            default => throw new \InvalidArgumentException(
                "timestamp() is only defined for version 6 or 7 UUIDs, got version {$this->version()}"
            ),
        };
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

    /** The RFC 9562 §5.9 Nil UUID — all 128 bits zero. */
    public static function nil(): self
    {
        static $v = null;
        return $v ??= new self(str_repeat("\x00", 16));
    }

    /** The RFC 9562 §5.10 Max UUID — all 128 bits one. */
    public static function max(): self
    {
        static $v = null;
        return $v ??= new self(str_repeat("\xFF", 16));
    }
}
