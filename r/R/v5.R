#' Name-based (version 5) UUIDs
#'
#' The same namespace and name always give the same id (SHA-1, RFC 9562 section 5.5). The
#' output is persisted data in other systems' keys, so it is pinned by the corpus.
#'
#' @param namespace A single UUID string; see [uuid_namespaces]. It is an argument, so an
#'   `NA` or malformed namespace is an error.
#' @param name A character vector (hashed as UTF-8; `NA` gives `NA`), or a list of raw
#'   vectors, each element one name.
#' @return A character vector of lowercase hyphenated ids, one per name.
#' @family generate
#' @export
#' @examples
#' uuid_v5(uuid_namespaces$dns, "www.example.com")
#' uuid_v5(uuid_namespaces$dns, c("a", NA))
#' uuid_v5(uuid_namespaces$dns, list(charToRaw("www.example.com")))
uuid_v5 <- function(namespace, name) {
  if (length(namespace) != 1L) stop("namespace must be a single UUID", call. = FALSE)
  ns <- tryCatch(uuid_to_raw(namespace), error = function(e) {
    stop(if (is.na(namespace)) "namespace is NA" else "namespace is not a UUID", call. = FALSE)
  })
  if (is.factor(name)) name <- as.character(name)
  if (is.logical(name) && all(is.na(name))) name <- as.character(name)
  .Call(C_new_v5, as.vector(ns), name)
}
