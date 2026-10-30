<?php

/**
 * Regenerates stubs/hyperuuid.stub.php, the IDE and static-analysis stubs for the extension:
 *
 *   php tools/generate-stubs.php [--out <file>]
 *
 * from php/ on Linux or macOS, with cargo-php CARGO_PHP_VERSION installed
 * (`cargo install cargo-php --version <it> --locked`) and a C compiler on PATH. Steps:
 *
 * 1. Builds the extension's debug library, `cargo rustc --crate-type cdylib --features php
 *    --target-dir target/php` in rust/ (on Linux with `-- -C link-arg=-Wl,-z,lazy`, below):
 *    ext-php-rs exports the description cargo-php reads (`ext_php_rs_describe_module_v2`) only
 *    from a debug build. Release builds are untouched.
 * 2. Runs `cargo php stubs --stdout` on it, preloading a library that defines the PHP data
 *    symbols the extension references but cargo-php doesn't stand in for (its own list covers
 *    ext-php-rs's bindings only), since a data symbol must resolve when the library loads.
 * 3. Applies REWRITES (what the extension sets after registration, which cargo-php's view of
 *    the builders can't show), each of which must match exactly as often as recorded, and
 *    appends EXCEPTIONS (classes registered by hand, which cargo-php can't see).
 *
 * tests/StubTest.php runs this into a temporary file and compares it with the committed stub.
 */

declare(strict_types=1);

// A script, run directly: its helper functions sit beside the steps that use them.
// phpcs:disable PSR1.Files.SideEffects.FoundWithSymbols

const CARGO_PHP_VERSION = '0.2.1';

/** PHP data symbols the extension imports beyond cargo-php's stand-ins, as null pointers. */
const SYMBOLS_C = <<<'C'
    void *php_json_serializable_ce, *spl_ce_InvalidArgumentException, *spl_ce_RuntimeException;
    void *zend_empty_string, *zend_one_char_string[256], *zend_post_startup_cb;
    C;

/** [search, replace, occurrences]. */
const REWRITES = [
    // registration.rs widens each $unixMillis from ext-php-rs's `mixed` (signature and @param).
    ['mixed $unixMillis', '\\DateTimeInterface|int|null $unixMillis', 6],
    // ...and clears the nullable bit ext-php-rs is told about for Uuid's defaulted parameters
    // (args.rs, `Flag`), as 1.0 declared them.
    ['?bool $throwOnMismatch', 'bool $throwOnMismatch', 1],
    ['@param bool|null $throwOnMismatch', '@param bool $throwOnMismatch', 1],
    ['?\\HyperUuid\\UuidLayout $layout', '\\HyperUuid\\UuidLayout $layout', 4],
    ['@param \\HyperUuid\\UuidLayout|null $layout', '@param \\HyperUuid\\UuidLayout $layout', 4],
    // The classes are registered final; cargo-php's stubs leave the modifier out.
    ["    class HyperUuid {\n", "    final class HyperUuid {\n", 1],
    ["    class Namespaces {\n", "    final class Namespaces {\n", 1],
    ["    class Uuid implements \\JsonSerializable {\n", "    final class Uuid implements \\JsonSerializable {\n", 1],
];

/** errors.rs registers these in MINIT as plain subclasses, outside cargo-php's view. */
const EXCEPTIONS = <<<'PHP'

    namespace HyperUuid {
        /**
         * The version 7 stream cannot advance without running more than 1000 ms ahead of the
         * supplied timestamp or past its last millisecond.
         */
        final class CounterExhaustedException extends \RuntimeException {}

        /**
         * The operating system's random source failed.
         */
        final class RandomSourceException extends \RuntimeException {}

        /**
         * A timestamp doesn't fit the RFC 9562 48-bit field.
         */
        final class TimestampOutOfRangeException extends \RuntimeException {}
    }

    PHP;

const HEADER = <<<'PHP'
    <?php

    /**
     * Stubs for the hyperuuid extension, for IDEs and static analysis. Generated: don't edit.
     * Regenerate from php/ with `php tools/generate-stubs.php`, which runs
     * `cargo php stubs --stdout` (cargo-php %s) on the debug build of the extension and
     * completes its output (the tool says how).
     */

    PHP;

function fail(string $message): never
{
    fwrite(STDERR, $message . PHP_EOL);
    exit(1);
}

