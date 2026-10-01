package hyperuuid

import (
	"fmt"
	"sync"
)

var (
	initOnce sync.Once
	initErr  error

	// nativeVersion is the packed major<<16 | minor<<8 | patch the loaded core reported, set
	// by each backend's loadBackend as its final step — so a successful load has already
	// made one real call through the ABI, not merely resolved its symbols.
	nativeVersion uint32
)

// ensureLoaded loads whichever backend this build selected (loadBackend, one per backend
// file), exactly once, and caches the outcome: the name and signature are the same on all
// three backends so uuidgen.go needs no knowledge of which one it got.
func ensureLoaded() error {
	initOnce.Do(func() {
		initErr = loadFailure(loadBackend())
	})
	return initErr
}

// loadFailure is the one place a backend's load error becomes this package's public one:
// it wraps ErrNativeUnavailable around the reason, so errors.Is finds the sentinel and the
// message still says what actually went wrong. A nil stays nil.
func loadFailure(err error) error {
	if err == nil {
		return nil
	}
	return fmt.Errorf("%w: %w", ErrNativeUnavailable, err)
}

// Available reports whether the native library (or, under the hyperuuid_wasm tag, the
// wasm module) loaded and exports the ABI this binding was built against — every symbol
// resolved and hyperuuid_version answered. Probed once and cached; a false is permanent for
// the process. LoadError says why.
func Available() bool {
	return ensureLoaded() == nil
}

// LoadError returns nil when the native library loaded, and otherwise the reason it did
// not: the same error every other function in this package returns in that case, wrapping
// ErrNativeUnavailable. Probed once and cached, like Available.
func LoadError() error {
	return ensureLoaded()
}

// NativeVersion reports the loaded core's own version as "major.minor.patch" — read from
// the library itself, not from this module — so a deployment can name a mismatch between
// the binary it resolved and the one this binding was built against before minting its
// first UUID. Returns the LoadError if the native library could not be loaded.
func NativeVersion() (string, error) {
	if err := ensureLoaded(); err != nil {
		return "", err
	}
	v := nativeVersion
	return fmt.Sprintf("%d.%d.%d", v>>16, (v>>8)&0xFF, v&0xFF), nil
}
