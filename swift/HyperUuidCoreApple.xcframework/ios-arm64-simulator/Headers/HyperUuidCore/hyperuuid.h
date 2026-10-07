// The C ABI of the hyperuuid core: the thirteen functions rust/src/ffi.rs exports, which is
// everything the static libraries in this bundle define. The Swift binding imports this as
// the module HyperUuidCore where the core is linked in (Linux and WebAssembly); every other
// binding declares the same signatures in its own language.
//
// Each uuid pointer is sixteen bytes; a batch's out_ptr is sixteen bytes per UUID. A
// generating function returns 0 on success, 1 if the random source failed, and 2 if
// unix_millis does not fit the timestamp field.
#ifndef HYPERUUID_H
#define HYPERUUID_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// The crate version, packed major << 16 | minor << 8 | patch.
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

#ifdef __cplusplus
}
#endif

#endif
