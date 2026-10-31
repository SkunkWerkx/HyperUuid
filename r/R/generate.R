#' Generate random (version 4) UUIDs
#'
#' 122 random bits from the core's random source (seeded from the operating system; see the
#' root README). `n` is an argument, so a
#' missing, negative, fractional or non-scalar `n` is an error.
#'
#' @param n How many ids to generate: a single non-negative whole number.
#' @return `uuid_v4()` returns a character vector of `n` lowercase hyphenated ids.
#'   `uuid_v4_raw()` returns a raw matrix with 16 rows and `n` columns, one id per column.
#' @family generate
#' @export
#' @examples
#' uuid_v4()
#' uuid_v4(3)
#' dim(uuid_v4_raw(3))
uuid_v4 <- function(n = 1) {
  .Call(C_new_v4, as_count(n), FALSE)
}

#' @rdname uuid_v4
#' @export
uuid_v4_raw <- function(n = 1) {
  .Call(C_new_v4, as_count(n), TRUE)
}

#' Generate time-ordered (version 7) UUIDs
#'
#' A scalar `time` generates the whole request as one batch, one contiguous run of the
#' stream, so the ids are strictly increasing; at most [uuid_v7_max_batch] per call. A `time` of
#' length `n` generates one id per element.
#'
#' Every call draws from one process-wide stream, so ids made with a `time` within 10 s of
#' the newest `time` the stream has reserved at are ordered after every id from a stream
#' reservation that completed before the call began, even across calls and threads. The
#' stream may run up to 1000 ms ahead of the `time` you pass; a request that would take it
#' further, or past its last millisecond (2^44 - 2), is an error
#' (`version 7 counter exhausted: ...`). It is a state error: it reserves nothing, and a call
#' whose `time` is past the stream's millisecond succeeds. A `time` more than 11 s behind
#' the stream, or past 2^44 - 2 ms, is minted at itself, outside the stream and unordered.
#' See the root README, "v7 ordering, precisely".
#'
#' `time` is converted to whole Unix milliseconds as `floor(seconds * 1000)`; a `Date` is
#' midnight UTC. `time` is an argument, so a missing or non-finite element, a time before
#' 1970 or after 9999-12-31T23:59:59.999Z (the generation cap), a length other than 1 or `n`,
#' and `n` over
#' [uuid_v7_max_batch] with a scalar `time` are errors.
#'
#' @param n How many ids to generate: a single non-negative whole number.
#' @param time A `POSIXct`, `POSIXlt` or `Date` (midnight UTC), or a number of seconds
#'   since the Unix epoch. Length 1, or length `n`. Defaults to the current time.
#' @return `uuid_v7()` returns a character vector of `n` lowercase hyphenated ids.
#'   `uuid_v7_raw()` returns a raw matrix with 16 rows and `n` columns, one id per column.
#' @family generate
#' @export
#' @examples
#' t <- as.POSIXct(1645557742, origin = "1970-01-01", tz = "UTC")
#' ids <- uuid_v7(3, time = t)
#' ids
#' identical(sort(ids), ids)
#' uuid_timestamp(ids)
#' uuid_v7(2, time = c(t, t + 1))
#' dim(uuid_v7_raw(3, time = t))
uuid_v7 <- function(n = 1, time = Sys.time()) {
  n <- as_count(n)
  millis <- unix_millis(time, n)
  .Call(C_new_v7, millis, n, FALSE)
}

#' @rdname uuid_v7
#' @export
uuid_v7_raw <- function(n = 1, time = Sys.time()) {
  n <- as_count(n)
  millis <- unix_millis(time, n)
  .Call(C_new_v7, millis, n, TRUE)
}
