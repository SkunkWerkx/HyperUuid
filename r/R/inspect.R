#' Inspect UUIDs
#'
#' Results are element-wise; `NA` and malformed input give `NA`. A factor is read as its
#' levels. `layout` and `version` are arguments, so a bad one is an error, as is an `x` that
#' is not character.
#'
#' `uuid_v7_unix_millis()` is the unchecked bit read: it returns the 48 timestamp bits of
#' any well-formed id without checking its version (the corpus replays it on ids of every
#' version), so guard it with [uuid_is_rfc()]. `uuid_timestamp()` is the checked reading.
#'
#' @param x A character vector of UUIDs.
#' @param layout `"rfc"` (RFC 9562 byte order, the default) or `"sql"` (SQL Server order,
#'   the form a v7 takes after conversion).
#' @param version The version to test for, a single whole number from 0 to 15.
#' @return `uuid_version()` an integer vector (in `"sql"` layout, 7 for a converted v7, else
#'   0). `uuid_variant()` a character vector: `"ncs"`, `"rfc9562"`, `"microsoft"` or
#'   `"future"`. `uuid_is_rfc()` a logical vector: is the id an RFC 9562 UUID of `version`
#'   in `layout`. `uuid_timestamp()` a `POSIXct` in UTC, `NA` unless the id is a v7.
#'   `uuid_v7_unix_millis()` a double of Unix milliseconds read straight from the timestamp
#'   bits, without checking the id is a v7 (guard with [uuid_is_rfc()]).
#' @family inspect
#' @export
#' @examples
#' id <- uuid_v7(time = as.POSIXct(1645557742, origin = "1970-01-01", tz = "UTC"))
#' ids <- c(id, uuid_namespaces$dns, NA, "not a uuid")
#' uuid_version(ids)
#' uuid_variant(ids)
#' uuid_is_rfc(ids, 7)
#' uuid_timestamp(ids)
#' uuid_v7_unix_millis(id)
#' uuid_version(uuid_to_sql_order(id), layout = "sql")
uuid_version <- function(x, layout = c("rfc", "sql")) {
  .Call(C_version, as_ids(x), layout_code(layout))
}

#' @rdname uuid_version
#' @export
uuid_variant <- function(x) {
  codes <- .Call(C_variant, as_ids(x))
  c("ncs", "rfc9562", "microsoft", "future")[codes]
}

#' @rdname uuid_version
#' @export
uuid_is_rfc <- function(x, version, layout = c("rfc", "sql")) {
  if (!is.numeric(version) || length(version) != 1L || !is.finite(version) ||
        version != floor(version) || version < 0 || version > 15) {
    stop("version must be a single whole number from 0 to 15", call. = FALSE)
  }
  .Call(C_is_rfc, as_ids(x), as.integer(version), layout_code(layout))
}

#' @rdname uuid_version
#' @export
uuid_timestamp <- function(x, layout = c("rfc", "sql")) {
  millis <- .Call(C_get_timestamp, as_ids(x), layout_code(layout))
  .POSIXct(millis / 1000, tz = "UTC")
}

#' @rdname uuid_version
#' @export
uuid_v7_unix_millis <- function(x, layout = c("rfc", "sql")) {
  .Call(C_v7_unix_millis, as_ids(x), layout_code(layout))
}
