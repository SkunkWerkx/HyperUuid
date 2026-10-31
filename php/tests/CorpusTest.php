<?php

declare(strict_types=1);

namespace HyperUuid\Tests;

use HyperUuid\HyperUuid;
use HyperUuid\Namespaces;
use HyperUuid\Uuid;
use HyperUuid\UuidLayout;
use HyperUuid\UuidVariant;
use PHPUnit\Framework\TestCase;

/**
 * Replays the shared conformance corpus (`corpus/*.json` at the repository root), the same
 * files the Rust core's own suite replays, through this binding's public API. A Uuid holds
 * its bytes exactly as given, so every hex value becomes a Uuid of those bytes in its layout's
 * order, the way {@see Uuid::toSqlOrder()} returns a SQL-ordered one. A vector that fails here
 * is a break in the cross-language contract, and for `sql_order.json` a change to data
 * already persisted in SQL Server.
 */
final class CorpusTest extends TestCase
{
    private static function corpusDirectory(): string
    {
        $dir = __DIR__;
        while (!is_dir("{$dir}/corpus")) {
            if (\dirname($dir) === $dir) {
                self::fail('corpus directory not found above ' . __DIR__);
            }
            $dir = \dirname($dir);
        }
        return "{$dir}/corpus";
    }

    /** @return list<array<string, mixed>> */
    private static function corpus(string $name): array
    {
        $vectors = json_decode(
            (string) file_get_contents(self::corpusDirectory() . "/{$name}"),
            true,
            flags: JSON_THROW_ON_ERROR
        );
        self::assertIsArray($vectors);
        self::assertNotEmpty($vectors, $name);
        return $vectors;
    }

    private static function uuid(string $hex): Uuid
    {
        return new Uuid((string) hex2bin($hex));
    }

    private static function layout(string $name): UuidLayout
    {
        return match ($name) {
            'rfc9562' => UuidLayout::Rfc9562,
            'sql_server' => UuidLayout::SqlServer,
        };
    }

    private static function millisOf(\DateTimeImmutable $instant): int
    {
        return $instant->getTimestamp() * 1000 + (int) $instant->format('v');
    }

    public function testV5Corpus(): void
    {
        foreach (self::corpus('v5.json') as $vector) {
            $context = json_encode($vector);
            $ns = match ($vector['namespace']) {
                'dns' => Namespaces::dns(),
                'url' => Namespaces::url(),
                'oid' => Namespaces::oid(),
                'x500' => Namespaces::x500(),
            };
            // A PHP string is bytes, so the raw-name door and the text door are one method.
            $expected = $vector['expect'];
            $name = (string) hex2bin($vector['name_hex']);
            self::assertSame($expected, bin2hex(HyperUuid::newV5($ns, $name)->bytes()), $context);
            if (isset($vector['name'])) {
                self::assertSame($expected, bin2hex(HyperUuid::newV5($ns, $vector['name'])->bytes()), $context);
            }
        }
    }

    public function testSqlOrderCorpus(): void
    {
        foreach (self::corpus('sql_order.json') as $vector) {
            $context = json_encode($vector);
            $rfc = self::uuid($vector['rfc']);
            $sql = self::uuid($vector['sql']);
            self::assertSame($vector['sql'], bin2hex($rfc->toSqlOrder()->bytes()), $context);
            self::assertSame($vector['rfc'], bin2hex($sql->fromSqlOrder()->bytes()), $context);
            self::assertSame(7, $sql->version(UuidLayout::SqlServer), $context);
            self::assertTrue($sql->isRfc(7, UuidLayout::SqlServer), $context);
        }
    }

    /**
     * Every refused vector fails with InvalidArgumentException and the core's wrong-version
     * message, through the conversion its direction names; the source Uuid is left as it was.
     */
    public function testSqlOrderRefusedCorpus(): void
    {
        foreach (self::corpus('sql_order_refused.json') as $vector) {
            $context = json_encode($vector);
            $id = self::uuid($vector['uuid']);
            try {
                match ($vector['direction']) {
                    'to_sql' => $id->toSqlOrder(),
                    'from_sql' => $id->fromSqlOrder(),
                };
                self::fail("accepted: {$context}");
            } catch (\InvalidArgumentException $e) {
                self::assertSame(\InvalidArgumentException::class, $e::class, $context);
                self::assertSame(
                    'the value is not a UUID of the version this conversion takes',
                    $e->getMessage(),
                    $context
                );
            }
            self::assertSame($vector['uuid'], bin2hex($id->bytes()), $context);
        }
    }

    public function testTimestampCorpus(): void
    {
        foreach (self::corpus('timestamp.json') as $vector) {
            $context = json_encode($vector);
            $id = self::uuid($vector['uuid']);
            $layout = self::layout($vector['layout']);
            $millis = $vector['unix_millis'];
            // Both doors check the version, and answer only for a timestamp the core would mint:
            // a version 7 dated past the generation cap, 253402300799999 ms, has none, as a
            // version 6 has none (the corpus row carries the 48-bit field's value).
            $expected = $millis !== null && $millis <= 253_402_300_799_999 ? $millis : null;

            self::assertSame($vector['version'], $id->version($layout), $context);
            self::assertSame($expected, $id->unixMillis($layout), $context);
            $instant = $id->timestamp(false, $layout);
            self::assertSame($expected, $instant === null ? null : self::millisOf($instant), $context);
            if ($expected !== null) {
                self::assertSame($expected, self::millisOf($id->timestamp(layout: $layout)), $context);
            } else {
                try {
                    $id->timestamp(layout: $layout);
                    self::fail("timestamp() answered for a value with none: $context");
                } catch (\InvalidArgumentException) {
                }
            }

            if ($layout === UuidLayout::Rfc9562) {
                self::assertSame($vector['version'], $id->version(), $context);
                self::assertSame($expected, $id->unixMillis(), $context);
                $instant = $id->timestamp(false);
                self::assertSame($expected, $instant === null ? null : self::millisOf($instant), $context);
            }
        }
    }

    public function testInspectCorpus(): void
    {
        foreach (self::corpus('inspect.json') as $vector) {
            $context = json_encode($vector);
            $id = self::uuid($vector['uuid']);
            $layout = self::layout($vector['layout']);
            $version = $vector['version'];

            self::assertSame($version, $id->version($layout), $context);
            self::assertSame($vector['is_rfc'], $id->isRfc($version, $layout), $context);
            for ($other = 0; $other <= 15; $other++) {
                if ($other !== $version) {
                    self::assertFalse($id->isRfc($other, $layout), "{$context} version {$other}");
                }
            }

            if (isset($vector['variant'])) {
                $variant = match ($vector['variant']) {
                    'ncs' => UuidVariant::Ncs,
                    'rfc9562' => UuidVariant::Rfc9562,
                    'microsoft' => UuidVariant::Microsoft,
                    'future' => UuidVariant::Future,
                };
                self::assertSame($variant, $id->variant(), $context);
                self::assertSame($version, $id->version(), $context);
                self::assertSame($vector['is_rfc'], $id->isRfc($version), $context);
            }
        }
    }
}
