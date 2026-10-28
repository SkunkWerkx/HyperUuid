#' The largest batch a single `uuid_v7()` call can generate
#'
#' A scalar `time` generates one batch, a contiguous run of the process-wide v7 stream's
#' steps at 64 ids to a step, so a call holds at most this many ids (2^26, every step one
#' millisecond holds). Ask for more and [uuid_v7()] errors; split the
#' request, or pass a `time` of length `n`.
#'
#' @format An integer, `67108864L`.
#' @family generate
#' @export
#' @examples
#' uuid_v7_max_batch
#' uuid_v7_max_batch == 2^26
uuid_v7_max_batch <- 67108864L

#' The nil and max UUIDs
#'
#' The two special values of RFC 9562 sections 5.9 and 5.10. Neither has an RFC 9562
#' variant, so [uuid_is_rfc()] is `FALSE` for both.
#'
#' @format Strings: `uuid_nil` is all zero bits, `uuid_max` all one bits.
#' @family convert
#' @export
#' @examples
#' uuid_nil
#' uuid_max
#' uuid_variant(c(uuid_nil, uuid_max))
uuid_nil <- "00000000-0000-0000-0000-000000000000"

#' @rdname uuid_nil
#' @export
uuid_max <- "ffffffff-ffff-ffff-ffff-ffffffffffff"

#' The RFC 9562 namespace UUIDs
#'
#' The four predefined namespaces for [uuid_v5()] (RFC 9562 appendix A): `dns`, `url`,
#' `oid` and `x500`.
#'
#' @format A named list of four strings.
#' @family generate
#' @export
#' @examples
#' uuid_namespaces$dns
#' uuid_v5(uuid_namespaces$url, "https://www.ietf.org/rfc/rfc9562.txt")
uuid_namespaces <- list(
  dns = "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
  url = "6ba7b811-9dad-11d1-80b4-00c04fd430c8",
  oid = "6ba7b812-9dad-11d1-80b4-00c04fd430c8",
  x500 = "6ba7b814-9dad-11d1-80b4-00c04fd430c8"
)
