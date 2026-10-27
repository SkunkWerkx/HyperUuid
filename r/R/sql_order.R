#' SQL Server order for version 7 UUIDs
#'
#' `uuid_to_sql_order()` rewrites a v7 into the byte order SQL Server's `uniqueidentifier`
#' sorts by creation time; `uuid_from_sql_order()` is the way back. The result is persisted
#' data: SQL Server does not compare a `uniqueidentifier`'s bytes left to right, so the
#' converted value (an RFC 9562 version 8 as SQL Server and .NET display it) holds the
#' timestamp and counter in the bytes it compares first. Read it in place with
#' `layout = "sql"` in [uuid_version()] and its family. `NA` and malformed input give `NA`;
#' a well-formed id of the wrong version is an error.
#'
#' @param x A character vector of UUIDs (RFC v7 for `uuid_to_sql_order()`, SQL-ordered for
#'   `uuid_from_sql_order()`). A factor is read as its levels.
#' @return A character vector of lowercase hyphenated ids.
#' @family convert
#' @export
#' @examples
#' id <- uuid_v7(time = as.POSIXct(1645557742, origin = "1970-01-01", tz = "UTC"))
#' sql <- uuid_to_sql_order(id)
#' sql
#' uuid_from_sql_order(sql) == id
#' uuid_to_sql_order(c(NA, "not a uuid"))
uuid_to_sql_order <- function(x) {
  .Call(C_sql_order, as_ids(x), 1L)
}

#' @rdname uuid_to_sql_order
#' @export
uuid_from_sql_order <- function(x) {
  .Call(C_sql_order, as_ids(x), 2L)
}
