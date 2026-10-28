#include <R.h>
#include <Rinternals.h>
#include <limits.h>
#include <string.h>

#include "core.h"

/*
 * Text <-> bytes helpers shared by every entry point. Neither allocates; both are
 * table-driven.
 */

/* The value of a hex digit, or -1. Indexed by an unsigned char. */
static const signed char HEX_VALUE[256] = {
    ['0'] = 0 + 1,  ['1'] = 1 + 1,  ['2'] = 2 + 1,  ['3'] = 3 + 1,  ['4'] = 4 + 1,  ['5'] = 5 + 1,
    ['6'] = 6 + 1,  ['7'] = 7 + 1,  ['8'] = 8 + 1,  ['9'] = 9 + 1,  ['a'] = 10 + 1, ['b'] = 11 + 1,
    ['c'] = 12 + 1, ['d'] = 13 + 1, ['e'] = 14 + 1, ['f'] = 15 + 1, ['A'] = 10 + 1, ['B'] = 11 + 1,
    ['C'] = 12 + 1, ['D'] = 13 + 1, ['E'] = 14 + 1, ['F'] = 15 + 1,
};
/* HEX_VALUE stores value + 1 so the zero-initialized entries mean "not a digit". */
#define HEX(c) ((int)HEX_VALUE[(unsigned char)(c)] - 1)

static const char HEX_DIGIT[16] = "0123456789abcdef";

/*
 * Every loop over n ids lets the user interrupt once per 2^20 iterations (never on the
 * first, so a one-id call pays nothing). R_CheckUserInterrupt can longjmp out: that is safe
 * here because each loop's only allocations are PROTECTed R vectors, which R unwinds, and
 * its buffers are on the stack. Nothing is malloc'd.
 */
static inline void interrupt_point(uint64_t i) {
  if ((i & 0xFFFFF) == 0xFFFFF)
    R_CheckUserInterrupt();
}

/*
 * Parses 32 hex digits, or the 8-4-4-4-12 hyphenated form, in any case, into 16 bytes.
 * Returns 1 on success and 0 on anything malformed (out is then unspecified).
 */
static int parse16(const char *s, uint8_t out[16]) {
  size_t len = 0;
  while (len < 37 && s[len] != '\0')
    len++;
  int hyphenated;
  if (len == 32) {
    hyphenated = 0;
  } else if (len == 36) {
    if (s[8] != '-' || s[13] != '-' || s[18] != '-' || s[23] != '-')
      return 0;
    hyphenated = 1;
  } else {
    return 0;
  }
  size_t i = 0;
  for (int b = 0; b < 16; b++) {
    if (hyphenated && (i == 8 || i == 13 || i == 18 || i == 23))
      i++;
    int hi = HEX(s[i]);
    int lo = HEX(s[i + 1]);
    if (hi < 0 || lo < 0)
      return 0;
    out[b] = (uint8_t)((hi << 4) | lo);
    i += 2;
  }
  return 1;
}

/* Formats 16 bytes as the lowercase hyphenated form, NUL-terminated. */
static void format36(const uint8_t in[16], char out[37]) {
  size_t o = 0;
  for (int b = 0; b < 16; b++) {
    if (b == 4 || b == 6 || b == 8 || b == 10)
      out[o++] = '-';
    out[o++] = HEX_DIGIT[in[b] >> 4];
    out[o++] = HEX_DIGIT[in[b] & 0x0f];
  }
  out[36] = '\0';
}

/* The packed core version (major << 16 | minor << 8 | patch); R unpacks it. */
SEXP C_native_version(void) { return ScalarInteger((int)hyperuuid_version()); }

/* ---- generation ---- */

#define MAX_BATCH 67108864u            /* 2^26, the v7 counter space */
#define MILLIS_LIMIT 281474976710656.0 /* 2^48 */

/* The 16 x n raw matrix every generator fills, then (optionally) turned into strings. */
static SEXP alloc_ids(int n) { return allocMatrix(RAWSXP, 16, n); }

/* Returns the 16 x n raw matrix as is, or the n lowercase hyphenated strings. */
static SEXP shape_ids(SEXP raw, int n, int as_raw) {
  if (as_raw)
    return raw;
  SEXP out = PROTECT(allocVector(STRSXP, n));
  const uint8_t *p = RAW(raw);
  char buf[37];
  for (int i = 0; i < n; i++) {
    interrupt_point((uint64_t)i);
    format36(p + (size_t)i * 16, buf);
    SET_STRING_ELT(out, i, mkCharLen(buf, 36));
  }
  UNPROTECT(1);
  return out;
}