/** Runs `$command` in `$cwd` with `$env` added; returns stdout, or fails with stderr. */
function run(array $command, string $cwd, array $env = []): string
{
    $process = proc_open(
        $command,
        [1 => ['pipe', 'w'], 2 => ['pipe', 'w']],
        $pipes,
        $cwd,
        $env === [] ? null : [...getenv(), ...$env]
    );
    if (!is_resource($process)) {
        fail('could not start ' . $command[0]);
    }
    $stdout = (string) stream_get_contents($pipes[1]);
    $stderr = (string) stream_get_contents($pipes[2]);
    fclose($pipes[1]);
    fclose($pipes[2]);
    if (proc_close($process) !== 0) {
        fail(implode(' ', $command) . " failed:\n" . $stderr . $stdout);
    }
    return $stdout;
}

if (PHP_OS_FAMILY === 'Windows') {
    fail('cargo-php generates stubs on Linux and macOS only');
}

$root = dirname(__DIR__);
$rust = dirname($root) . '/rust';
$out = $root . '/stubs/hyperuuid.stub.php';
if (($argv[1] ?? null) === '--out' && isset($argv[2])) {
    $out = $argv[2];
} elseif (count($argv) > 1) {
    fail('usage: php tools/generate-stubs.php [--out <file>]');
}

$version = trim(run(['cargo', 'php', '--version'], $rust));
if (!str_starts_with($version, 'cargo-php ' . CARGO_PHP_VERSION . ' ')) {
    fail("cargo-php " . CARGO_PHP_VERSION . " is required (its output is the stub's format), found: {$version}");
}

$darwin = PHP_OS_FAMILY === 'Darwin';
$build = ['cargo', 'rustc', '--quiet', '--crate-type', 'cdylib', '--features', 'php', '--target-dir', 'target/php'];
// Rust links with `-z now` on Linux, which binds every function when the library loads, and
// cargo-php stands in only for ext-php-rs's bindings; `-z lazy` defers the rest, as on macOS.
// The flag reaches only this crate's link, so the cached dependencies are reused.
$linux = $darwin ? [] : ['--', '-C', 'link-arg=-Wl,-z,lazy'];
run([...$build, ...$linux], $rust);
$library = $rust . '/target/php/debug/' . ($darwin ? 'libhyperuuid.dylib' : 'libhyperuuid.so');

$scratch = sys_get_temp_dir() . '/hyperuuid-stubs-' . getmypid();
if (!is_dir($scratch) && !mkdir($scratch, 0700, true)) {
    fail("cannot create {$scratch}");
}
// `fail()` exits, which skips `finally`; a shutdown function runs on every way out.
register_shutdown_function(static function () use ($scratch): void {
    foreach (glob($scratch . '/*') ?: [] as $file) {
        unlink($file);
    }
    if (is_dir($scratch)) {
        rmdir($scratch);
    }
});
$symbols = $scratch . '/symbols' . ($darwin ? '.dylib' : '.so');
file_put_contents($scratch . '/symbols.c', SYMBOLS_C . "\n");
run(['cc', '-shared', '-fPIC', '-o', $symbols, $scratch . '/symbols.c'], $scratch);
$preload = $darwin ? ['DYLD_INSERT_LIBRARIES' => $symbols] : ['LD_PRELOAD' => $symbols];
$stubs = run(['cargo', 'php', 'stubs', '--stdout', $library], $rust, $preload);

$prefix = "<?php\n\n// Stubs for hyperuuid\n\n";
if (!str_starts_with($stubs, $prefix)) {
    fail("cargo-php's output no longer starts with the expected lines:\n" . substr($stubs, 0, 200));
}
$stubs = substr($stubs, strlen($prefix));
foreach (REWRITES as [$search, $replace, $expected]) {
    $count = substr_count($stubs, $search);
    if ($count !== $expected) {
        fail("rewrite of '{$search}' expects {$expected} matches, found {$count}");
    }
    $stubs = str_replace($search, $replace, $stubs);
}
$stubs = sprintf(HEADER, CARGO_PHP_VERSION) . "\n" . rtrim($stubs, "\n") . "\n" . EXCEPTIONS;
if (file_put_contents($out, $stubs) === false) {
    fail("cannot write {$out}");
}
