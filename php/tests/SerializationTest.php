<?php

declare(strict_types=1);

namespace HyperUuid\Tests;

use HyperUuid\Tests\Support\ChildProcess;
use HyperUuid\Uuid;
use PHPUnit\Framework\TestCase;

/**
 * `serialize` and `unserialize` keep the 1.0 class's payload, byte for byte, in both
 * directions: 1.0's payloads sit in sessions and caches today, and a 1.0 reader may meet a
 * 1.1 payload. A payload 1.0 could never have written is refused with an exception, never a
 * crash or a half-built value.
 */
final class SerializationTest extends TestCase
{
    use ChildProcess;

    private const SAMPLE_HEX = '0123456789abcdeffedcba9876543210';

    private const KEY = "\0HyperUuid\\Uuid\0bytes";

    /** 1.0's `serialize(new Uuid(hex2bin(SAMPLE_HEX)))`; HYPERUUID_V1_SERIALIZED names another. */
    private static function v1Payload(): string
    {
        $path = getenv('HYPERUUID_V1_SERIALIZED');
        if (!\is_string($path) || $path === '') {
            $path = __DIR__ . '/fixtures/v1/serialized.txt';
        }
        $payload = file_get_contents($path);
        self::assertIsString($payload);
        return $payload;
    }

    /** A Uuid payload whose body is `$count` entries, serialized as `$entries`. */
    private static function payload(int $count, string $entries): string
    {
        return 'O:14:"HyperUuid\\Uuid":' . $count . ':{' . $entries . '}';
    }

    private static function key(): string
    {
        return 's:21:"' . self::KEY . '";';
    }

    /** Runs `unserialize($payload)` in a child and reports what it returned or threw. */
    private static function unserializeInChild(string $payload): string
    {
        return self::child(
            ['-n', '-d', 'extension=' . self::ext()],
            '$p = base64_decode("' . base64_encode($payload) . '");'
            . 'try { $r = unserialize($p); echo "returned ", get_debug_type($r); }'
            . ' catch (\Throwable $e) { echo $e::class, ": ", $e->getMessage(); }'
        );
    }

    public function testV1PayloadUnserializes(): void
    {
        $u = unserialize(self::v1Payload());
        self::assertInstanceOf(Uuid::class, $u);
        self::assertSame(hex2bin(self::SAMPLE_HEX), $u->bytes());
        self::assertSame('01234567-89ab-cdef-fedc-ba9876543210', (string) $u);
    }

    public function testPayloadIsByteIdenticalToV1(): void
    {
        self::assertSame(self::v1Payload(), serialize(new Uuid((string) hex2bin(self::SAMPLE_HEX))));
    }

    public function testRoundTrip(): void
    {
        for ($i = 0; $i < 64; $i++) {
            $bytes = random_bytes(16);
            $u = unserialize(serialize(new Uuid($bytes)));
            self::assertInstanceOf(Uuid::class, $u);
            self::assertSame($bytes, $u->bytes());
            self::assertTrue($u == new Uuid($bytes));
        }
        $nil = unserialize(serialize(Uuid::nil()));
        self::assertInstanceOf(Uuid::class, $nil);
        self::assertSame(str_repeat("\0", 16), $nil->bytes());
        self::assertNotSame(Uuid::nil(), $nil);
    }

    public function testSerializeReturnsTheMangledKey(): void
    {
        $bytes = (string) hex2bin(self::SAMPLE_HEX);
        self::assertSame([self::KEY => $bytes], (new Uuid($bytes))->__serialize());
    }

    public function testNestedAndSharedValues(): void
    {
        $u = new Uuid(random_bytes(16));
        $back = unserialize(serialize(['a' => $u, 'b' => $u, 'c' => [new Uuid(random_bytes(16))]]));
        self::assertIsArray($back);
        self::assertSame($back['a'], $back['b']);
        self::assertSame($u->bytes(), $back['a']->bytes());
        self::assertInstanceOf(Uuid::class, $back['c'][0]);
    }

    public function testReferencedStringIsAccepted(): void
    {
        // `R:2;` points the bytes entry at the array's first element, a 16-byte string, as a
        // PHP reference. 1.0's typed property took the referenced string; so does this.
        $bytes = (string) hex2bin(self::SAMPLE_HEX);
        $payload = 'a:2:{i:0;s:16:"' . $bytes . '";i:1;' . self::payload(1, self::key() . 'R:2;') . '}';
        $back = unserialize($payload);
        self::assertIsArray($back);
        self::assertInstanceOf(Uuid::class, $back[1]);
        self::assertSame($bytes, $back[1]->bytes());
    }