static int count_arg(SEXP n) {
  int v = asInteger(n);
  if (v == NA_INTEGER || v < 0)
    Rf_error("n must be a single non-negative whole number");
  return v;
}

/* n random version 4 ids. */
SEXP C_new_v4(SEXP n, SEXP as_raw) {
  int count = count_arg(n);
  SEXP raw = PROTECT(alloc_ids(count));
  uint8_t *p = RAW(raw);
  for (int i = 0; i < count; i++) {
    interrupt_point((uint64_t)i);
    if (uuid_new_v4(p + (size_t)i * 16) != 0)
      Rf_error("uuid_new_v4: random source failure");
  }
  SEXP out = shape_ids(raw, count, asLogical(as_raw) == 1);
  UNPROTECT(1);
  return out;
}

static uint64_t millis_arg(double d) {
  if (!(d >= 0.0 && d < MILLIS_LIMIT)) {
    Rf_error("unix_millis must be non-negative and fit within 48 bits");
  }
  return (uint64_t)d;
}

/*
 * n version 7 ids. millis is a double vector of length 1 (one batch call) or n (one call
 * per id), holding whole milliseconds R has already checked are finite.
 */
SEXP C_new_v7(SEXP millis, SEXP n, SEXP as_raw) {
  int count = count_arg(n);
  R_xlen_t mlen = XLENGTH(millis);
  if (TYPEOF(millis) != REALSXP || (mlen != 1 && mlen != count)) {
    Rf_error("millis must be a double vector of length 1 or n");
  }
  const double *m = REAL(millis);
  for (R_xlen_t i = 0; i < mlen; i++)
    millis_arg(m[i]);
  if (mlen == 1 && (uint64_t)count > MAX_BATCH) {
    Rf_error("batch count exceeds the 26-bit counter space (67108864 per call)");
  }
  SEXP raw = PROTECT(alloc_ids(count));
  uint8_t *p = RAW(raw);
  if (count > 0) {
    if (mlen == 1) {
      int32_t rc = uuid_new_v7_batch(millis_arg(m[0]), (uint32_t)count, p);
      if (rc == 1)
        Rf_error("uuid_new_v7_batch: random source failure");
      if (rc == 2)
        Rf_error("unix_millis must be non-negative and fit within 48 bits");
      if (rc == 4)
        Rf_error("batch count exceeds the 26-bit counter space (67108864 per call)");
      if (rc == 6)
        Rf_error("version 7 counter exhausted: the stream cannot advance without running more than "
                 "1000 ms ahead of the supplied timestamp or past its last millisecond");
      if (rc != 0)
        Rf_error("uuid_new_v7_batch: unexpected status %d", (int)rc);
    } else {
      for (int i = 0; i < count; i++) {
        interrupt_point((uint64_t)i);
        int32_t rc = uuid_new_v7(millis_arg(m[i]), p + (size_t)i * 16);
        if (rc == 1)
          Rf_error("uuid_new_v7: random source failure");
        if (rc == 2)
          Rf_error("unix_millis must be non-negative and fit within 48 bits");
        if (rc == 6)
          Rf_error("version 7 counter exhausted: the stream cannot advance without running more "
                   "than 1000 ms ahead of the supplied timestamp or past its last millisecond");
        if (rc != 0)
          Rf_error("uuid_new_v7: unexpected status %d", (int)rc);
      }
    }
  }
  SEXP out = shape_ids(raw, count, asLogical(as_raw) == 1);
  UNPROTECT(1);
  return out;
}

/* ---- parsing, raw conversion, inspection ---- */

static void require_strings(SEXP x) {
  if (TYPEOF(x) != STRSXP)
    Rf_error("x must be a character vector of UUIDs");
}

/* Strict text to bytes: a 16 x n raw matrix, or an error naming the first bad element. */
SEXP C_to_raw(SEXP x) {
  require_strings(x);
  R_xlen_t n = XLENGTH(x);
  if (n > INT_MAX)
    Rf_error("x is too long");
  SEXP raw = PROTECT(alloc_ids((int)n));
  uint8_t *p = RAW(raw);
  for (R_xlen_t i = 0; i < n; i++) {
    interrupt_point((uint64_t)i);
    SEXP s = STRING_ELT(x, i);
    if (s == NA_STRING)
      Rf_error("uuid_to_raw(): element %lld is NA", (long long)i + 1);
    if (!parse16(CHAR(s), p + (size_t)i * 16)) {
      Rf_error("uuid_to_raw(): element %lld is not a UUID", (long long)i + 1);
    }
  }
  UNPROTECT(1);
  return raw;
}

