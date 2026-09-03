//go:build cgo && (darwin || linux) && !hyperuuid_wasm

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

// A UUID crosses BY VALUE in both directions. Every single-UUID door used to hand the
// core `&out[0]` of a Go local, and any Go pointer passed to a cgo call escapes to the
// heap — one allocation per NewV4/NewV6/NewV7 that the README once called a floor for
// the call shape. It was a floor for pointer-passing, not for the ABI: the shims below
// keep the 16 bytes on the C stack and return them as a struct, so no Go pointer crosses
// for anything but a caller's own slice (the v5 name, a batch destination).
typedef struct { uint8_t b[16]; } hc_uuid;
typedef struct { hc_uuid id; int32_t code; } hc_result;

static hc_result call_new_v4(void *fn) {
	hc_result r;
	r.code = ((fn_new_v4)fn)(r.id.b);
	return r;
}
static hc_result call_new_v5(void *fn, hc_uuid ns, const uint8_t *name, uint32_t name_len) {
	hc_result r;
	r.code = ((fn_new_v5)fn)(ns.b, name, name_len, r.id.b);
	return r;
}
static hc_result call_new_v6_v7(void *fn, uint64_t unix_millis) {
	hc_result r;
	r.code = ((fn_new_v6_v7)fn)(unix_millis, r.id.b);
	return r;
}
static uint64_t call_unix_millis(void *fn, hc_uuid uuid) {
	return ((fn_unix_millis)fn)(uuid.b);
}
static int32_t call_new_batch(void *fn, uint64_t unix_millis, uint32_t count, uint8_t *out) {
	return ((fn_new_batch)fn)(unix_millis, count, out);
}
static hc_uuid call_sql_order(void *fn, hc_uuid uuid) {
	((fn_sql_order)fn)(uuid.b);
	return uuid;
}
static void call_sql_order_bytes(void *fn, uint8_t *uuid) {
	((fn_sql_order)fn)(uuid);
}
*/
import "C"

import (
	"fmt"
	"sync"
	"unsafe"

	"github.com/google/uuid"
)

var (
	initOnce sync.Once
	initErr  error

	symNewV4, symNewV5, symNewV6, symNewV7, symV6UnixMillis, symV7UnixMillis,
	symNewV6Batch, symNewV7Batch, symV7ToSql, symV7ToRfc, symV6ToSql, symV6ToRfc unsafe.Pointer
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

		symNewV4, symNewV5, symNewV6, symNewV7 = newV4, newV5, newV6, newV7
		symV6UnixMillis, symV7UnixMillis = v6UnixMillis, v7UnixMillis
		symNewV6Batch, symNewV7Batch = newV6Batch, newV7Batch
		symV7ToSql, symV7ToRfc, symV6ToSql, symV6ToRfc = v7ToSql, v7ToRfc, v6ToSql, v6ToRfc
	})
	return initErr
}

func toGo(r C.hc_result) (uuid.UUID, int32) {
	return *(*uuid.UUID)(unsafe.Pointer(&r.id)), int32(r.code)
}

func toC(id uuid.UUID) C.hc_uuid {
	return *(*C.hc_uuid)(unsafe.Pointer(&id))
}

func newV4() (uuid.UUID, int32) { return toGo(C.call_new_v4(symNewV4)) }

func newV5(ns uuid.UUID, name []byte) (uuid.UUID, int32) {
	var namePtr *C.uint8_t
	if len(name) > 0 {
		namePtr = (*C.uint8_t)(unsafe.Pointer(&name[0]))
	}
	return toGo(C.call_new_v5(symNewV5, toC(ns), namePtr, C.uint32_t(len(name))))
}

func newV6(unixMillis uint64) (uuid.UUID, int32) {
	return toGo(C.call_new_v6_v7(symNewV6, C.uint64_t(unixMillis)))
}

func newV7(unixMillis uint64) (uuid.UUID, int32) {
	return toGo(C.call_new_v6_v7(symNewV7, C.uint64_t(unixMillis)))
}

func v6UnixMillis(id uuid.UUID) uint64 { return uint64(C.call_unix_millis(symV6UnixMillis, toC(id))) }
func v7UnixMillis(id uuid.UUID) uint64 { return uint64(C.call_unix_millis(symV7UnixMillis, toC(id))) }

func newV6Batch(unixMillis uint64, count uint32, out unsafe.Pointer) int32 {
	return int32(C.call_new_batch(symNewV6Batch, C.uint64_t(unixMillis), C.uint32_t(count), (*C.uint8_t)(out)))
}

func newV7Batch(unixMillis uint64, count uint32, out unsafe.Pointer) int32 {
	return int32(C.call_new_batch(symNewV7Batch, C.uint64_t(unixMillis), C.uint32_t(count), (*C.uint8_t)(out)))
}

func sqlOrder(sym unsafe.Pointer, id uuid.UUID) uuid.UUID {
	r := C.call_sql_order(sym, toC(id))
	return *(*uuid.UUID)(unsafe.Pointer(&r))
}

func v7ToSqlOrder(id uuid.UUID) uuid.UUID { return sqlOrder(symV7ToSql, id) }
func v7ToRfcOrder(id uuid.UUID) uuid.UUID { return sqlOrder(symV7ToRfc, id) }
func v6ToSqlOrder(id uuid.UUID) uuid.UUID { return sqlOrder(symV6ToSql, id) }
func v6ToRfcOrder(id uuid.UUID) uuid.UUID { return sqlOrder(symV6ToRfc, id) }

func v7ToSqlOrderBytes(p unsafe.Pointer) { C.call_sql_order_bytes(symV7ToSql, (*C.uint8_t)(p)) }
func v7ToRfcOrderBytes(p unsafe.Pointer) { C.call_sql_order_bytes(symV7ToRfc, (*C.uint8_t)(p)) }
func v6ToSqlOrderBytes(p unsafe.Pointer) { C.call_sql_order_bytes(symV6ToSql, (*C.uint8_t)(p)) }
func v6ToRfcOrderBytes(p unsafe.Pointer) { C.call_sql_order_bytes(symV6ToRfc, (*C.uint8_t)(p)) }
