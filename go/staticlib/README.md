# staticlib/

The core as a static library, one per platform the cgo backend links it on:
`staticlib/{goos}_{goarch}/libhyperuuid.a` for `linux_amd64`, `linux_arm64`, `darwin_amd64`
and `darwin_arm64`. `backend_static.go` names the one for the build's platform on its cgo
link line, and that archive is everything a cgo build takes from this module — no embedded
shared libraries, nothing extracted at run time.

They are committed, for the reason the shared libraries under `native/` are: a `go get`
consumer has no packing step, so what is in the git tree at the resolved module version is
what gets linked. Only `stage-native-binaries.yml` commits them, after verifying each one's
build provenance, from the same CI run that built everything else; `.gitignore` lists them
so that a locally built copy cannot ride into a commit by accident.

The two Linux archives are the core built for the musl target and are linked on glibc and
musl alike. cgo has no build constraint that tells the two C libraries apart, so there
cannot be one archive per libc; of the two builds, the musl one is the one that asks the C
library for nothing both do not have. CI runs the suite against it on Debian and on Alpine.

## Building them yourself

`go test` under cgo needs the archive for your platform. On a checkout that has none — a
branch that has not been staged, or after changing `rust/` — build it with the forge's
script, from a checkout of [SkunkWerkx/.github](https://github.com/SkunkWerkx/.github)
beside this repository:

```shell
../.github/.github/actions/static-libs/build-static-libs.sh hyperuuid HyperUuid . x86_64-unknown-linux-musl
```

Name the Rust target for your platform (`aarch64-unknown-linux-musl`, `x86_64-apple-darwin`,
`aarch64-apple-darwin`); with no target it builds every one. A static library is compiled
and never linked, so all of them cross-compile from any machine with `rustup`.

Each archive is one object: the crate with everything it uses from Rust's `core` already
inside it, built without the standard library so that HyperUuid's and HyperCast's can both
be linked into one program.

## Verifying provenance

```shell
gh attestation verify staticlib/linux_amd64/libhyperuuid.a \
  --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
```

`--signer-repo` is required: the signing step is in the shared `SkunkWerkx/.github` forge
repo, so that is the identity recorded as the build signer. See `native/README.md` for the
longer explanation.
