//go:build cgo && (darwin || linux)

// This backend calls libhyperuuid through real cgo instead of purego — measured ~5x faster
// per call on this project's own benchmarks (see README.md) because a real C call avoids the
// purego call-trampoline's per-call heap allocations. It's only built on darwin/linux, and
// only when cgo itself is enabled:
//
//   - Windows stays on purego (backend_purego.go) unconditionally, even when CGO_ENABLED=1 —
//     a cgo build there needs a MinGW-class C toolchain, which has no arm64 support in the
//     mainline distribution (only llvm-mingw/MSYS2 clangarm64, neither bundled by default).
//     Windows Go servers are a small slice of this module's likely audience next to
//     Linux/macOS, so trading a real perf win there for one clean cross-platform story wasn't
//     worth it — see git history for the fuller version of that tradeoff.
//   - Any darwin/linux build with CGO_ENABLED=0 falls back to purego automatically — including,
//     per Go's own default behavior (confirmed empirically: `GOARCH=amd64 go env CGO_ENABLED`
//     on an arm64 host prints 0), any cross-compile to a non-native GOOS/GOARCH. A consumer
//     cross-compiling this module for linux/arm64 from an amd64 CI runner, or building inside
//     a QEMU-emulated container, gets purego with zero action on their part — cgo requires a
//     working C cross-compiler set via CC, which Go's toolchain won't assume is present.
//
// GitHub Actions' own ubuntu-latest and macos-latest images both ship a working native C
// toolchain by default (gcc and Xcode Command Line Tools' clang respectively — confirmed
// against actions/runner-images' own published tool manifests, not assumed), so this
// project's own CI exercises this backend for real on every darwin/linux leg, not just
// Windows via purego.
//
// Same embed-and-extract-to-a-temp-file loading strategy as the purego backend (see
// native_extract.go) — only the FFI mechanism differs: real dlopen/dlsym via cgo's own
// <dlfcn.h> binding, then a small per-signature C shim (cgo can't call an opaquely-typed
// void* function pointer directly — it needs a real, statically-typed C call site) instead of
// purego's dynamically-generated call trampoline.
package hyperuuid

/*
#include <dlfcn.h>
#include <stdint.h>
#include <stdlib.h>

typedef int32_t (*fn_new_v4)(uint8_t*);
typedef int32_t (*fn_new_v5)(const uint8_t*, const uint8_t*, uint32_t, uint8_t*);
typedef int32_t (*fn_new_v6_v7)(uint64_t, uint8_t*);
typedef uint64_t (*fn_unix_millis)(const uint8_t*);
typedef int32_t (*fn_new_batch)(uint64_t, uint32_t, uint8_t*);
typedef void (*fn_sql_order)(uint8_t*);

static int32_t call_new_v4(void *fn, uint8_t *out) {
	return ((fn_new_v4)fn)(out);
}
static int32_t call_new_v5(void *fn, const uint8_t *ns, const uint8_t *name, uint32_t name_len, uint8_t *out) {
	return ((fn_new_v5)fn)(ns, name, name_len, out);
}
static int32_t call_new_v6_v7(void *fn, uint64_t unix_millis, uint8_t *out) {
	return ((fn_new_v6_v7)fn)(unix_millis, out);
}
static uint64_t call_unix_millis(void *fn, const uint8_t *uuid) {
	return ((fn_unix_millis)fn)(uuid);
}
static int32_t call_new_batch(void *fn, uint64_t unix_millis, uint32_t count, uint8_t *out) {
	return ((fn_new_batch)fn)(unix_millis, count, out);
}
static void call_sql_order(void *fn, uint8_t *uuid) {
	((fn_sql_order)fn)(uuid);
}
*/
import "C"

