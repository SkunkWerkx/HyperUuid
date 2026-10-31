<?php

declare(strict_types=1);

namespace HyperUuid\Tests;

use HyperUuid\Tests\Support\ChildProcess;
use HyperUuid\Uuid;
use PHPUnit\Framework\TestCase;

/**
 * `Uuid` behaves as the 1.0 PHP class did under PHP's generic object machinery: loose `==`
 * and `<=>` over its private readonly `bytes` property, clone, var_dump, and refusal of every
 * write from outside. The comparison rows were recorded from the 1.0 class.
 */
final class ObjectBehaviourTest extends TestCase
{
    use ChildProcess;

    private const SAMPLE = "\x01\x23\x45\x67\x89\xab\xcd\xef\xfe\xdc\xba\x98\x76\x54\x32\x10";

    public function testComparisonsMatchV1(): void
    {
        $rows = json_decode(
            (string) file_get_contents(__DIR__ . '/fixtures/v1/comparisons.json'),
            true,
            flags: JSON_THROW_ON_ERROR
        );
        self::assertNotEmpty($rows);
        foreach ($rows as $row) {
            $a = new Uuid((string) hex2bin($row['a_hex']));
            $b = new Uuid((string) hex2bin($row['b_hex']));
            $label = "{$row['a_hex']} vs {$row['b_hex']}";
            self::assertSame($row['eq'], $a == $b, "== {$label}");
            self::assertSame($row['cmp'], $a <=> $b, "<=> {$label}");
            self::assertSame($row['equals'], $a->equals($b), "equals {$label}");
        }
    }

    public function testCloneIsEqualAndIndependent(): void
    {
        $u = new Uuid(self::SAMPLE);
        $c = clone $u;
        self::assertTrue($c->equals($u));
        self::assertNotSame($c, $u);
        self::assertSame(self::SAMPLE, $c->bytes());
        self::assertTrue($c == $u);
    }

    public function testVarDumpShowsBytes(): void
    {
        // In a child without ini files: Xdebug, where it is installed, replaces var_dump's format.
        $out = self::child(
            ['-n', '-d', 'extension=' . self::ext()],
            'var_dump(new HyperUuid\Uuid(hex2bin("' . bin2hex(self::SAMPLE) . '")));'
        );
        self::assertStringContainsString('["bytes":"HyperUuid\Uuid":private]=>', $out);
        self::assertStringContainsString('string(16) "' . self::SAMPLE . '"', $out);
    }

    public function testPrintRAndArrayCastShowBytes(): void
    {
        $u = new Uuid(self::SAMPLE);
        self::assertSame(["\0HyperUuid\\Uuid\0bytes" => self::SAMPLE], (array) $u);
        self::assertStringContainsString(
            '[bytes:HyperUuid\Uuid:private] => ' . self::SAMPLE,
            print_r($u, true)
        );
    }

    public function testNonUtf8BytesRoundTrip(): void
    {
        $b = "\xff\xfe\x00\x80" . str_repeat("\xc3", 12);
        self::assertSame($b, (new Uuid($b))->bytes());
    }

    public function testReadonlyFromOutside(): void
    {
        $u = new Uuid(self::SAMPLE);
        $attempts = [
            'write' => static function () use ($u): void {
                $u->bytes = 'x';
            },
            'unset' => static function () use ($u): void {
                unset($u->bytes);
            },
            'reflection' => static function () use ($u): void {
                (new \ReflectionProperty(Uuid::class, 'bytes'))->setValue($u, str_repeat('a', 16));
            },
        ];
        foreach ($attempts as $name => $attempt) {
            try {
                $attempt();
                self::fail("{$name} did not throw");
            } catch (\Error $e) {
                self::assertMatchesRegularExpression(
                    '/readonly|Cannot (modify|access|unset)/',
                    $e->getMessage(),
                    $name
                );
            }
            self::assertSame(self::SAMPLE, $u->bytes(), $name);
        }
    }

    public function testReconstructionIsRefused(): void
    {
        $fresh = new Uuid(self::SAMPLE);
        $nil = Uuid::nil();
        foreach (['fresh' => $fresh, 'nil' => $nil] as $name => $u) {
            try {
                $u->__construct(str_repeat("\xff", 16));
                self::fail("{$name}: re-construction did not throw");
            } catch (\Error $e) {
                self::assertSame(\Error::class, $e::class, $name);
                self::assertSame('Cannot modify readonly property HyperUuid\Uuid::$bytes', $e->getMessage(), $name);
            }
        }
        self::assertSame(self::SAMPLE, $fresh->bytes());
        self::assertSame(str_repeat("\x00", 16), $nil->bytes());
        self::assertSame(str_repeat("\x00", 16), Uuid::nil()->bytes());
        self::assertSame($nil, Uuid::nil());
    }

    public function testConstructorRequiresBytes(): void
    {
        $this->expectException(\ArgumentCountError::class);
        new Uuid(); // @phpstan-ignore arguments.count
    }

    public function testWrongLengthMessage(): void
    {
        $this->expectException(\InvalidArgumentException::class);
        $this->expectExceptionMessage('bytes must be exactly 16 bytes');
        new Uuid('short');
    }
}
