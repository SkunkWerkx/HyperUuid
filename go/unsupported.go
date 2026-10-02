//go:build !(cgo && (darwin || linux || windows) && (amd64 || arm64))

// Every build backend_static.go does not cover lands here, and stops: CGO_ENABLED=0 (Go's
// default for a cross-compile, and whenever no C compiler is found), GOOS=wasip1 or js, and
// any platform there is no archive for. Go has no #error, so the stop is a reference to an
// identifier that does not exist, named to read as the explanation in the compiler's
// "undefined:" message. The alternative was a build that compiles and then fails every call
// at run time, which is what this module used to do on platforms it had no library for.
//
// Go compiled to WebAssembly cannot use this module at all: its toolchain links Go code
// only, with no cgo, so a foreign archive has nowhere to go. github.com/google/uuid is pure
// Go and builds there.

package hyperuuid

var _ = hyperuuid_needs_cgo_and_a_C_compiler_on_linux_darwin_or_windows_amd64_arm64__set_CGO_ENABLED_1__GOOS_wasip1_and_js_are_unsupported
