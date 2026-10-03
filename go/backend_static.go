//go:build cgo && !tinygo && (darwin || linux || windows) && (amd64 || arm64)

// The native backend: libhyperuuid linked into the binary. The core is a static library under
// staticlib/{goos}_{goarch}/, named on the cgo link line below, and every call is an
// ordinary C call to a symbol the linker resolved. Nothing is embedded, nothing is written to
// a temp directory, nothing is loaded at run time: a binary carries the ~20 KB of the core
// for its own platform, starts without touching the filesystem, runs from a read-only or
// `scratch` image, and has nothing that can fail to load.
//
// That takes cgo, and so a C compiler wherever the module is built — gcc or clang on Linux,
// the Xcode command-line tools on macOS, a MinGW-w64 gcc (or llvm-mingw on arm64) on
// Windows. TinyGo compiling to WebAssembly links the same core from backend_tinygo.go — its
// cgo cannot parse this file's per-platform #cgo lines, hence `!tinygo` above. Anything else
// does not compile; unsupported.go says so by name. There used to be
// three more backends — a purego one for CGO_ENABLED=0 and Windows that extracted an
// embedded shared library to a temp file, a cgo one that loaded that library instead of
// linking it, and a wasmtime one — and none of them reached a platform this one does not:
// wasmtime-go itself needs cgo and ships engines only for these same platforms.
//
// One archive serves both C libraries on Linux. cgo has no build constraint that tells
// glibc from musl, so there cannot be one per libc; the archive is the core built for the
// musl target, which asks the C library for nothing but calls glibc and musl have both had
// for a decade (getrandom, and open/read/poll for the fallback). The suite runs against it
// on Debian and on Alpine.
//
// Windows links the same MSVC archive C#'s Native AOT publish does: MinGW's linker reads
// MSVC's COFF objects, and the archive carries its own import stub for ProcessPrng, the
// entropy source getrandom uses there, so the link line names nothing else.
//
// The archives are committed (staticlib/README.md): a Go module is whatever is in the tree at
// the resolved version, with no packing step to stage them in.
package hyperuuid

/*
#cgo linux,amd64 LDFLAGS: ${SRCDIR}/staticlib/linux_amd64/libhyperuuid.a
#cgo linux,arm64 LDFLAGS: ${SRCDIR}/staticlib/linux_arm64/libhyperuuid.a
#cgo darwin,amd64 LDFLAGS: ${SRCDIR}/staticlib/darwin_amd64/libhyperuuid.a
#cgo darwin,arm64 LDFLAGS: ${SRCDIR}/staticlib/darwin_arm64/libhyperuuid.a
#cgo windows,amd64 LDFLAGS: ${SRCDIR}/staticlib/windows_amd64/libhyperuuid.a
#cgo windows,arm64 LDFLAGS: ${SRCDIR}/staticlib/windows_arm64/libhyperuuid.a
#include <stdint.h>

// The core's C ABI — rust/src/ffi.rs, the thirteen exports every binding calls.
uint32_t hyperuuid_version(void);
int32_t uuid_new_v4(uint8_t *out_ptr);
int32_t uuid_new_v5(const uint8_t *ns_ptr, const uint8_t *name_ptr, uint32_t name_len, uint8_t *out_ptr);
int32_t uuid_new_v6(uint64_t unix_millis, uint8_t *out_ptr);
int32_t uuid_new_v6_batch(uint64_t unix_millis, uint32_t count, uint8_t *out_ptr);
uint64_t uuid_v6_unix_millis(const uint8_t *uuid_ptr);
void uuid_v6_to_sql_order(uint8_t *uuid_ptr);
void uuid_v6_to_rfc_order(uint8_t *uuid_ptr);
int32_t uuid_new_v7(uint64_t unix_millis, uint8_t *out_ptr);
int32_t uuid_new_v7_batch(uint64_t unix_millis, uint32_t count, uint8_t *out_ptr);
uint64_t uuid_v7_unix_millis(const uint8_t *uuid_ptr);
void uuid_v7_to_sql_order(uint8_t *uuid_ptr);
void uuid_v7_to_rfc_order(uint8_t *uuid_ptr);

// A UUID crosses BY VALUE in both directions. Any Go pointer passed to a cgo call escapes
// to the heap, so handing the core `&out[0]` of a Go local costs one allocation per
// NewV4/NewV6/NewV7; these shims keep the 16 bytes on the C stack and return them as a
// struct instead, so no Go pointer crosses for anything but a caller's own slice (the v5
// name, a batch destination).
typedef struct { uint8_t b[16]; } hu_uuid;
typedef struct { hu_uuid id; int32_t code; } hu_result;

static hu_result call_new_v4(void) {
	hu_result r;
	r.code = uuid_new_v4(r.id.b);
	return r;
}
static hu_result call_new_v5(hu_uuid ns, const uint8_t *name, uint32_t name_len) {
	hu_result r;
	r.code = uuid_new_v5(ns.b, name, name_len, r.id.b);
	return r;
}
static hu_result call_new_v6(uint64_t unix_millis) {
	hu_result r;
	r.code = uuid_new_v6(unix_millis, r.id.b);
	return r;
}
static hu_result call_new_v7(uint64_t unix_millis) {
	hu_result r;
	r.code = uuid_new_v7(unix_millis, r.id.b);
	return r;
}
static uint64_t call_v6_unix_millis(hu_uuid uuid) { return uuid_v6_unix_millis(uuid.b); }
static uint64_t call_v7_unix_millis(hu_uuid uuid) { return uuid_v7_unix_millis(uuid.b); }
static hu_uuid call_v6_to_sql_order(hu_uuid uuid) { uuid_v6_to_sql_order(uuid.b); return uuid; }
static hu_uuid call_v6_to_rfc_order(hu_uuid uuid) { uuid_v6_to_rfc_order(uuid.b); return uuid; }
static hu_uuid call_v7_to_sql_order(hu_uuid uuid) { uuid_v7_to_sql_order(uuid.b); return uuid; }
static hu_uuid call_v7_to_rfc_order(hu_uuid uuid) { uuid_v7_to_rfc_order(uuid.b); return uuid; }
*/
import "C"

