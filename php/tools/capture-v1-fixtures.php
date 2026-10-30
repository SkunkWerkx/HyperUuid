<?php

/**
 * Records 1.0's (FFI-based) PHP behaviour as fixtures, for the parity tests of the extension
 * rewrite. Two modes:
 *
 *   HYPERUUID_NATIVE_LIBRARY=/abs/path/libhyperuuid.dylib php tools/capture-v1-fixtures.php [outdir]
 *
 * wrote provenance.json, surface.json, comparisons.json, serialized.txt and datetimes.json, on a
 * checkout of 466d19b, whose php/src it autoloaded; php/src is gone since.
 *
 *   HYPERUUID_NATIVE_LIBRARY=/abs/path/libhyperuuid.dylib \
 *       php tools/capture-v1-fixtures.php --coercion <v1-src> [outdir]
 *
 * adds the running PHP minor's recording to coercion.json (tests/Support/CoercionMatrix.php
 * says what it holds; the engine's own messages differ between minors, so each minor has its
 * own), loading 1.0's classes from `<v1-src>`, a copy of php/src from a commit that still has
 * it, extracted without touching the checkout:
 *
 *   git archive <commit> php/src | tar -x -C <scratch>    # <v1-src> is <scratch>/php/src
 *
 * The commit is recorded as the checkout's HEAD, or HYPERUUID_V1_COMMIT where the tool runs
 * outside the checkout (in a container, say); it must be the commit archived.
 *
 * Fixtures are add-only: the first mode is not run again over fixtures that exist, and the
 * second refuses a minor coercion.json already holds.
 *
 * Both refuse to run without HYPERUUID_NATIVE_LIBRARY, because Runtime prefers the committed
 * php/src/native library over the one named.
 */

declare(strict_types=1);

use HyperUuid\HyperUuid;
use HyperUuid\Tests\Support\CoercionMatrix;
use HyperUuid\Uuid;

$writeJson = static function (string $path, mixed $data): void {
    $flags = JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR;
    $json = json_encode($data, $flags);
    file_put_contents($path, $json . "\n");
};

$typeName = static function (?ReflectionType $type): ?string {
    return $type === null ? null : (string) $type;
};

$library = getenv('HYPERUUID_NATIVE_LIBRARY');
if ($library === false || $library === '' || !is_file($library)) {
    fwrite(STDERR, "HYPERUUID_NATIVE_LIBRARY must name an existing library file\n");
    exit(1);
}
$library = realpath($library);

$root = dirname(__DIR__);
require $root . '/vendor/autoload.php';
$repo = dirname($root);

$args = array_slice($argv, 1);
$v1Src = null;
if (($args[0] ?? null) === '--coercion') {
    $v1Src = realpath($args[1] ?? '');
    if ($v1Src === false || !is_file($v1Src . '/HyperUuid.php')) {
        fwrite(STDERR, "--coercion needs the path of a 1.0 php/src tree\n");
        exit(1);
    }
    $args = array_slice($args, 2);
    // 1.0's classes, one file each, straight under HyperUuid\.
    spl_autoload_register(static function (string $class) use ($v1Src): void {
        if (preg_match('/^HyperUuid\\\\(\w+)$/', $class, $m) === 1 && is_file("{$v1Src}/{$m[1]}.php")) {
            require "{$v1Src}/{$m[1]}.php";
        }
    });
}

$outDir = $args[0] ?? $root . '/tests/fixtures/v1';
if (!is_dir($outDir) && !mkdir($outDir, 0777, true)) {
    fwrite(STDERR, "cannot create {$outDir}\n");
    exit(1);
}

