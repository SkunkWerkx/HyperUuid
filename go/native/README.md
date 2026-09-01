# native/

Populated per-RID with the platform's native `libhyperuuid` build (`native/{rid}/{lib}`),
committed to git — unlike NuGet/Maven Central/PyPI/crates.io, a `go get`/`go build` consumer
has no packing step of its own: `//go:embed native` (in `embed.go`) embeds whatever's
literally in the git tree at the resolved module version, so the native binaries have to live
here for real, not be staged in transiently by CI (the same real bug found and fixed for the
PHP/Swift bindings' own package managers — see `php/src/native/README.md`). Regenerate
locally with `cargo build --release` in `rust/` and copy the result in if you need to update
one by hand; CI's own `build-native` job does the same per-leg during in-repo testing, overwriting
whichever platform's file matches that leg — harmless, since it's the same build either way.

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
`verifying with issuer "sigstore.dev"`, which looks like a bad signature but is really a
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
