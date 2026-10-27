#' The version of the native core
#'
#' The version of the Rust core this session actually loaded, decoded from the packed
#' `hyperuuid_version` export every binding probes. It equals the package version.
#'
#' @return A [package_version] of length 1.
#' @family inspect
#' @export
#' @examples
#' hyperuuid_native_version()
hyperuuid_native_version <- function() {
  v <- .Call(C_native_version)
  package_version(sprintf(
    "%d.%d.%d",
    bitwAnd(bitwShiftR(v, 16L), 255L),
    bitwAnd(bitwShiftR(v, 8L), 255L),
    bitwAnd(v, 255L)
  ))
}
