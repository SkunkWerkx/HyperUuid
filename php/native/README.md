# hyperuuid-php-native (spike)

A `ext-php-rs` Zend extension linking the `hyperuuid` Rust core directly into PHP, mirroring
the Python (PyO3) / Ruby (Magnus) native-backend pattern. Built to test whether it beats
`../src/Runtime.php`'s `ext-ffi` path — see [`../README.md`'s ext-php-rs spike
section](../README.md#ext-php-rs-spike-not-shipped) for the numbers.

**Not wired into the shipped `skunkwerkx/hyperuuid` Composer package.** No prebuilt binary is
bundled, no CI leg builds it, `HyperUuid.php` has no fallback-detection for it. Composer also
has no mechanism to deliver a compiled Zend extension the way it delivers the FFI `.so` — a
`php.ini` `extension=` entry has to exist before PHP starts, so this can't be dlopen'd at
runtime the way `Runtime.php` dlopens `libhyperuuid`. This crate exists so the approach and
its measured numbers are on record for whoever wants to pursue real packaging later, not as a
second production backend.

Mirrors `Runtime.php`'s raw-bytes function shapes exactly (16-byte binary strings in/out, no
`Uuid` value-object construction) so the benchmark isolates the crossing mechanism, not
wrapper cost. Function names match the FFI core's C exports 1:1
(`hyperuuid_native_new_v4`, `hyperuuid_native_new_v5`, ...).

## Build and try it

```shell
cargo build --release
php -d extension=$(pwd)/target/release/libhyperuuid_php_native.so -r 'echo bin2hex(hyperuuid_native_new_v4()), "\n";'
```

## Reproduce the benchmark

```shell
(cd .. && composer install)  # php/ — bench_compare.php requires its vendor/autoload.php
XDEBUG_MODE=off php -d extension=$(pwd)/target/release/libhyperuuid_php_native.so bench_compare.php
```
