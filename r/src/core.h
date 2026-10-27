/*
 * The C ABI of the hyperuuid core: the thirteen functions rust/src/ffi.rs exports, linked
 * in from the rcore static library (src/rust). This file is the R package's only ABI
 * surface, and .github/scripts/check_exports.py reads it: keep every prototype on one line,
 * starting at column 0 with its return type.
 *
 * Each uuid pointer is sixteen bytes; a batch's out_ptr is sixteen bytes per UUID. A
 * generating function returns 0 on success, 1 if the random source failed, and 2 if
 * unix_millis does not fit the timestamp field. A batch also returns 3 if count * 16 bytes
 * cannot be addressed (32-bit targets only), and 4 if count is over 2^26; neither writes
 * anything. The two order conversions rewrite their bytes in place and return 0, or 5 if
 * the value is not of the version the function takes, leaving the bytes as they were.
 *
 * A layout code is 1 for RFC 9562 byte order and 2 for SQL Server order. A variant code is
 * 1 NCS, 2 RFC 9562, 3 Microsoft, 4 Future. 0 is reserved in both.
 */
#ifndef HYPERUUID_CORE_H
#define HYPERUUID_CORE_H

#include <stdint.h>

/* One line per prototype, whatever its width: check_exports.py reads them. */
// clang-format off
/* The crate version, packed major << 16 | minor << 8 | patch. */
uint32_t hyperuuid_version(void);

int32_t uuid_new_v4(uint8_t *out_ptr);
int32_t uuid_new_v5(const uint8_t *ns_ptr, const uint8_t *name_ptr, uint32_t name_len, uint8_t *out_ptr);

int32_t uuid_new_v7(uint64_t unix_millis, uint8_t *out_ptr);
int32_t uuid_new_v7_batch(uint64_t unix_millis, uint32_t count, uint8_t *out_ptr);
uint64_t uuid_v7_unix_millis(const uint8_t *uuid_ptr);
int32_t uuid_v7_to_sql_order(uint8_t *uuid_ptr);
int32_t uuid_v7_to_rfc_order(uint8_t *uuid_ptr);

/* The version as `layout` reads it (in SQL Server order, 7 for a converted v7, else 0). */
uint32_t uuid_version(const uint8_t *uuid_ptr, uint32_t layout_code);
/* The variant code of an RFC 9562-ordered UUID. */
uint32_t uuid_variant(const uint8_t *uuid_ptr);
/* 1 if the UUID is an RFC 9562 UUID of `version` held in `layout`'s order, else 0. */
uint32_t uuid_is_rfc(const uint8_t *uuid_ptr, uint32_t version, uint32_t layout_code);
/* uuid_v7_unix_millis for a UUID held in `layout`'s order. */
uint64_t uuid_v7_unix_millis_in(const uint8_t *uuid_ptr, uint32_t layout_code);
/* Writes a v7's Unix milliseconds to *millis_out and returns 7; anything else returns 0. */
uint32_t uuid_get_timestamp(const uint8_t *uuid_ptr, uint32_t layout_code, uint64_t *millis_out);

// clang-format on
#endif
