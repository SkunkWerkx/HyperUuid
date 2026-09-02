//go:build !(cgo && (darwin || linux))

// This backend calls libhyperuuid through github.com/ebitengine/purego — dlopen/dlsym plus
// per-arch call trampolines, no cgo and no C compiler required. It's the only backend on
// Windows (see backend_cgo.go's doc comment for why) and the fallback everywhere else: any
// darwin/linux build with CGO_ENABLED=0 — including, per Go's own default, any cross-compile
// to a non-native GOOS/GOARCH — lands here automatically, with no action needed from a
// consumer of this module.
package hyperuuid

import (
	"fmt"
	"sync"
	"unsafe"

	"github.com/ebitengine/purego"
	"github.com/google/uuid"
)

var (
	initOnce sync.Once
	initErr  error

	uuidNewV4        func(out unsafe.Pointer) int32
	uuidNewV5        func(ns, name unsafe.Pointer, nameLen uint32, out unsafe.Pointer) int32
	uuidNewV6        func(unixMillis uint64, out unsafe.Pointer) int32
	uuidV6UnixMillis func(uuid unsafe.Pointer) uint64
	uuidNewV6Batch   func(unixMillis uint64, count uint32, out unsafe.Pointer) int32
	uuidNewV7        func(unixMillis uint64, out unsafe.Pointer) int32
	uuidV7UnixMillis func(uuid unsafe.Pointer) uint64
	uuidNewV7Batch   func(unixMillis uint64, count uint32, out unsafe.Pointer) int32
	uuidV7ToSqlOrder func(uuid unsafe.Pointer)
	uuidV7ToRfcOrder func(uuid unsafe.Pointer)
	uuidV6ToSqlOrder func(uuid unsafe.Pointer)
	uuidV6ToRfcOrder func(uuid unsafe.Pointer)
)

// ensureLoaded extracts this platform's embedded native library to a temp file and dlopen's
// it, exactly once.
func ensureLoaded() error {
	initOnce.Do(func() {
		path, err := extractNativeLib()
		if err != nil {
			initErr = err
			return
		}

		handle, err := openLibrary(path)
		if err != nil {
			initErr = fmt.Errorf("hyperuuid: loading native library: %w", err)
			return
		}

		purego.RegisterLibFunc(&uuidNewV4, handle, "uuid_new_v4")
		purego.RegisterLibFunc(&uuidNewV5, handle, "uuid_new_v5")
		purego.RegisterLibFunc(&uuidNewV6, handle, "uuid_new_v6")
		purego.RegisterLibFunc(&uuidV6UnixMillis, handle, "uuid_v6_unix_millis")
		purego.RegisterLibFunc(&uuidNewV6Batch, handle, "uuid_new_v6_batch")
		purego.RegisterLibFunc(&uuidNewV7, handle, "uuid_new_v7")
		purego.RegisterLibFunc(&uuidV7UnixMillis, handle, "uuid_v7_unix_millis")
		purego.RegisterLibFunc(&uuidNewV7Batch, handle, "uuid_new_v7_batch")
		purego.RegisterLibFunc(&uuidV7ToSqlOrder, handle, "uuid_v7_to_sql_order")
		purego.RegisterLibFunc(&uuidV7ToRfcOrder, handle, "uuid_v7_to_rfc_order")
		purego.RegisterLibFunc(&uuidV6ToSqlOrder, handle, "uuid_v6_to_sql_order")
		purego.RegisterLibFunc(&uuidV6ToRfcOrder, handle, "uuid_v6_to_rfc_order")
	})
	return initErr
}

// The same by-value doors the cgo backend exposes, filled through pointers here: purego's
// trampoline boxes its arguments per call regardless, so the heap escape the cgo shims
// exist to avoid is not the cost that dominates this backend.
func newV4() (uuid.UUID, int32) {
	var out uuid.UUID
	rc := uuidNewV4(unsafe.Pointer(&out[0]))
	return out, rc
}

func newV5(ns uuid.UUID, name []byte) (uuid.UUID, int32) {
	var out uuid.UUID
	var namePtr unsafe.Pointer
	if len(name) > 0 {
		namePtr = unsafe.Pointer(&name[0])
	}
	rc := uuidNewV5(unsafe.Pointer(&ns[0]), namePtr, uint32(len(name)), unsafe.Pointer(&out[0]))
	return out, rc
}

func newV6(unixMillis uint64) (uuid.UUID, int32) {
	var out uuid.UUID
	rc := uuidNewV6(unixMillis, unsafe.Pointer(&out[0]))
	return out, rc
}

func newV7(unixMillis uint64) (uuid.UUID, int32) {
	var out uuid.UUID
	rc := uuidNewV7(unixMillis, unsafe.Pointer(&out[0]))
	return out, rc
}

func v6UnixMillis(id uuid.UUID) uint64 { return uuidV6UnixMillis(unsafe.Pointer(&id[0])) }
func v7UnixMillis(id uuid.UUID) uint64 { return uuidV7UnixMillis(unsafe.Pointer(&id[0])) }

func newV6Batch(unixMillis uint64, count uint32, out unsafe.Pointer) int32 {
	return uuidNewV6Batch(unixMillis, count, out)
}

func newV7Batch(unixMillis uint64, count uint32, out unsafe.Pointer) int32 {
	return uuidNewV7Batch(unixMillis, count, out)
}

func v7ToSqlOrder(id uuid.UUID) uuid.UUID { uuidV7ToSqlOrder(unsafe.Pointer(&id[0])); return id }
func v7ToRfcOrder(id uuid.UUID) uuid.UUID { uuidV7ToRfcOrder(unsafe.Pointer(&id[0])); return id }
func v6ToSqlOrder(id uuid.UUID) uuid.UUID { uuidV6ToSqlOrder(unsafe.Pointer(&id[0])); return id }
func v6ToRfcOrder(id uuid.UUID) uuid.UUID { uuidV6ToRfcOrder(unsafe.Pointer(&id[0])); return id }

func v7ToSqlOrderBytes(p unsafe.Pointer) { uuidV7ToSqlOrder(p) }
func v7ToRfcOrderBytes(p unsafe.Pointer) { uuidV7ToRfcOrder(p) }
func v6ToSqlOrderBytes(p unsafe.Pointer) { uuidV6ToSqlOrder(p) }
func v6ToRfcOrderBytes(p unsafe.Pointer) { uuidV6ToRfcOrder(p) }
