// Package hyperuuid provides RFC 9562 UUID generation (v4 random, v5 deterministic, v7
// time-sortable) calling directly into the native libhyperuuid shared library via
// github.com/ebitengine/purego — dlopen/dlsym plus per-arch call trampolines, no cgo and no C
// compiler required to build or consume this module (the same "no runtime bridge" positioning
// as the Python/ctypes and Kotlin/FFM bindings).
//
// This module bundles a native build for every supported platform (see currentTarget) and
// loads the right one at runtime, the same trick the Kotlin binding uses since neither a Go
// module nor a .jar has NuGet-style per-RID package selection.
package hyperuuid

import (
	"fmt"
	"os"
	"sync"
	"time"
	"unsafe"

	"github.com/ebitengine/purego"
	"github.com/google/uuid"
)

// Well-known namespace UUIDs defined in RFC 9562 Section 6.6. These are byte-identical to
// google/uuid's own namespace constants, so they're just re-exported for API-shape symmetry
// with the other bindings' Namespaces.* members.
var (
	NamespaceDNS  = uuid.NameSpaceDNS
	NamespaceURL  = uuid.NameSpaceURL
	NamespaceOID  = uuid.NameSpaceOID
	NamespaceX500 = uuid.NameSpaceX500
)

var (
	initOnce sync.Once
	initErr  error

	uuidNewV4 func(out unsafe.Pointer) int32
	uuidNewV5 func(ns, name unsafe.Pointer, nameLen uint32, out unsafe.Pointer) int32
	uuidNewV7 func(unixMillis uint64, out unsafe.Pointer) int32
)

// ensureLoaded extracts this platform's embedded native library to a temp file and dlopen's
// it, exactly once. The temp file is deliberately never removed afterward — Go has no
// reliable process-exit hook, the same best-effort tradeoff the Kotlin binding makes with
// File.deleteOnExit (itself not guaranteed, e.g. on kill -9).
func ensureLoaded() error {
	initOnce.Do(func() {
		t, err := currentTarget()
		if err != nil {
			initErr = err
			return
		}

		resourcePath := "native/" + t.rid + "/" + t.libName
		data, err := nativeFS.ReadFile(resourcePath)
		if err != nil {
			initErr = fmt.Errorf("hyperuuid: %s not found in embedded native libs (unsupported platform, or this module was built without a native library for it): %w", resourcePath, err)
			return
		}

		tmp, err := os.CreateTemp("", "libhyperuuid-*-"+t.libName)
		if err != nil {
			initErr = fmt.Errorf("hyperuuid: creating temp file for native library: %w", err)
			return
		}
		defer tmp.Close()
		if _, err := tmp.Write(data); err != nil {
			initErr = fmt.Errorf("hyperuuid: writing native library to temp file: %w", err)
			return
		}

		handle, err := purego.Dlopen(tmp.Name(), purego.RTLD_NOW|purego.RTLD_GLOBAL)
		if err != nil {
			initErr = fmt.Errorf("hyperuuid: loading native library: %w", err)
			return
		}

		purego.RegisterLibFunc(&uuidNewV4, handle, "uuid_new_v4")
		purego.RegisterLibFunc(&uuidNewV5, handle, "uuid_new_v5")
		purego.RegisterLibFunc(&uuidNewV7, handle, "uuid_new_v7")
	})
	return initErr
}

// NewV4 creates a random UUID version 4 (RFC 9562 §5.4).
func NewV4() (uuid.UUID, error) {
	if err := ensureLoaded(); err != nil {
		return uuid.UUID{}, err
	}
	var out uuid.UUID
	if rc := uuidNewV4(unsafe.Pointer(&out[0])); rc != 0 {
		return uuid.UUID{}, fmt.Errorf("uuid_new_v4 failed with code %d: %w", rc, ErrRandomSource)
	}
	return out, nil
}

// NewV5 creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and raw name
// bytes. The same (namespace, name) pair always produces the same UUID.
func NewV5(namespace uuid.UUID, name []byte) (uuid.UUID, error) {
	if err := ensureLoaded(); err != nil {
		return uuid.UUID{}, err
	}
	var out uuid.UUID
	var namePtr unsafe.Pointer
	if len(name) > 0 {
		namePtr = unsafe.Pointer(&name[0])
	}
	if rc := uuidNewV5(unsafe.Pointer(&namespace[0]), namePtr, uint32(len(name)), unsafe.Pointer(&out[0])); rc != 0 {
		return uuid.UUID{}, fmt.Errorf("uuid_new_v5 failed with code %d: %w", rc, ErrRandomSource)
	}
	return out, nil
}

// NewV5String creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a
// UTF-8 name.
func NewV5String(namespace uuid.UUID, name string) (uuid.UUID, error) {
	return NewV5(namespace, []byte(name))
}

// NewV7 creates a time-sortable UUID version 7 (RFC 9562 §6.2) using the current time.
func NewV7() (uuid.UUID, error) {
	return NewV7At(uint64(time.Now().UnixMilli()))
}

// NewV7At creates a time-sortable UUID version 7 (RFC 9562 §6.2), embedding unixMillis
// (milliseconds since the Unix epoch).
func NewV7At(unixMillis uint64) (uuid.UUID, error) {
	if err := ensureLoaded(); err != nil {
		return uuid.UUID{}, err
	}
	var out uuid.UUID
	switch rc := uuidNewV7(unixMillis, unsafe.Pointer(&out[0])); rc {
	case 0:
		return out, nil
	case 2:
		return uuid.UUID{}, fmt.Errorf("uuid_new_v7 failed with code %d: %w", rc, ErrTimestampOutOfRange)
	default:
		return uuid.UUID{}, fmt.Errorf("uuid_new_v7 failed with code %d: %w", rc, ErrRandomSource)
	}
}