/* A raw matrix with 16 rows, or a raw vector of 16k bytes, to k canonical strings. */
SEXP C_from_raw(SEXP m) {
  static const char *msg = "uuid_from_raw(): expected a raw matrix with 16 rows or a raw "
                           "vector whose length is a multiple of 16";
  if (TYPEOF(m) != RAWSXP)
    Rf_error("%s", msg);
  SEXP dim = getAttrib(m, R_DimSymbol);
  if (dim != R_NilValue) {
    if (TYPEOF(dim) != INTSXP || LENGTH(dim) != 2 || INTEGER(dim)[0] != 16)
      Rf_error("%s", msg);
  } else if (XLENGTH(m) % 16 != 0) {
    Rf_error("%s", msg);
  }
  R_xlen_t n = XLENGTH(m) / 16;
  SEXP out = PROTECT(allocVector(STRSXP, n));
  const uint8_t *p = RAW(m);
  char buf[37];
  for (R_xlen_t i = 0; i < n; i++) {
    interrupt_point((uint64_t)i);
    format36(p + (size_t)i * 16, buf);
    SET_STRING_ELT(out, i, mkCharLen(buf, 36));
  }
  UNPROTECT(1);
  return out;
}

static uint32_t layout_arg(SEXP layout) {
  int v = asInteger(layout);
  if (v != 1 && v != 2)
    Rf_error("layout must be \"rfc\" or \"sql\"");
  return (uint32_t)v;
}

/* The element's bytes, or 0 for NA or malformed. */
static int element16(SEXP x, R_xlen_t i, uint8_t out[16]) {
  SEXP s = STRING_ELT(x, i);
  return s != NA_STRING && parse16(CHAR(s), out);
}

SEXP C_version(SEXP x, SEXP layout) {
  require_strings(x);
  uint32_t lay = layout_arg(layout);
  R_xlen_t n = XLENGTH(x);
  SEXP out = PROTECT(allocVector(INTSXP, n));
  uint8_t b[16];
  for (R_xlen_t i = 0; i < n; i++) {
    interrupt_point((uint64_t)i);
    INTEGER(out)[i] = element16(x, i, b) ? (int)uuid_version(b, lay) : NA_INTEGER;
  }
  UNPROTECT(1);
  return out;
}

/* Variant codes 1-4 (NA for NA or malformed); R maps them to names. */
SEXP C_variant(SEXP x) {
  require_strings(x);
  R_xlen_t n = XLENGTH(x);
  SEXP out = PROTECT(allocVector(INTSXP, n));
  uint8_t b[16];
  for (R_xlen_t i = 0; i < n; i++) {
    interrupt_point((uint64_t)i);
    INTEGER(out)[i] = element16(x, i, b) ? (int)uuid_variant(b) : NA_INTEGER;
  }
  UNPROTECT(1);
  return out;
}

SEXP C_is_rfc(SEXP x, SEXP version, SEXP layout) {
  require_strings(x);
  int v = asInteger(version);
  if (v == NA_INTEGER || v < 0 || v > 15)
    Rf_error("version must be a single whole number from 0 to 15");
  uint32_t lay = layout_arg(layout);
  R_xlen_t n = XLENGTH(x);
  SEXP out = PROTECT(allocVector(LGLSXP, n));
  uint8_t b[16];
  for (R_xlen_t i = 0; i < n; i++) {
    interrupt_point((uint64_t)i);
    LOGICAL(out)[i] = element16(x, i, b) ? (int)uuid_is_rfc(b, (uint32_t)v, lay) : NA_LOGICAL;
  }
  UNPROTECT(1);
  return out;
}

/* Unix milliseconds of each v7, NA for anything else (including NA and malformed). */
SEXP C_get_timestamp(SEXP x, SEXP layout) {
  require_strings(x);
  uint32_t lay = layout_arg(layout);
  R_xlen_t n = XLENGTH(x);
  SEXP out = PROTECT(allocVector(REALSXP, n));
  uint8_t b[16];
  for (R_xlen_t i = 0; i < n; i++) {
    interrupt_point((uint64_t)i);
    uint64_t ms = 0;
    if (element16(x, i, b) && uuid_get_timestamp(b, lay, &ms) == 7) {
      REAL(out)[i] = (double)ms;
    } else {
      REAL(out)[i] = NA_REAL;
    }
  }
  UNPROTECT(1);
  return out;
}

