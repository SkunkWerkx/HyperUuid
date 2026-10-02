//go:build !(cgo && (darwin || linux) && (amd64 || arm64) && !hyperuuid_wasm && !hyperuuid_dynamic)

package hyperuuid

import (
	"fmt"
	"os"
)

// extractNativeLib copies this platform's embedded native library to a temp file and returns
// its path, ready for either backend (purego's Dlopen/LoadLibrary or cgo's dlopen) to load.
// The temp file is deliberately never removed afterward — Go has no reliable process-exit
// hook, the same best-effort tradeoff the Java binding makes with File.deleteOnExit (itself
// not guaranteed, e.g. on kill -9).
func extractNativeLib() (string, error) {
	t, err := currentTarget()
	if err != nil {
		return "", err
	}

	resourcePath := "native/" + t.rid + "/" + t.libName
	data, err := nativeFS.ReadFile(resourcePath)
	if err != nil {
		// A supported platform whose directory simply isn't in this version of the module (a
		// linux-musl-* build, in a version from before those were staged) lands here, with the
		// missing path named, rather than on a dlopen error.
		return "", fmt.Errorf("%s not found in embedded native libs (this version of the module carries no native library for %s): %w", resourcePath, t.rid, err)
	}

	tmp, err := os.CreateTemp("", "libhyperuuid-*-"+t.libName)
	if err != nil {
		return "", fmt.Errorf("creating temp file for native library: %w", err)
	}
	_, writeErr := tmp.Write(data)
	closeErr := tmp.Close()
	// The write handle must be closed before the loader (dlopen/LoadLibrary) opens the same
	// path — Windows enforces exclusive file access far more strictly than Unix, and
	// LoadLibrary fails outright while a write handle on the same file is still open.
	if writeErr != nil {
		return "", fmt.Errorf("writing native library to temp file: %w", writeErr)
	}
	if closeErr != nil {
		return "", fmt.Errorf("closing temp file for native library: %w", closeErr)
	}

	return tmp.Name(), nil
}
