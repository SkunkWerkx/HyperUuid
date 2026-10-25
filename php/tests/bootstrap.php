<?php

declare(strict_types=1);

// The core freshness check. Outside CI this suite loads the committed libraries under
// src/native/, which stage-native-binaries.yml refreshes only at release, so after a change to
// the core a plain phpunit run would test the core as it was at the last staging. The check
// finds the commit that staging was built from and refuses to run when rust/ differs from it.
// CI writes a freshly built core into src/native/ before the suite runs, which the check sees
// as a changed tree, and sets CI besides; HYPERUUID_NATIVE_LIBRARY, which loads another
// library in place of the committed one, skips it.

require __DIR__ . '/../vendor/autoload.php';

$staleCoreMessage = static function (): ?string {
    $tree = 'php/src/native';
    if ((string) getenv('CI') !== '' || (string) getenv('HYPERUUID_NATIVE_LIBRARY') !== '') {
        return null;
    }
    // git runs in the repository's top level once that is known (this directory until then),
    // and reports its exit code, or -1 when it could not run at all.
    $top = null;
    $git = static function (string ...$args) use (&$top): array {
        array_unshift($args, '-C', $top ?? __DIR__);
        $null = PHP_OS_FAMILY === 'Windows' ? 'NUL' : '/dev/null';
        try {
            $process = @proc_open(['git', ...$args], [1 => ['pipe', 'w'], 2 => ['file', $null, 'w']], $pipes);
        } catch (\Throwable) {
            return ['', -1];
        }
        if (!\is_resource($process)) {
            return ['', -1];
        }
        $out = (string) stream_get_contents($pipes[1]);
        fclose($pipes[1]);
        return [trim($out), proc_close($process)];
    };
    [$dir, $code] = $git('rev-parse', '--show-toplevel');
    if ($code !== 0 || $dir === '') {
        return null;
    }
    $top = $dir;
    [$staging] = $git('log', '-1', '--format=%H', '--grep=^chore: stage native binaries', 'HEAD');
    if ($staging === '') {
        return null;
    }
    [$source] = $git('log', '-1', '--format=%(trailers:key=Release-Source-SHA,valueonly)', $staging);
    if ($source === '') {
        [$subject] = $git('log', '-1', '--format=%s', $staging);
        if (preg_match('/\(([0-9a-f]{40})\)$/', $subject, $match) === 1) {
            $source = $match[1];
        }
    }
    if (\strlen($source) < 12) {
        return null;
    }
    if ($git('cat-file', '-e', "{$source}^{commit}")[1] !== 0) {
        return null;
    }
    // Fresh binaries written into the tree count; its READMEs do not.
    if ($git('diff', '--quiet', 'HEAD', '--', $tree, ":(exclude,glob){$tree}/**/README.md")[1] !== 0) {
        return null;
    }
    [, $code] = $git('diff', '--quiet', $source, '--', 'rust/src', 'rust/Cargo.toml', 'rust/Cargo.lock');
    [$untracked] = $git('ls-files', '--others', '--exclude-standard', '--', 'rust/src');
    if ($code !== 1 && $untracked === '') {
        return null;
    }
    $library = match (PHP_OS_FAMILY) {
        'Darwin' => 'libhyperuuid.dylib',
        'Windows' => 'hyperuuid.dll',
        default => 'libhyperuuid.so',
    };
    return \sprintf(
        "%s was built from rust/ at %s, and this checkout's rust/ differs from it, so these tests "
        . 'would run against a stale core. Build the checkout\'s own with .github/scripts/local-core.sh '
        . '(FORGE=<path> if the SkunkWerkx/.github checkout is not ../.github or ../HyperForge), then '
        . 'run %s instead; CI builds its own core and skips this check.',
        $tree,
        substr($source, 0, 12),
        "HYPERUUID_NATIVE_LIBRARY={$top}/rust/target/release/{$library} vendor/bin/phpunit",
    );
};

$message = $staleCoreMessage();
if ($message !== null) {
    fwrite(STDERR, $message . PHP_EOL);
    exit(1);
}
