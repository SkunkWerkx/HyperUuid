# native/

Populated per-RID with the platform's native `libhyperuuid` build (`native/{rid}/{lib}`),
committed to git — unlike every other registry this repo publishes to (NuGet, Maven Central,
PyPI, crates.io), Packagist has no packing/build step of its own: whatever's literally in the
git tree at a tagged commit *is* the published package, so the native binaries have to live
here for real, not be staged in transiently by CI. Regenerate locally with
`cargo build --release` in `rust/` and copy the result in if you need to update one by hand;
CI's own `test-php` job does the same per-leg during in-repo testing, overwriting whichever
platform's file matches that leg — harmless, since it's the same build either way.
