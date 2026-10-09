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
            self::assertSame($vector['rfc'], bin2hex($sql->fromSqlOrder($vector['version'])->bytes()), $context);
            // The polymorphic inverse reads the version from the SQL-ordered bytes themselves.
            self::assertSame($vector['rfc'], bin2hex($sql->fromSqlOrder()->bytes()), $context);
            self::assertSame($vector['version'], $sql->version(UuidLayout::SqlServer), $context);
        }
    }

    public function testTimestampCorpus(): void
    {
        foreach (self::corpus('timestamp.json') as $vector) {
            $context = json_encode($vector);
            $id = self::uuid($vector['uuid']);
            $layout = self::layout($vector['layout']);
            $millis = $vector['unix_millis'];

            self::assertSame($vector['version'], $id->version($layout), $context);
            self::assertSame($millis, $id->unixMillis($layout), $context);
            $instant = $id->timestamp(false, $layout);
            self::assertSame($millis, $instant === null ? null : self::millisOf($instant), $context);
            if ($millis !== null) {
                self::assertSame($millis, self::millisOf($id->timestamp(layout: $layout)), $context);
            }

            if ($layout === UuidLayout::Rfc9562) {
                self::assertSame($vector['version'], $id->version(), $context);
                self::assertSame($millis, $id->unixMillis(), $context);
                $instant = $id->timestamp(false);
                self::assertSame($millis, $instant === null ? null : self::millisOf($instant), $context);
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
