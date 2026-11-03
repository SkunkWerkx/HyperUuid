<?php

declare(strict_types=1);

namespace HyperUuid\Tests;

use HyperUuid\Tests\Support\ChildProcess;
use PHPUnit\Framework\TestCase;

/**
 * How the extension loads on each kind of PHP. At startup (`extension=` in php.ini, which PIE
 * writes) it loads on both. A `dl()` loads it on an NTS PHP only: on a thread-safe one each
 * thread that called `dl()` would register its classes again into state every thread shares,
 * so its MINIT refuses with a warning naming the php.ini line, and PHP ends the script, as it
 * does for any module that fails to start under `dl()`. Each leg runs the test for its own PHP.
 */
final class ThreadSafetyTest extends TestCase
{
    use ChildProcess;

    /**
     * `dl()`s the build the run was started with into a child that loads nothing else.
     *
     * @return array{int, string, string}
     */
    private static function dlChild(): array
    {
        $ext = self::ext();
        return self::childRun(
            ['-n', '-d', 'enable_dl=1', '-d', 'extension_dir=' . \dirname($ext), '-d', 'display_errors=stderr'],
            'var_dump(dl(' . var_export(basename($ext), true) . '), extension_loaded("hyperuuid"));'
        );
    }

    public function testAStartupLoadWorksOnThisPhp(): void
    {
        $out = self::child(['-n', '-d', 'extension=' . self::ext()], 'echo (int) PHP_ZTS, " ", (int) extension_loaded("hyperuuid");');
        self::assertSame((PHP_ZTS ? '1' : '0') . ' 1', $out);
    }

    public function testANonThreadSafePhpLoadsItWithDl(): void
    {
        if (PHP_ZTS) {
            self::markTestSkipped('this PHP is thread-safe; testAThreadSafePhpRefusesDl runs here');
        }
        [$status, $out, $err] = self::dlChild();
        self::assertSame(0, $status, $err);
        self::assertSame("bool(true)\nbool(true)", trim($out), $err);
    }

    public function testAThreadSafePhpRefusesDl(): void
    {
        if (!PHP_ZTS) {
            self::markTestSkipped('this PHP is NTS; testANonThreadSafePhpLoadsItWithDl runs here');
        }
        [$status, $out, $err] = self::dlChild();
        self::assertSame(255, $status, $out . $err);
        self::assertSame('', trim($out), $err);
        self::assertStringContainsString('Unable to start hyperuuid module', $err);
        self::assertStringContainsString(
            'hyperuuid: a thread-safe PHP loads this extension at startup only: add extension=hyperuuid to php.ini (PIE writes it) rather than calling dl()',
            $err
        );
    }
}
