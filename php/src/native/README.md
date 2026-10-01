# native/

Populated per-RID with the platform's native `libhyperuuid` build (`native/{rid}/{lib}`),
committed to git — unlike every other registry this repo publishes to (NuGet, Maven Central,
PyPI, crates.io), Packagist has no packing/build step of its own: whatever's literally in the
git tree at a tagged commit *is* the published package, so the native binaries have to live
here for real, not be staged in transiently by CI. Regenerate locally with
`cargo build --release` in `rust/` and copy the result in if you need to update one by hand;
CI's own `build-native` job does the same per-leg during in-repo testing, overwriting whichever
platform's file matches that leg — harmless, since it's the same build either way. When a
platform's file is absent the binding falls back to `rust/target/release/`, so a development
checkout can also just delete the stale one.

## Which library loads

`NativePlatform.php` picks the directory from the running process:

| RID | Platform |
| --- | --- |
| `linux-x64`, `linux-arm64` | Linux on glibc 2.34 or newer |
| `linux-musl-x64`, `linux-musl-arm64` | Linux on musl (Alpine) — detected by a musl loader in `/proc/self/maps` |
| `osx-x64`, `osx-arm64` | macOS |
| `win-x64` | Windows, including ARM hardware |

There is no `win-arm64` for PHP by design: PHP has never shipped a native Windows ARM64
build, so on ARM hardware it is an x64 process under emulation and can only load the x64
DLL. A 32-bit PHP, any other architecture and any other OS family are an
unsupported-platform error.

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
gh attestation verify linux-arm64/libhyperuuid.so \
  --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
```

`--signer-repo` is required, not decoration. `--repo` on its own asserts two things at
once: that the artifact came from that repo, and that the workflow which signed it lives
there. Only the first is true here — the signing step is in `hyper-build-native.yml`,
which lives in the shared `SkunkWerkx/.github` forge repo, so that is what Fulcio records
as the build signer. Omit the flag and verification fails with an unhelpful
`verifying with issuer "sigstore.dev"`, which looks like a bad signature but is really an
identity mismatch.

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
