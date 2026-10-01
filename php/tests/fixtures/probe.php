<?php

declare(strict_types=1);

// Child-process probe for the availability tests. argv[1] is the src directory to load the
// binding from, so a test can point it at a copy with no native library beside it; the php
// flags the test passes (-n, -d ffi.enable=0) do the rest. Prints "available" or
// "unavailable", then whether ext-ffi was loaded at all — and nothing else, since
// isAvailable() must never throw.

$src = $argv[1];
spl_autoload_register(static function (string $class) use ($src): void {
    if (str_starts_with($class, 'HyperUuid\\')) {
        $file = $src . '/' . substr($class, \strlen('HyperUuid\\')) . '.php';
        if (is_file($file)) {
            require $file;
        }
    }
});

echo \HyperUuid\HyperUuid::isAvailable() ? 'available' : 'unavailable';
echo extension_loaded('ffi') ? ' ffi' : ' no-ffi';