if ($v1Src !== null) {
    $target = $outDir . '/coercion.json';
    $minor = PHP_MAJOR_VERSION . '.' . PHP_MINOR_VERSION;
    $fixture = is_file($target)
        ? json_decode((string) file_get_contents($target), true, flags: JSON_THROW_ON_ERROR)
        : ['captures' => []];
    if (isset($fixture['captures'][$minor])) {
        fwrite(STDERR, "{$target} already holds PHP {$minor}; fixtures are add-only\n");
        exit(1);
    }
    $commit = getenv('HYPERUUID_V1_COMMIT');
    if ($commit === false || $commit === '') {
        $commit = trim((string) shell_exec('git -C ' . escapeshellarg($repo) . ' rev-parse HEAD 2>/dev/null'));
    }
    if (preg_match('/^[0-9a-f]{40}$/', $commit) !== 1) {
        fwrite(STDERR, "cannot tell which commit <v1-src> came from; set HYPERUUID_V1_COMMIT\n");
        exit(1);
    }
    // Where the library and the classes came from: a path relative to the archived tree (the
    // library as committed at that commit), so no machine's path is recorded.
    $relative = static function (string $path) use ($repo, $v1Src): string {
        $tree = dirname($v1Src, 2);
        foreach ([$tree, $repo] as $base) {
            if (str_starts_with($path, $base . '/')) {
                return substr($path, \strlen($base) + 1);
            }
        }
        return basename($path);
    };
    $scratch = sys_get_temp_dir() . '/hyperuuid-coercion-' . getmypid();
    if (!is_dir($scratch) && !mkdir($scratch, 0700, true)) {
        fwrite(STDERR, "cannot create {$scratch}\n");
        exit(1);
    }
    $matrix = CoercionMatrix::run($scratch);
    array_map('unlink', glob($scratch . '/*') ?: []);
    rmdir($scratch);
    $fixture['captures'][$minor] = [
        'provenance' => [
            'tool' => 'php/tools/capture-v1-fixtures.php --coercion',
            'classes' => 'php/src at source_sha (git archive), autoloaded',
            'library' => $relative($library),
            'library_sha256' => hash_file('sha256', $library),
            'native_version' => HyperUuid::nativeVersion(),
            'source_sha' => $commit,
            'php_version' => PHP_VERSION,
            'os' => PHP_OS_FAMILY,
        ],
        'matrix' => $matrix,
    ];
    ksort($fixture['captures']);
    $writeJson($target, $fixture);
    fwrite(STDOUT, "PHP {$minor} added to {$target}\n");
    exit(0);
}

// Provenance
$rustStatus = trim((string) shell_exec('git -C ' . escapeshellarg($repo) . ' status --porcelain rust/'));
$writeJson($outDir . '/provenance.json', [
    // Repo-relative, so no machine's path is committed; the hash and source_sha identify it.
    'library' => str_starts_with($library, $repo . '/') ? substr($library, \strlen($repo) + 1) : basename($library),
    'library_sha256' => hash_file('sha256', $library),
    'native_version' => HyperUuid::nativeVersion(),
    'source_sha' => trim((string) shell_exec('git -C ' . escapeshellarg($repo) . ' rev-parse HEAD')),
    'rust_tree_clean' => $rustStatus === '',
    'php_version' => PHP_VERSION,
]);

