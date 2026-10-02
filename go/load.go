package hyperuuid

import "fmt"

// Available reports whether the core can be called. The core is linked into the binary
// (backend_static.go), so there is nothing to load and nothing that can fail: it is always
// true. It stays for code written against the load probe every binding carries, and so a
// caller can keep gating on it as it does on HyperCast's.
func Available() bool {
	return true
}

// LoadError returns why the core could not be loaded. A linked core always can, so it is
// always nil. Kept for the reason Available is.
func LoadError() error {
	return nil
}

// NativeVersion reports the linked core's own version as "major.minor.patch" — read from the
// core itself, not from this module — so a deployment can confirm which build of the archive
// went into the binary. The error is always nil; it stays in the signature so code written
// against the load probe keeps compiling.
func NativeVersion() (string, error) {
	v := packedVersion()
	return fmt.Sprintf("%d.%d.%d", v>>16, (v>>8)&0xFF, v&0xFF), nil
}