import (
	"unsafe"

	"github.com/google/uuid"
)

// packedVersion is the core's own version, major<<16 | minor<<8 | patch, read through the
// ABI rather than from this module, so NativeVersion reports the archive that was linked.
func packedVersion() uint32 { return uint32(C.hyperuuid_version()) }

func toGo(r C.hu_result) (uuid.UUID, int32) {
	return *(*uuid.UUID)(unsafe.Pointer(&r.id)), int32(r.code)
}

func toC(id uuid.UUID) C.hu_uuid {
	return *(*C.hu_uuid)(unsafe.Pointer(&id))
}

func fromC(id C.hu_uuid) uuid.UUID {
	return *(*uuid.UUID)(unsafe.Pointer(&id))
}

func newV4() (uuid.UUID, int32) { return toGo(C.call_new_v4()) }

func newV5(ns uuid.UUID, name []byte) (uuid.UUID, int32) {
	var namePtr *C.uint8_t
	if len(name) > 0 {
		namePtr = (*C.uint8_t)(unsafe.Pointer(&name[0]))
	}
	return toGo(C.call_new_v5(toC(ns), namePtr, C.uint32_t(len(name))))
}

func newV6(unixMillis uint64) (uuid.UUID, int32) {
	return toGo(C.call_new_v6(C.uint64_t(unixMillis)))
}

func newV7(unixMillis uint64) (uuid.UUID, int32) {
	return toGo(C.call_new_v7(C.uint64_t(unixMillis)))
}

func v6UnixMillis(id uuid.UUID) uint64 { return uint64(C.call_v6_unix_millis(toC(id))) }
func v7UnixMillis(id uuid.UUID) uint64 { return uint64(C.call_v7_unix_millis(toC(id))) }

func newV6Batch(unixMillis uint64, count uint32, out unsafe.Pointer) int32 {
	return int32(C.uuid_new_v6_batch(C.uint64_t(unixMillis), C.uint32_t(count), (*C.uint8_t)(out)))
}

func newV7Batch(unixMillis uint64, count uint32, out unsafe.Pointer) int32 {
	return int32(C.uuid_new_v7_batch(C.uint64_t(unixMillis), C.uint32_t(count), (*C.uint8_t)(out)))
}

func v7ToSqlOrder(id uuid.UUID) uuid.UUID { return fromC(C.call_v7_to_sql_order(toC(id))) }
func v7ToRfcOrder(id uuid.UUID) uuid.UUID { return fromC(C.call_v7_to_rfc_order(toC(id))) }
func v6ToSqlOrder(id uuid.UUID) uuid.UUID { return fromC(C.call_v6_to_sql_order(toC(id))) }
func v6ToRfcOrder(id uuid.UUID) uuid.UUID { return fromC(C.call_v6_to_rfc_order(toC(id))) }

func v7ToSqlOrderBytes(p unsafe.Pointer) { C.uuid_v7_to_sql_order((*C.uint8_t)(p)) }
func v7ToRfcOrderBytes(p unsafe.Pointer) { C.uuid_v7_to_rfc_order((*C.uint8_t)(p)) }
func v6ToSqlOrderBytes(p unsafe.Pointer) { C.uuid_v6_to_sql_order((*C.uint8_t)(p)) }
func v6ToRfcOrderBytes(p unsafe.Pointer) { C.uuid_v6_to_rfc_order((*C.uint8_t)(p)) }
