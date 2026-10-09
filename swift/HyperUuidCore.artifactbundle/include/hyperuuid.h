// The C ABI of the hyperuuid core: the nineteen functions rust/src/ffi.rs exports, which is
// everything the static libraries in this bundle define. The Swift binding imports this as
// the module HyperUuidCore where the core is linked in (Linux and WebAssembly); every other
// binding declares the same signatures in its own language.
//
// Each uuid pointer is sixteen bytes; a batch's out_ptr is sixteen bytes per UUID. A
// generating function returns 0 on success, 1 if the random source failed, and 2 if
// unix_millis does not fit the timestamp field. A batch also returns 3 if count * 16 bytes
// cannot be addressed (32-bit targets only), and a v7 batch 4 if count is over 2^26, the
// 26-bit counter space; neither writes anything.
//
// A layout code is 1 for RFC 9562 byte order and 2 for SQL Server order (what the
// *_to_sql_order functions write). A variant code is 1 NCS, 2 RFC 9562, 3 Microsoft, 4 Future.
// 0 is reserved in both: an unknown layout code makes the inspection functions answer 0.
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

// The version as `layout` reads it: the nibble, 0-15, in RFC 9562 order; 6, 7, or 0 ("not a
// SQL-ordered v6 or v7") in SQL Server order.
uint32_t uuid_version(const uint8_t *uuid_ptr, uint32_t layout_code);
// The variant code of an RFC 9562-ordered UUID.
uint32_t uuid_variant(const uint8_t *uuid_ptr);
// 1 if the UUID is an RFC 9562 UUID of `version` held in `layout`'s order, else 0.
uint32_t uuid_is_rfc(const uint8_t *uuid_ptr, uint32_t version, uint32_t layout_code);
// uuid_v6_unix_millis and uuid_v7_unix_millis for a UUID held in `layout`'s order.
uint64_t uuid_v6_unix_millis_in(const uint8_t *uuid_ptr, uint32_t layout_code);
uint64_t uuid_v7_unix_millis_in(const uint8_t *uuid_ptr, uint32_t layout_code);
// The timestamp of an RFC 9562 version 6 or 7 UUID held in `layout`'s order, in one call:
// writes its Unix milliseconds to *millis_out and returns 6 or 7. Anything else (another
// version, another variant, an unknown layout code) returns 0 and leaves *millis_out alone.
uint32_t uuid_get_timestamp(const uint8_t *uuid_ptr, uint32_t layout_code, uint64_t *millis_out);

#ifdef __cplusplus
}
#endif

#endif