    public function testHostilePayloads(): void
    {
        $sixteen = 's:16:"' . str_repeat('a', 16) . '";';
        $cases = [
            'int value' => [
                self::payload(1, self::key() . 'i:5;'),
                'TypeError: Cannot assign int to property HyperUuid\Uuid::$bytes of type string',
            ],
            'true value' => [
                self::payload(1, self::key() . 'b:1;'),
                'TypeError: Cannot assign true to property HyperUuid\Uuid::$bytes of type string',
            ],
            'null value' => [
                self::payload(1, self::key() . 'N;'),
                'TypeError: Cannot assign null to property HyperUuid\Uuid::$bytes of type string',
            ],
            'array value' => [
                self::payload(1, self::key() . 'a:0:{}'),
                'TypeError: Cannot assign array to property HyperUuid\Uuid::$bytes of type string',
            ],
            'object value' => [
                self::payload(1, self::key() . 'O:8:"stdClass":0:{}'),
                'TypeError: Cannot assign stdClass to property HyperUuid\Uuid::$bytes of type string',
            ],
            'Uuid value' => [
                self::payload(1, self::key() . self::payload(1, self::key() . $sixteen)),
                'TypeError: Cannot assign HyperUuid\Uuid to property HyperUuid\Uuid::$bytes of type string',
            ],
            'key missing' => [
                self::payload(0, ''),
                'InvalidArgumentException: invalid serialization data for HyperUuid\Uuid',
            ],
            'unmangled key' => [
                self::payload(1, 's:5:"bytes";' . $sixteen),
                'InvalidArgumentException: invalid serialization data for HyperUuid\Uuid',
            ],
            'integer key' => [
                self::payload(1, 'i:0;' . $sixteen),
                'InvalidArgumentException: invalid serialization data for HyperUuid\Uuid',
            ],
            'extra key' => [
                self::payload(2, self::key() . $sixteen . 's:1:"x";i:1;'),
                'InvalidArgumentException: invalid serialization data for HyperUuid\Uuid',
            ],
            '15 bytes' => [
                self::payload(1, self::key() . 's:15:"' . str_repeat('a', 15) . '";'),
                'InvalidArgumentException: bytes must be exactly 16 bytes',
            ],
            '17 bytes' => [
                self::payload(1, self::key() . 's:17:"' . str_repeat('a', 17) . '";'),
                'InvalidArgumentException: bytes must be exactly 16 bytes',
            ],
            'empty string' => [
                self::payload(1, self::key() . 's:0:"";'),
                'InvalidArgumentException: bytes must be exactly 16 bytes',
            ],
            'inside an array' => [
                'a:1:{i:0;' . self::payload(1, self::key() . 's:15:"' . str_repeat('a', 15) . '";') . '}',
                'InvalidArgumentException: bytes must be exactly 16 bytes',
            ],
        ];
        foreach ($cases as $name => [$payload, $expected]) {
            self::assertSame($expected, self::unserializeInChild($payload), $name);
        }
    }

    public function testFailedUnserializeLeavesNoUsableValue(): void
    {
        // A class with a destructor and no __wakeup or __unserialize has no delayed call to
        // skip, so PHP runs its destructor after the Uuid's __unserialize fails, and the
        // destructor can reach the Uuid PHP had already allocated. What it finds holds no
        // bytes: every method refuses, and nothing reads as a default value.
        $inner = self::payload(1, self::key() . 's:15:"' . str_repeat('a', 15) . '";');
        $payload = 'O:5:"Stash":1:{s:1:"u";' . $inner . '}';
        $out = self::child(
            ['-n', '-d', 'extension=' . self::ext()],
            'class Stash { public $u; public function __destruct() { $GLOBALS["g"] = $this->u; } }'
            . '$p = base64_decode("' . base64_encode($payload) . '");'
            . 'try { unserialize($p); echo "returned\n"; }'
            . ' catch (\Throwable $e) { echo $e::class, ": ", $e->getMessage(), "\n"; }'
            . '$g = $GLOBALS["g"] ?? null; echo get_debug_type($g), "\n";'
            . 'foreach (["bytes" => fn () => $g->bytes(), "string" => fn () => (string) $g,'
            . ' "serialize" => fn () => serialize($g), "equals" => fn () => $g->equals($g),'
            . ' "json" => fn () => json_encode($g, JSON_THROW_ON_ERROR)] as $n => $f) {'
            . ' try { $r = $f(); echo $n, " returned ", var_export($r, true), "\n"; }'
            . ' catch (\Throwable $e) { echo $n, " ", $e::class, "\n"; } }'
        );
        $lines = explode("\n", $out);
        self::assertSame('InvalidArgumentException: bytes must be exactly 16 bytes', $lines[0], $out);
        if ($lines[1] === 'null') {
            self::markTestSkipped('this PHP no longer runs the destructor gadget after a failed unserialize');
        }
        self::assertSame(Uuid::class, $lines[1], $out);
        foreach (\array_slice($lines, 2) as $line) {
            self::assertStringNotContainsString(' returned ', $line, $out);
        }
        self::assertCount(7, $lines, $out);
    }

