//go:build !((cgo && !tinygo && (darwin || linux || windows) && (amd64 || arm64) && !(ios && amd64 && !maccatalyst)) || tinygo.wasm)

// Every build neither backend covers lands here, and stops: CGO_ENABLED=0 (Go's default for
// a cross-compile, and whenever no C compiler is found), stock Go's GOOS=wasip1 and js, TinyGo
// on anything but WebAssembly, and any platform there is no archive for (the iOS simulator
// on amd64 among them, which the darwin constraint would otherwise let through). Go has no #error, so
// the stop is a reference to an identifier that does not exist, named to read as the
// explanation in the compiler's "undefined:" message. The alternative was a build that
// compiles and then fails every call at run time, which is what this module used to do on
// platforms it had no library for.
//
// Stock Go compiled to WebAssembly cannot use this module: its toolchain links Go code only,
// with no cgo, so a foreign archive has nowhere to go. TinyGo can — it links with wasm-ld and
// has cgo — and backend_tinygo.go is that build. github.com/google/uuid is pure Go and builds
// under either.

package hyperuuid

var _ = hyperuuid_needs_cgo_and_a_C_compiler_on_linux_darwin_or_windows_amd64_arm64__set_CGO_ENABLED_1__for_wasm_build_with_TinyGo