// Surface
$surface = [];
$classes = [
    'HyperUuid', 'Uuid', 'Namespaces', 'UuidLayout', 'UuidVariant',
    'CounterExhaustedException', 'RandomSourceException', 'TimestampOutOfRangeException',
];
foreach ($classes as $short) {
    $rc = new ReflectionClass('HyperUuid\\' . $short);
    $entry = [
        'kind' => $rc->isEnum() ? 'enum' : ($rc->isInterface() ? 'interface' : 'class'),
        'final' => $rc->isFinal(),
        'readonly' => $rc->isReadOnly(),
        'interfaces' => $rc->getInterfaceNames(),
        'parent' => $rc->getParentClass() ? $rc->getParentClass()->getName() : null,
        'constants' => [],
        'cases' => [],
        'methods' => [],
    ];
    sort($entry['interfaces']);
    foreach ($rc->getReflectionConstants() as $const) {
        if ($const->isEnumCase()) {
            $case = $const->getValue();
            $entry['cases'][$const->getName()] = $case instanceof BackedEnum ? $case->value : null;
        } else {
            $entry['constants'][$const->getName()] = $const->getValue();
        }
    }
    foreach ($rc->getMethods() as $m) {
        if ($m->getDeclaringClass()->getName() !== $rc->getName() || !$m->isPublic()) {
            continue;
        }
        if ($short === 'HyperUuid' && $m->getName() === 'isAvailable') {
            continue;
        }
        $params = [];
        foreach ($m->getParameters() as $p) {
            $default = null;
            if ($p->isDefaultValueAvailable()) {
                $default = $p->isDefaultValueConstant()
                    ? $p->getDefaultValueConstantName()
                    : var_export($p->getDefaultValue(), true);
            }
            $params[] = [
                'name' => $p->getName(),
                'type' => $typeName($p->getType()),
                'optional' => $p->isOptional(),
                'default' => $default,
            ];
        }
        $entry['methods'][$m->getName()] = [
            'static' => $m->isStatic(),
            'params' => $params,
            'return' => $typeName($m->getReturnType()),
        ];
    }
    ksort($entry['constants']);
    ksort($entry['methods']);
    foreach (['constants', 'cases', 'methods'] as $key) {
        $entry[$key] = (object) $entry[$key];
    }
    $surface[$short] = $entry;
}
$surface['removed'] = ['HyperUuid::isAvailable'];
$writeJson($outDir . '/surface.json', $surface);

// Comparisons
$v7 = Uuid::parse('018bcfe5-6800-7000-8000-000000000001');
$pairs = [
    [hex2bin('0123456789abcdeffedcba9876543210'), hex2bin('0123456789abcdeffedcba9876543210')],
    [hex2bin('9f1c2b3a4d5e6f708192a3b4c5d6e7f8'), hex2bin('3a7d0c1e2b4f5a6978887766554433aa')],
    ['0000000000000001', '1e00000000000000'],
    [Uuid::nil()->bytes(), Uuid::max()->bytes()],
    [$v7->bytes(), $v7->toSqlOrder()->bytes()],
];
$comparisons = [];
foreach ($pairs as [$a, $b]) {
    $ua = new Uuid($a);
    $ub = new Uuid($b);
    $comparisons[] = [
        'a_hex' => bin2hex($ua->bytes()),
        'b_hex' => bin2hex($ub->bytes()),
        'eq' => $ua == $ub,
        'cmp' => $ua <=> $ub,
        'equals' => $ua->equals($ub),
    ];
}
$writeJson($outDir . '/comparisons.json', $comparisons);

// Serialization
file_put_contents(
    $outDir . '/serialized.txt',
    serialize(new Uuid(hex2bin('0123456789abcdeffedcba9876543210')))
);

// DateTime cases
$utc = new DateTimeZone('UTC');
$cases = [
    ['2026-10-30T12:34:56.789999Z', fn() => new DateTimeImmutable('2026-10-30T12:34:56.789999Z')],
    ['2026-10-30T12:34:56.000500+05:30', fn() => new DateTimeImmutable('2026-10-30T12:34:56.000500+05:30')],
    ['1970-01-01T00:00:00.000Z', fn() => new DateTimeImmutable('1970-01-01T00:00:00.000Z')],
    ['1969-12-31T23:59:59.999Z', fn() => new DateTimeImmutable('1969-12-31T23:59:59.999Z')],
    ['@281474976710.655000', fn() => DateTimeImmutable::createFromFormat('U.u', '281474976710.655000', $utc)],
    ['@281474976710.656000', fn() => DateTimeImmutable::createFromFormat('U.u', '281474976710.656000', $utc)],
];
$datetimes = [];
foreach ($cases as [$input, $make]) {
    $row = ['input' => $input];
    try {
        $row['millis'] = HyperUuid::newV7($make())->unixMillis();
    } catch (Throwable $e) {
        $row['exception_class'] = get_class($e);
        $row['message'] = $e->getMessage();
    }
    $datetimes[] = $row;
}
$writeJson($outDir . '/datetimes.json', $datetimes);

fwrite(STDOUT, "fixtures written to {$outDir}\n");
