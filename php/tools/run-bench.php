<?php

declare(strict_types=1);

// `composer bench`: runs PHPBench with the extension loaded (tools/extension.php says which).
// PHPBench times each iteration in a PHP process of its own, which takes no -d flag from this
// one, so the extension reaches those processes through --php-config. Arguments pass through:
// `composer bench -- --report=aggregate`.

require __DIR__ . '/extension.php';

$root = \dirname(__DIR__);
$path = hyperuuid_extension($root);
$config = json_encode(['extension' => $path], JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES);
hyperuuid_run(
    [PHP_BINARY, $root . '/vendor/bin/phpbench', 'run', '--php-config=' . $config, ...\array_slice($argv, 1)],
    $root,
    'phpbench',
);
