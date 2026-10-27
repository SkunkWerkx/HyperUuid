# Validates `x` as a character vector of UUIDs; a factor (common from read.csv) is read as
# its levels, not its integer codes.
as_ids <- function(x) {
  # An all-NA logical (a bare NA) is a missing id.
  if (is.factor(x) || (is.logical(x) && all(is.na(x)))) x <- as.character(x)
  if (!is.character(x)) stop("x must be a character vector of UUIDs", call. = FALSE)
  x
}

# Maps the layout argument to the ABI's layout code: 1 for RFC 9562, 2 for SQL Server.
layout_code <- function(layout) {
  c(rfc = 1L, sql = 2L)[[match.arg(layout, c("rfc", "sql"), several.ok = FALSE)]]
}

#' Convert between UUID text and raw bytes
#'
#' Raw results are strict, with no `NA`: `uuid_to_raw()` errors on the first `NA` or
#' malformed element, so filter with `is.na()` or [uuid_is_rfc()] first. Text may be 32 hex
#' digits or the 8-4-4-4-12 hyphenated form, in any case; text out is lowercase and
#' hyphenated. A factor is read as its levels.
#'
#' Errors: `uuid_to_raw()` names the first element that is `NA` or not a UUID, and refuses
#' an `x` that is not character; `uuid_from_raw()` refuses anything but a raw matrix with
#' 16 rows or a raw vector whose length is a multiple of 16.
#'
#' @param x A character vector of UUIDs.
#' @param m A raw matrix with 16 rows (one id per column), or a raw vector whose length is a
#'   multiple of 16.
#' @return `uuid_to_raw()` returns a raw matrix with 16 rows and `length(x)` columns (16 x 0
#'   for an empty `x`). `uuid_from_raw()` returns a character vector, one id per column
#'   (`character(0)` for an empty `m`).
#' @family convert
#' @export
#' @examples
#' m <- uuid_to_raw(c(uuid_nil, "6BA7B810-9DAD-11D1-80B4-00C04FD430C8"))
#' dim(m)
#' uuid_from_raw(m)
#'
#' # Filter first: the raw door errors on NA.
#' ids <- c(uuid_namespaces$dns, NA, "not a uuid")
#' uuid_to_raw(ids[uuid_is_rfc(ids, 1) %in% TRUE])
uuid_to_raw <- function(x) {
  .Call(C_to_raw, as_ids(x))
}

#' @rdname uuid_to_raw
#' @export
uuid_from_raw <- function(m) {
  .Call(C_from_raw, m)
}
