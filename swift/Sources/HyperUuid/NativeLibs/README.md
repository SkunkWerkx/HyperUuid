# NativeLibs/

Populated per-RID with the platform's native `libhyperuuid` build (`NativeLibs/{rid}/{lib}`),
committed to git — unlike NuGet/Maven Central/PyPI/crates.io, SwiftPM has no packing/build
step of its own for a git-URL dependency: whatever's literally in the git tree at a tagged
commit *is* what a real `.package(url:, from:)` consumer gets, so the native binaries have to
live here for real, not be staged in transiently by CI (the same real bug found and fixed for
the PHP binding's Composer/Packagist package — see `php/src/native/README.md`). Regenerate
locally with `cargo build --release` in `rust/` and copy the result in if you need to update
one by hand; CI's own `test-swift` job does the same per-leg during in-repo testing,
overwriting whichever platform's file matches that leg — harmless, since it's the same build
either way.
