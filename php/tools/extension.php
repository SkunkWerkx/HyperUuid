<?php

declare(strict_types=1);

// The extension `composer test` and `composer bench` load: the local cargo build (`cargo
// php-ext` in rust/) unless HYPERUUID_PHP_EXTENSION names another, which CI does. Resolved to
// an absolute path and exported back into HYPERUUID_PHP_EXTENSION, so child processes load
// the same file. Exits with a message when it is not built.

function hyperuuid_extension(string $root): string
{
    $path = getenv('HYPERUUID_PHP_EXTENSION');
    if (!\is_string($path) || $path === '') {
        $library = match (PHP_OS_FAMILY) {
            'Darwin' => 'libhyperuuid.dylib',
            'Windows' => 'hyperuuid.dll',
            default => 'libhyperuuid.so',
        };
        $path = $root . '/../rust/target/php/release/' . $library;
    }
    $resolved = realpath($path);
    if ($resolved === false || !is_file($resolved)) {
        fwrite(
            STDERR,
            "the hyperuuid extension is not built: {$path} not found (run cargo php-ext in rust/)" . PHP_EOL
        );
        exit(1);
    }
    putenv('HYPERUUID_PHP_EXTENSION=' . $resolved);
    return $resolved;
}

/** Runs $command from $root with this process's standard streams, and exits with its status. */
function hyperuuid_run(array $command, string $root, string $what): never
{
    $process = proc_open($command, [0 => STDIN, 1 => STDOUT, 2 => STDERR], $pipes, $root);
    if (!\is_resource($process)) {
        fwrite(STDERR, "could not start {$what}" . PHP_EOL);
        exit(1);
    }
    exit(proc_close($process));
}
