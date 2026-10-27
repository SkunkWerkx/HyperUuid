#include <R.h>
#include <R_ext/Rdynload.h>
#include <Rinternals.h>
#include <stddef.h>

SEXP C_native_version(void);
SEXP C_new_v4(SEXP n, SEXP as_raw);
SEXP C_new_v7(SEXP millis, SEXP n, SEXP as_raw);
SEXP C_to_raw(SEXP x);
SEXP C_from_raw(SEXP m);
SEXP C_version(SEXP x, SEXP layout);
SEXP C_variant(SEXP x);
SEXP C_is_rfc(SEXP x, SEXP version, SEXP layout);
SEXP C_get_timestamp(SEXP x, SEXP layout);
SEXP C_v7_unix_millis(SEXP x, SEXP layout);
SEXP C_new_v5(SEXP ns_bytes, SEXP names);
SEXP C_sql_order(SEXP x, SEXP direction);

static const R_CallMethodDef CALL_ENTRIES[] = {
    {"native_version", (DL_FUNC)&C_native_version, 0},
    {"new_v4", (DL_FUNC)&C_new_v4, 2},
    {"new_v7", (DL_FUNC)&C_new_v7, 3},
    {"to_raw", (DL_FUNC)&C_to_raw, 1},
    {"from_raw", (DL_FUNC)&C_from_raw, 1},
    {"version", (DL_FUNC)&C_version, 2},
    {"variant", (DL_FUNC)&C_variant, 1},
    {"is_rfc", (DL_FUNC)&C_is_rfc, 3},
    {"get_timestamp", (DL_FUNC)&C_get_timestamp, 2},
    {"v7_unix_millis", (DL_FUNC)&C_v7_unix_millis, 2},
    {"new_v5", (DL_FUNC)&C_new_v5, 2},
    {"sql_order", (DL_FUNC)&C_sql_order, 2},
    {NULL, NULL, 0},
};

void R_init_hyperuuid(DllInfo *dll) {
  R_registerRoutines(dll, NULL, CALL_ENTRIES, NULL, NULL);
  R_useDynamicSymbols(dll, FALSE);
  R_forceSymbols(dll, TRUE);
}
