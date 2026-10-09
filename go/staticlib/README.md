# staticlib/

The core as a static library, one per platform the module links it on:
`staticlib/{goos}_{goarch}/libhyperuuid.a` for `linux_amd64`, `linux_arm64`, `darwin_amd64`,
`darwin_arm64`, `windows_amd64` and `windows_arm64`, plus `staticlib/wasm/libhyperuuid.a`
for TinyGo on WebAssembly, four that build as `GOOS=ios` and are chosen by build tag:
`ios_arm64`, `iossimulator_arm64`, `maccatalyst_arm64` and `maccatalyst_amd64`, and two
for `GOOS=android`: `android_arm64` and `android_amd64`. `backend_static.go` names the one for the build's platform on its
cgo link line (`backend_tinygo.go` names `wasm` under TinyGo), and that archive is
everything a build takes from this module — no shared libraries, nothing loaded or
extracted at run time.

They are committed because a `go get` consumer has no packing step, so what is in the git
tree at the resolved module version is what gets linked. Only `stage-native-binaries.yml` commits them, after verifying each one's
build provenance, from the same CI run that built everything else; `.gitignore` lists them
so that a locally built copy cannot ride into a commit by accident.

The two Linux archives are the core built for the musl target and are linked on glibc and
musl alike. cgo has no build constraint that tells the two C libraries apart, so there
cannot be one archive per libc; of the two builds, the musl one is the one that asks the C
library for nothing both do not have. CI runs the suite against it on Debian and on Alpine.

The two Windows archives are the MSVC builds (`x86_64-pc-windows-msvc`,
`aarch64-pc-windows-msvc`), byte for byte the ones the C# package links under Native AOT,
renamed to the `libhyperuuid.a` every cgo line here uses. MinGW's linker reads MSVC's COFF
objects, and the archive carries its own import stub for `ProcessPrng`, Windows' entropy
source, so nothing else goes on the link line.

The `wasm` archive is the `wasm32-wasip1` build, byte for byte the one Swift's artifact
bundle links for WebAssembly. TinyGo's browser target is a wasm32-wasi build underneath, so
it links unchanged there too, and TinyGo's `wasm_exec.js` supplies the archive's one import,
WASI's `random_get`, from `crypto.getRandomValues`.

The four Apple mobile archives are the `aarch64-apple-ios`, `aarch64-apple-ios-sim`,
`aarch64-apple-ios-macabi` and `x86_64-apple-ios-macabi` builds, the ones the C# package
links for iOS and Mac Catalyst and (the arm64 three) the Swift XCFramework carries. They are
separate files because a Mach-O object records the platform it was built for and the linker
refuses any other; the module's README has the tags that select each.

The two Android archives are the `aarch64-linux-android` and `x86_64-linux-android` builds,
byte for byte the ones the C# package links under Native AOT on Android and Swift's artifact
bundle carries for the Swift SDK for Android. They are separate from the Linux pair because
`GOOS=android` links against Bionic through the NDK's clang, and an archive built for musl
asks for symbols under names Bionic does not promise.

## Building them yourself

`go test` needs the archive for your platform. On a checkout that has none — a branch that
has not been staged, or after changing `rust/` — build it with the forge's script, from a
checkout of [SkunkWerkx/.github](https://github.com/SkunkWerkx/.github) beside this
repository:

```shell
../.github/.github/actions/static-libs/build-static-libs.sh hyperuuid HyperUuid . x86_64-unknown-linux-musl
```

Name the Rust target for your platform (`aarch64-unknown-linux-musl`, `x86_64-apple-darwin`,
`aarch64-apple-darwin`, `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`,
`aarch64-linux-android`, `x86_64-linux-android`, or `wasm32-wasip1` for TinyGo); with no
target
it builds every one. A static library is compiled
and never linked, so all of them cross-compile from any machine with `rustup`.

Each archive is the crate's one object, with everything it uses from Rust's `core` already
inside it — plus, on Windows, the few runtime-helper objects and the import stub a linker
would have pulled, since MSVC supplies no 128-bit helpers — built without the standard library so that HyperUuid's and HyperCast's can both
be linked into one program.

## Verifying provenance

```shell
gh attestation verify staticlib/linux_amd64/libhyperuuid.a \
  --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
```

`--signer-repo` is required: the signing step is in the shared `SkunkWerkx/.github` forge
repo, so that is the identity recorded as the build signer. See
[the Go README](../README.md#verifying-build-provenance) for the longer explanation.