/* The unchecked bit read: whatever the timestamp field holds, NA only for NA or malformed. */
SEXP C_v7_unix_millis(SEXP x, SEXP layout) {
  require_strings(x);
  uint32_t lay = layout_arg(layout);
  R_xlen_t n = XLENGTH(x);
  SEXP out = PROTECT(allocVector(REALSXP, n));
  uint8_t b[16];
  for (R_xlen_t i = 0; i < n; i++) {
    interrupt_point((uint64_t)i);
    REAL(out)[i] = element16(x, i, b) ? (double)uuid_v7_unix_millis_in(b, lay) : NA_REAL;
  }
  UNPROTECT(1);
  return out;
}

/* ---- version 5 and SQL Server order ---- */

/*
 * v5 ids of `names` in the namespace `ns` (16 raw bytes made in R). names is a STRSXP
 * (NA gives NA, otherwise the UTF-8 bytes) or a VECSXP of RAWSXP (one name per element).
 */
SEXP C_new_v5(SEXP ns_bytes, SEXP names) {
  if (TYPEOF(ns_bytes) != RAWSXP || XLENGTH(ns_bytes) != 16) {
    Rf_error("namespace must be a raw vector of 16 bytes");
  }
  int is_list = TYPEOF(names) == VECSXP;
  if (!is_list && TYPEOF(names) != STRSXP) {
    Rf_error("name must be a character vector or a list of raw vectors");
  }
  R_xlen_t n = XLENGTH(names);
  SEXP out = PROTECT(allocVector(STRSXP, n));
  const uint8_t *ns = RAW(ns_bytes);
  uint8_t id[16];
  char buf[37];
  static const uint8_t EMPTY = 0;
  for (R_xlen_t i = 0; i < n; i++) {
    interrupt_point((uint64_t)i);
    const uint8_t *data;
    R_xlen_t len;
    if (is_list) {
      SEXP el = VECTOR_ELT(names, i);
      if (TYPEOF(el) != RAWSXP)
        Rf_error("name must be a list of raw vectors");
      data = RAW(el);
      len = XLENGTH(el);
    } else {
      SEXP s = STRING_ELT(names, i);
      if (s == NA_STRING) {
        SET_STRING_ELT(out, i, NA_STRING);
        continue;
      }
      const char *c = translateCharUTF8(s);
      data = (const uint8_t *)c;
      len = (R_xlen_t)strlen(c);
    }
    if (len > (R_xlen_t)UINT32_MAX)
      Rf_error("name is too long");
    if (len == 0)
      data = &EMPTY;
    int32_t rc = uuid_new_v5(ns, data, (uint32_t)len, id);
    if (rc != 0)
      Rf_error("uuid_new_v5: unexpected status %d", (int)rc);
    format36(id, buf);
    SET_STRING_ELT(out, i, mkCharLen(buf, 36));
  }
  UNPROTECT(1);
  return out;
}

/* direction 1: RFC v7 to SQL Server order; 2: back. NA and malformed give NA. */
SEXP C_sql_order(SEXP x, SEXP direction) {
  require_strings(x);
  int dir = asInteger(direction);
  if (dir != 1 && dir != 2)
    Rf_error("direction must be 1 or 2");
  R_xlen_t n = XLENGTH(x);
  SEXP out = PROTECT(allocVector(STRSXP, n));
  uint8_t b[16];
  char buf[37];
  for (R_xlen_t i = 0; i < n; i++) {
    interrupt_point((uint64_t)i);
    if (!element16(x, i, b)) {
      SET_STRING_ELT(out, i, NA_STRING);
      continue;
    }
    int32_t rc = dir == 1 ? uuid_v7_to_sql_order(b) : uuid_v7_to_rfc_order(b);
    if (rc == 5) {
      Rf_error("the value is not a UUID of the version this conversion takes (element %lld)",
               (long long)i + 1);
    }
    if (rc != 0) {
      Rf_error("%s: unexpected status %d",
               dir == 1 ? "uuid_v7_to_sql_order" : "uuid_v7_to_rfc_order", (int)rc);
    }
    format36(b, buf);
    SET_STRING_ELT(out, i, mkCharLen(buf, 36));
  }
  UNPROTECT(1);
  return out;
}