    public function testSessionDecodedAtRequestStartup(): void
    {
        // `session.auto_start` decodes the session in ext/session's RINIT, which may run
        // before this extension's: the class must accept unserialization by then. Served by
        // `php -S`, the one SAPI every PHP build carries, twice, so a request that broke the
        // first would show in the second.
        if (!\extension_loaded('session')) {
            self::markTestSkipped('ext/session is not built in');
        }
        $bytes = (string) hex2bin(self::SAMPLE_HEX);
        $dir = sys_get_temp_dir() . '/hyperuuid-session-' . bin2hex(random_bytes(6));
        self::assertTrue(mkdir($dir . '/save', 0700, true));
        self::assertTrue(mkdir($dir . '/root', 0700, true));
        $session = 'abc123';
        file_put_contents("{$dir}/save/sess_{$session}", 'u|' . serialize(new Uuid($bytes)));
        file_put_contents(
            "{$dir}/root/index.php",
            '<?php $u = $_SESSION["u"] ?? null; echo $u instanceof HyperUuid\\Uuid'
            . ' ? bin2hex($u->bytes()) : "not a Uuid: " . get_debug_type($u);'
        );
        $socket = stream_socket_server('tcp://127.0.0.1:0');
        self::assertIsResource($socket);
        $address = (string) stream_socket_get_name($socket, false);
        fclose($socket);
        // Quoted: `-d` values are INI expressions, and Windows' temp directory is a short path
        // (`C:\Users\RUNNER~1\...`) whose `~` would otherwise be read as an operator.
        $server = proc_open(
            [
                PHP_BINARY, '-n', '-d', 'extension=' . self::ext(),
                '-d', 'session.auto_start=1', '-d', "session.save_path=\"{$dir}/save\"",
                '-d', 'session.use_strict_mode=0', '-d', 'display_errors=1',
                '-S', $address, '-t', "{$dir}/root",
            ],
            [1 => ['file', "{$dir}/server.log", 'w'], 2 => ['file', "{$dir}/server.log", 'a']],
            $pipes
        );
        self::assertIsResource($server);
        try {
            $context = stream_context_create(['http' => [
                'header' => "Cookie: PHPSESSID={$session}\r\n",
                'ignore_errors' => true,
                'timeout' => 10,
            ]]);
            $deadline = microtime(true) + 10;
            while (($probe = @stream_socket_client("tcp://{$address}", $errno, $error, 1)) === false) {
                self::assertLessThan($deadline, microtime(true), "php -S did not start: {$error}");
                usleep(50_000);
            }
            fclose($probe);
            foreach ([1, 2] as $request) {
                $body = file_get_contents("http://{$address}/index.php", false, $context);
                self::assertSame(self::SAMPLE_HEX, $body, "request {$request}");
            }
        } finally {
            proc_terminate($server);
            proc_close($server);
            foreach (['save/sess_' . $session, 'root/index.php', 'server.log'] as $file) {
                @unlink("{$dir}/{$file}");
            }
            @rmdir("{$dir}/save");
            @rmdir("{$dir}/root");
            @rmdir($dir);
        }
    }

    public function testUnserializeOnALiveValueIsRefused(): void
    {
        $sample = (string) hex2bin(self::SAMPLE_HEX);
        $fresh = new Uuid($sample);
        $nil = Uuid::nil();
        foreach (['fresh' => $fresh, 'nil' => $nil] as $name => $u) {
            foreach ([[self::KEY => str_repeat("\xff", 16)], [], ['x' => 1]] as $i => $data) {
                try {
                    $u->__unserialize($data);
                    self::fail("{$name} #{$i}: __unserialize on a live value did not throw");
                } catch (\Error $e) {
                    self::assertSame(\Error::class, $e::class, "{$name} #{$i}");
                    self::assertSame(
                        'Cannot modify readonly property HyperUuid\Uuid::$bytes',
                        $e->getMessage(),
                        "{$name} #{$i}"
                    );
                }
            }
        }
        self::assertSame($sample, $fresh->bytes());
        self::assertSame(str_repeat("\0", 16), $nil->bytes());
        self::assertSame(str_repeat("\0", 16), Uuid::nil()->bytes());
    }

    public function testUnserializeTakesAnArray(): void
    {
        $this->expectException(\TypeError::class);
        $this->expectExceptionMessage(
            'HyperUuid\Uuid::__unserialize(): Argument #1 ($data) must be of type array, string given'
        );
        (new Uuid(random_bytes(16)))->__unserialize('x'); // @phpstan-ignore argument.type
    }
}
