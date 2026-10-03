//go:build tinygo.wasm

// The same backend for TinyGo compiling to WebAssembly — a browser (`-target=wasm`, whose
// wasm_exec.js is TinyGo's own) or a WASI runtime (`-target=wasip1`). Stock Go cannot do
// this: its wasm toolchain links Go code only. TinyGo compiles through LLVM, links with
// wasm-ld against wasi-libc, and has cgo, so the core is an ordinary archive on the link
// line again: staticlib/wasm/libhyperuuid.a, the core built for wasm32-wasip1 — the same
// bytes as Swift's WebAssembly archive. Even `-target=wasm` is a wasm32-wasi build
// underneath, so the object links unchanged, and the one thing it imports, WASI's
// random_get, is supplied by TinyGo's wasm_exec.js from crypto.getRandomValues. Nothing is
// fetched or instantiated beside the app's own module, and the core and this binding add
// about 43 KB to an `-opt=z` build.
//
// The selector is the tinygo.wasm tag, which every TinyGo WebAssembly target sets. Not
// `cgo` (TinyGo never sets it) and not `wasm` (wasip2 and wasm-unknown report GOARCH=arm).
//
// Three limits of TinyGo's cgo make this a file of its own rather than more lines in
// backend_static.go, each of which failed there first:
//
//   - A #cgo line cannot carry a build constraint (`not implemented: build constraints in
//     #cgo line`, tinygo-org/tinygo#4087), so backend_static.go cannot even be parsed.
//     This file's //go:build does the selecting and its one #cgo line is unconditional.
//   - ${SRCDIR} is not expanded, and a bare archive path is refused as an invalid flag. A
//     relative -L is resolved against the package directory instead, which is where the
//     module cache puts staticlib/ too.
//   - A C struct cannot cross by value (tinygo-org/tinygo#4489). backend_static.go's
//     by-value shims link and then trap on the first call, so here the UUID crosses as a
//     pointer to the Go value, the way the core's own exports take it.
//
// `-target=wasip2` compiles and links, then stops at componentizing: TinyGo builds the
// component with no preview1 adapter, and the archive's random_get is a preview1 import.
package hyperuuid

/*
#cgo LDFLAGS: -Lstaticlib/wasm -lhyperuuid
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
*/
import "C"

import (
	"unsafe"

	"github.com/google/uuid"
)

// ptr is the 16 bytes of a UUID as the core takes them.
func ptr(id *uuid.UUID) *C.uint8_t { return (*C.uint8_t)(unsafe.Pointer(&id[0])) }

// packedVersion is the core's own version, major<<16 | minor<<8 | patch, read through the
// ABI rather than from this module, so NativeVersion reports the archive that was linked.
func packedVersion() uint32 { return uint32(C.hyperuuid_version()) }

func newV4() (id uuid.UUID, code int32) {
	code = int32(C.uuid_new_v4(ptr(&id)))
	return id, code
}

func newV5(ns uuid.UUID, name []byte) (id uuid.UUID, code int32) {
	var namePtr *C.uint8_t
	if len(name) > 0 {
		namePtr = (*C.uint8_t)(unsafe.Pointer(&name[0]))
	}
	code = int32(C.uuid_new_v5(ptr(&ns), namePtr, C.uint32_t(len(name)), ptr(&id)))
	return id, code
}

func newV6(unixMillis uint64) (id uuid.UUID, code int32) {
	code = int32(C.uuid_new_v6(C.uint64_t(unixMillis), ptr(&id)))
	return id, code
}

func newV7(unixMillis uint64) (id uuid.UUID, code int32) {
	code = int32(C.uuid_new_v7(C.uint64_t(unixMillis), ptr(&id)))
	return id, code
}

func v6UnixMillis(id uuid.UUID) uint64 { return uint64(C.uuid_v6_unix_millis(ptr(&id))) }
func v7UnixMillis(id uuid.UUID) uint64 { return uint64(C.uuid_v7_unix_millis(ptr(&id))) }

func newV6Batch(unixMillis uint64, count uint32, out unsafe.Pointer) int32 {
	return int32(C.uuid_new_v6_batch(C.uint64_t(unixMillis), C.uint32_t(count), (*C.uint8_t)(out)))
}

func newV7Batch(unixMillis uint64, count uint32, out unsafe.Pointer) int32 {
	return int32(C.uuid_new_v7_batch(C.uint64_t(unixMillis), C.uint32_t(count), (*C.uint8_t)(out)))
}

func v7ToSqlOrder(id uuid.UUID) uuid.UUID { C.uuid_v7_to_sql_order(ptr(&id)); return id }
func v7ToRfcOrder(id uuid.UUID) uuid.UUID { C.uuid_v7_to_rfc_order(ptr(&id)); return id }
func v6ToSqlOrder(id uuid.UUID) uuid.UUID { C.uuid_v6_to_sql_order(ptr(&id)); return id }
func v6ToRfcOrder(id uuid.UUID) uuid.UUID { C.uuid_v6_to_rfc_order(ptr(&id)); return id }

func v7ToSqlOrderBytes(p unsafe.Pointer) { C.uuid_v7_to_sql_order((*C.uint8_t)(p)) }
func v7ToRfcOrderBytes(p unsafe.Pointer) { C.uuid_v7_to_rfc_order((*C.uint8_t)(p)) }
func v6ToSqlOrderBytes(p unsafe.Pointer) { C.uuid_v6_to_sql_order((*C.uint8_t)(p)) }
func v6ToRfcOrderBytes(p unsafe.Pointer) { C.uuid_v6_to_rfc_order((*C.uint8_t)(p)) }
