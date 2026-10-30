//go:build hyperuuid_spike

// The archive comes from CGO_LDFLAGS (run.sh sets it to the local core), so the control
// is the same core the translated variants are built from.
package cgostatic

/*
#include <stdint.h>
uint32_t hyperuuid_version(void);
int32_t uuid_new_v4(uint8_t *out_ptr);
int32_t uuid_new_v7(uint64_t unix_millis, uint8_t *out_ptr);
int32_t uuid_new_v7_batch(uint64_t unix_millis, uint32_t count, uint8_t *out_ptr);
uint64_t uuid_v7_unix_millis(const uint8_t *uuid_ptr);
typedef struct { uint8_t b[16]; } hu_uuid;
typedef struct { hu_uuid id; int32_t code; } hu_result;
static hu_result call_new_v4(void) { hu_result r; r.code = uuid_new_v4(r.id.b); return r; }
static hu_result call_new_v7(uint64_t m) { hu_result r; r.code = uuid_new_v7(m, r.id.b); return r; }
static uint64_t call_v7_unix_millis(hu_uuid u) { return uuid_v7_unix_millis(u.b); }
*/
import "C"

import "unsafe"

func Version() uint32        { return uint32(C.hyperuuid_version()) }
func NewV4() (u [16]byte)    { r := C.call_new_v4(); return *(*[16]byte)(unsafe.Pointer(&r.id)) }
func NewV4Ptr() (u [16]byte) { C.uuid_new_v4((*C.uint8_t)(&u[0])); return }
func NewV7(m uint64) (u [16]byte) {
	r := C.call_new_v7(C.uint64_t(m))
	return *(*[16]byte)(unsafe.Pointer(&r.id))
}
func V7Millis(u [16]byte) uint64 {
	return uint64(C.call_v7_unix_millis(*(*C.hu_uuid)(unsafe.Pointer(&u))))
}
func Batch(m uint64, n uint32, buf []byte) {
	C.uuid_new_v7_batch(C.uint64_t(m), C.uint32_t(n), (*C.uint8_t)(&buf[0]))
}
