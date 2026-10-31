<?php

declare(strict_types=1);

namespace HyperUuid\Tests;

use PHPUnit\Framework\TestCase;

/**
 * The committed stubs/hyperuuid.stub.php is what tools/generate-stubs.php makes from the
 * current source. It needs cargo-php, a C compiler and a debug build of the extension, so it
 * runs only with HYPERUUID_CHECK_STUBS=1 (CI sets it on one leg), and never on Windows, where
 * cargo-php can't generate stubs.
 */
final class StubTest extends TestCase
{
    /**
     * Runs everywhere, without cargo-php: the committed stub declares exactly the extension's
     * `HyperUuid\` classes, enums and interfaces, each class with the loaded one's parent and
     * `final` flag. This is what holds the exceptions the tool appends by hand to the extension.
     */
    public function testStubDeclaresTheLoadedClasses(): void
    {
        $stub = (string) file_get_contents(\dirname(__DIR__) . '/stubs/hyperuuid.stub.php');
        preg_match_all('/^\s*namespace\s+([\w\\\\]+)\s*\{/m', $stub, $namespaces);
        self::assertNotEmpty($namespaces[1]);
        self::assertSame(['HyperUuid'], array_values(array_unique($namespaces[1])));

        $pattern = '/^\s*(final\s+)?(class|enum|interface)\s+(\w+)(?:\s+extends\s+\\\\?([\w\\\\]+))?/m';
        preg_match_all($pattern, $stub, $matches, PREG_SET_ORDER);
        $declared = [];
        foreach ($matches as $m) {
            $declared['HyperUuid\\' . $m[3]] = ['kind' => $m[2], 'final' => $m[1] !== '', 'parent' => $m[4] ?? ''];
        }

        // Every internal HyperUuid\ class, enum or interface; the test suite's own are user code.
        $loaded = array_filter(
            [...get_declared_classes(), ...get_declared_interfaces()],
            static fn (string $name): bool => str_starts_with($name, 'HyperUuid\\')
                && (new \ReflectionClass($name))->isInternal(),
        );
        $loaded = array_values(array_unique($loaded));
        sort($loaded);
        $names = array_keys($declared);
        sort($names);
        self::assertSame($loaded, $names, 'the stub declares other classes than the extension');

        foreach ($declared as $name => $decl) {
            if ($decl['kind'] !== 'class') {
                continue;
            }
            $class = new \ReflectionClass($name);
            self::assertSame($class->isFinal(), $decl['final'], "{$name}: final");
            $parent = $class->getParentClass();
            self::assertSame($parent === false ? '' : $parent->getName(), $decl['parent'], "{$name}: parent");
        }
    }

    public function testStubIsCurrent(): void
    {
        if (PHP_OS_FAMILY === 'Windows') {
            self::markTestSkipped('cargo-php generates stubs on Linux and macOS only');
        }
        if (getenv('HYPERUUID_CHECK_STUBS') !== '1') {
            self::markTestSkipped('set HYPERUUID_CHECK_STUBS=1 to regenerate and compare the stubs');
        }
        $root = \dirname(__DIR__);
        $generated = tempnam(sys_get_temp_dir(), 'hyperuuid-stub');
        self::assertIsString($generated);
        try {
            $process = proc_open(
                [PHP_BINARY, $root . '/tools/generate-stubs.php', '--out', $generated],
                [1 => ['pipe', 'w'], 2 => ['pipe', 'w']],
                $pipes,
                $root
            );
            self::assertIsResource($process);
            $output = stream_get_contents($pipes[1]) . stream_get_contents($pipes[2]);
            fclose($pipes[1]);
            fclose($pipes[2]);
            self::assertSame(0, proc_close($process), "generate-stubs.php failed:\n{$output}");
            self::assertFileEquals($generated, $root . '/stubs/hyperuuid.stub.php');
        } finally {
            unlink($generated);
        }
    }
}