import (
	"fmt"
	"sync"
	"unsafe"
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

// dlsymOrErr looks up name in the already-dlopen'd handle, wrapping dlerror()'s C string into
// a Go error on failure.
func dlsymOrErr(handle unsafe.Pointer, name string) (unsafe.Pointer, error) {
	cName := C.CString(name)
	defer C.free(unsafe.Pointer(cName))
	sym := C.dlsym(handle, cName)
	if sym == nil {
		return nil, fmt.Errorf("hyperuuid: symbol %s not found in native library: %s", name, C.GoString(C.dlerror()))
	}
	return sym, nil
}

// ensureLoaded extracts this platform's embedded native library to a temp file and dlopen's
// it via cgo, exactly once.
func ensureLoaded() error {
	initOnce.Do(func() {
		path, err := extractNativeLib()
		if err != nil {
			initErr = err
			return
		}

		cPath := C.CString(path)
		defer C.free(unsafe.Pointer(cPath))
		handle := C.dlopen(cPath, C.RTLD_NOW|C.RTLD_GLOBAL)
		if handle == nil {
			initErr = fmt.Errorf("hyperuuid: dlopen failed: %s", C.GoString(C.dlerror()))
			return
		}

		sym := func(name string) unsafe.Pointer {
			if initErr != nil {
				return nil
			}
			p, symErr := dlsymOrErr(handle, name)
			if symErr != nil {
				initErr = symErr
				return nil
			}
			return p
		}

		newV4 := sym("uuid_new_v4")
		newV5 := sym("uuid_new_v5")
		newV6 := sym("uuid_new_v6")
		v6UnixMillis := sym("uuid_v6_unix_millis")
		newV6Batch := sym("uuid_new_v6_batch")
		newV7 := sym("uuid_new_v7")
		v7UnixMillis := sym("uuid_v7_unix_millis")
		newV7Batch := sym("uuid_new_v7_batch")
		v7ToSql := sym("uuid_v7_to_sql_order")
		v7ToRfc := sym("uuid_v7_to_rfc_order")
		v6ToSql := sym("uuid_v6_to_sql_order")
		v6ToRfc := sym("uuid_v6_to_rfc_order")
		if initErr != nil {
			return
		}

		uuidNewV4 = func(out unsafe.Pointer) int32 {
			return int32(C.call_new_v4(newV4, (*C.uint8_t)(out)))
		}
		uuidNewV5 = func(ns, name unsafe.Pointer, nameLen uint32, out unsafe.Pointer) int32 {
			return int32(C.call_new_v5(newV5, (*C.uint8_t)(ns), (*C.uint8_t)(name), C.uint32_t(nameLen), (*C.uint8_t)(out)))
		}
		uuidNewV6 = func(unixMillis uint64, out unsafe.Pointer) int32 {
			return int32(C.call_new_v6_v7(newV6, C.uint64_t(unixMillis), (*C.uint8_t)(out)))
		}
		uuidV6UnixMillis = func(id unsafe.Pointer) uint64 {
			return uint64(C.call_unix_millis(v6UnixMillis, (*C.uint8_t)(id)))
		}
		uuidNewV6Batch = func(unixMillis uint64, count uint32, out unsafe.Pointer) int32 {
			return int32(C.call_new_batch(newV6Batch, C.uint64_t(unixMillis), C.uint32_t(count), (*C.uint8_t)(out)))
		}
		uuidNewV7 = func(unixMillis uint64, out unsafe.Pointer) int32 {
			return int32(C.call_new_v6_v7(newV7, C.uint64_t(unixMillis), (*C.uint8_t)(out)))
		}
		uuidV7UnixMillis = func(id unsafe.Pointer) uint64 {
			return uint64(C.call_unix_millis(v7UnixMillis, (*C.uint8_t)(id)))
		}
		uuidNewV7Batch = func(unixMillis uint64, count uint32, out unsafe.Pointer) int32 {
			return int32(C.call_new_batch(newV7Batch, C.uint64_t(unixMillis), C.uint32_t(count), (*C.uint8_t)(out)))
		}
		uuidV7ToSqlOrder = func(id unsafe.Pointer) {
			C.call_sql_order(v7ToSql, (*C.uint8_t)(id))
		}
		uuidV7ToRfcOrder = func(id unsafe.Pointer) {
			C.call_sql_order(v7ToRfc, (*C.uint8_t)(id))
		}
		uuidV6ToSqlOrder = func(id unsafe.Pointer) {
			C.call_sql_order(v6ToSql, (*C.uint8_t)(id))
		}
		uuidV6ToRfcOrder = func(id unsafe.Pointer) {
			C.call_sql_order(v6ToRfc, (*C.uint8_t)(id))
		}
	})
	return initErr
}
