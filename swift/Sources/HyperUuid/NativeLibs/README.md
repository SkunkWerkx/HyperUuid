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

## Verifying provenance

These are compiled binaries committed to git, which is the least inspectable thing in this
repository — you cannot read a diff of them. So they carry
[SLSA build provenance](https://github.com/actions/attest-build-provenance): every one is
signed as it is built, and `stage-native-binaries.yml` verifies that signature *before* it is
allowed to commit the file, so a binary reaching this directory has already had its origin
checked. The staging commit records each file's SHA-256 in its own message.

Verify any of them yourself, against GitHub's transparency log, without trusting this
repository or whoever handed you a copy:

```shell
gh attestation verify linux-arm64/libhyperuuid.so --repo SkunkWerkx/HyperUuid
```

That reports the exact commit and workflow run the binary was built from. Verification is by
content digest, so it holds for these committed copies even though they were produced as CI
artifacts — the bytes are identical. The same set of binaries is committed under `go/native/`,
`php/src/native/` and `swift/Sources/HyperUuid/NativeLibs/`; git stores each one as a single
shared blob, so the three copies cost no extra repository space, and one attestation covers
all three.

If you would rather not trust a binary at all, build the core from source instead — it is a
plain Rust crate with no build-time codegen:

```shell
cd rust && cargo build --release
```
