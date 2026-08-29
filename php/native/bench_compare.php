<?php

declare(strict_types=1);

/**
 * Benchmark spike: PHP's ext-ffi path (../src/Runtime.php) vs. this crate's ext-php-rs
 * native extension, at the same layer — raw 16-byte strings in/out, no Uuid value-object
 * construction on either side. Not part of the shipped package or its phpbench suite; run
 * manually after `cargo build --release` in this directory:
 *
 *   php -d extension=$(pwd)/target/release/libhyperuuid_php_native.so bench_compare.php
 *
 * Methodology matches ../bench/UuidBench.php: 5 iterations x 1000 revs, best-of taken as
 * the per-iteration mode would be (phpbench reports the modal iteration; a single process
 * has more jitter, so this reports min and median of the 5 iteration means, both in ns/call).
 */

require __DIR__ . '/../vendor/autoload.php';

use HyperUuid\Namespaces;
use HyperUuid\Runtime;

if (!\function_exists('hyperuuid_native_new_v4')) {
    fwrite(STDERR, "hyperuuid_php_native extension not loaded — pass -d extension=.../libhyperuuid_php_native.so\n");
    exit(1);
}

const ITERATIONS = 5;
const REVS = 1000;
const RFC_TEST_VECTOR_MS = 1_645_557_742_000;

$dnsNamespace = Namespaces::dns()->bytes();

/** @param callable(): void $fn */
function timeIterations(callable $fn, int $revs): array
{
    $means = [];
    for ($i = 0; $i < ITERATIONS; $i++) {
        $start = \hrtime(true);
        for ($r = 0; $r < $revs; $r++) {
            $fn();
        }
        $elapsed = \hrtime(true) - $start;
        $means[] = $elapsed / $revs;
    }
    sort($means);
    return [
        'min_ns' => $means[0],
        'median_ns' => $means[(int) floor(ITERATIONS / 2)],
    ];
}

/** @param array<string, callable(): void> $cases */
function report(string $label, array $cases, int $revs = REVS, int $warmup = 100): void
{
    echo "== {$label} ==\n";
    $results = [];
    foreach ($cases as $name => $fn) {
        // Warm up (JIT/opcache, first-call FFI setup) before timing.
        for ($w = 0; $w < $warmup; $w++) {
            $fn();
        }
        $results[$name] = timeIterations($fn, $revs);
    }
    $baseline = null;
    foreach ($results as $name => $r) {
        $baseline ??= $r['min_ns'];
        $ratio = $baseline / $r['min_ns'];
        printf(
            "  %-28s min=%7.1fns  median=%7.1fns  (%.2fx vs. first)\n",
            $name,
            $r['min_ns'],
            $r['median_ns'],
            $ratio
        );
    }
    echo "\n";
}

report('newV4', [
    'ffi (Runtime::newV4)' => static fn () => Runtime::newV4(),
    'native (ext-php-rs)' => static fn () => hyperuuid_native_new_v4(),
]);

report('newV5', [
    'ffi (Runtime::newV5)' => static fn () => Runtime::newV5($dnsNamespace, 'example.com'),
    'native (ext-php-rs)' => static fn () => hyperuuid_native_new_v5($dnsNamespace, 'example.com'),
]);

report('newV6', [
    'ffi (Runtime::newV6)' => static fn () => Runtime::newV6(RFC_TEST_VECTOR_MS),
    'native (ext-php-rs)' => static fn () => hyperuuid_native_new_v6(RFC_TEST_VECTOR_MS),
]);

report('newV7', [
    'ffi (Runtime::newV7)' => static fn () => Runtime::newV7(RFC_TEST_VECTOR_MS),
    'native (ext-php-rs)' => static fn () => hyperuuid_native_new_v7(RFC_TEST_VECTOR_MS),
]);

report('newV6Batch(1000)', [
    'ffi (Runtime::newV6Batch)' => static fn () => Runtime::newV6Batch(1000, RFC_TEST_VECTOR_MS),
    'native (ext-php-rs)' => static fn () => hyperuuid_native_new_v6_batch(1000, RFC_TEST_VECTOR_MS),
], revs: 20, warmup: 5);

report('newV7Batch(1000)', [
    'ffi (Runtime::newV7Batch)' => static fn () => Runtime::newV7Batch(1000, RFC_TEST_VECTOR_MS),
    'native (ext-php-rs)' => static fn () => hyperuuid_native_new_v7_batch(1000, RFC_TEST_VECTOR_MS),
], revs: 20, warmup: 5);
