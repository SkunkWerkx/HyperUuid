# Replays the shared conformance corpus (corpus/*.json at the repository root) through the
# public API, the same vectors the Rust core and every other binding replay. The staged copy
# under tests/testthat/corpus/ is what the tarball ships; a checkout without it walks up to
# the root's corpus/. A missing corpus is an error, never a skip.

# A dependency skip, not a corpus skip: jsonlite is in Suggests, and CRAN's noSuggests
# flavour runs the tests without it. With jsonlite present, a missing corpus still errors.
testthat::skip_if_not_installed("jsonlite")

corpus_dir <- function() {
  staged <- testthat::test_path("corpus")
  if (dir.exists(staged)) {
    return(staged)
  }
  here <- getwd()
  for (i in 1:6) {
    here <- file.path(here, "..")
    candidate <- file.path(here, "corpus")
    if (dir.exists(candidate) && file.exists(file.path(candidate, "v5.json"))) {
      return(normalizePath(candidate))
    }
  }
  stop("corpus not found at ", staged, " or above ", getwd())
}

read_vectors <- function(name) {
  path <- file.path(corpus_dir(), name)
  if (!file.exists(path) || file.size(path) == 0) {
    stop("corpus file missing or empty: ", path)
  }
  jsonlite::fromJSON(path, simplifyVector = FALSE)
}

hx <- function(id) gsub("-", "", id)
layout_of <- function(row) c(rfc9562 = "rfc", sql_server = "sql")[[row$layout]]
hex_bytes <- function(hex) {
  if (!nzchar(hex)) {
    return(raw(0))
  }
  as.raw(strtoi(substring(hex, seq(1, nchar(hex), 2), seq(2, nchar(hex), 2)), 16L))
}

test_that("the five corpus files are present in the corpus directory the tests read", {
  files <- c("v5.json", "sql_order.json", "sql_order_refused.json", "timestamp.json",
             "inspect.json")
  for (f in files) {
    expect_true(file.exists(file.path(corpus_dir(), f)), info = f)
  }
})

test_that("v5.json", {
  rows <- read_vectors("v5.json")
  expect_gt(length(rows), 0)
  for (row in rows) {
    ns <- uuid_namespaces[[row$namespace]]
    got <- hx(uuid_v5(ns, list(hex_bytes(row$name_hex))))
    expect_identical(got, row$expect, info = row$name_hex)
    if (!is.null(row$name)) {
      expect_identical(hx(uuid_v5(ns, row$name)), row$expect, info = row$name)
    }
  }
})

test_that("sql_order.json", {
  rows <- read_vectors("sql_order.json")
  expect_gt(length(rows), 0)
  for (row in rows) {
    expect_identical(hx(uuid_to_sql_order(row$rfc)), row$sql, info = row$rfc)
    expect_identical(hx(uuid_from_sql_order(row$sql)), row$rfc, info = row$sql)
  }
})

test_that("sql_order_refused.json", {
  rows <- read_vectors("sql_order_refused.json")
  expect_gt(length(rows), 0)
  msg <- "the value is not a UUID of the version this conversion takes"
  for (row in rows) {
    convert <- switch(row$direction,
      to_sql = uuid_to_sql_order,
      from_sql = uuid_from_sql_order,
      stop("unknown direction ", row$direction)
    )
    expect_error(convert(row$uuid), msg, fixed = TRUE, info = row$uuid)
  }
})

# as.numeric of a POSIXct is seconds, so * 1000 carries double rounding noise; the exact
# millisecond is asserted through uuid_v7_unix_millis, the timestamp is rounded to the ms.
test_that("timestamp.json", {
  rows <- read_vectors("timestamp.json")
  expect_gt(length(rows), 0)
  for (row in rows) {
    lay <- layout_of(row)
    rfc <- lay == "rfc"
    id <- row$uuid
    expect_identical(uuid_version(id, lay), as.integer(row$version), info = id)
    # The version-agnostic door answers only for a timestamp the core would mint: a version 7
    # dated past the generation cap, 253402300799999 ms, has none there, as a version 6 has
    # none. uuid_v7_unix_millis reads the 48-bit field as it is.
    minted <- !is.null(row$unix_millis) && row$unix_millis <= 253402300799999
    if (!minted) {
      expect_true(is.na(uuid_timestamp(id, lay)), info = id)
      if (rfc) expect_true(is.na(uuid_timestamp(id)), info = id)
    }
    if (is.null(row$unix_millis)) next
    ms <- row$unix_millis
    expect_identical(as.integer(row$version), 7L)
    expect_equal(uuid_v7_unix_millis(id, lay), ms, tolerance = 0, info = id)
    if (rfc) expect_equal(uuid_v7_unix_millis(id), ms, tolerance = 0, info = id)
    if (minted) {
      expect_equal(round(as.numeric(uuid_timestamp(id, lay)) * 1000), ms, tolerance = 0, info = id)
      if (rfc) expect_equal(round(as.numeric(uuid_timestamp(id)) * 1000), ms, tolerance = 0, info = id)
    }
  }
})

test_that("inspect.json", {
  rows <- read_vectors("inspect.json")
  expect_gt(length(rows), 0)
  for (row in rows) {
    lay <- layout_of(row)
    id <- row$uuid
    v <- as.integer(row$version)
    expect_identical(uuid_version(id, lay), v, info = id)
    expect_identical(uuid_is_rfc(id, v, lay), row$is_rfc, info = id)
    for (other in setdiff(0:15, v)) {
      expect_false(uuid_is_rfc(id, other, lay), info = paste(id, other))
    }
    if (!is.null(row$variant)) {
      expect_identical(uuid_variant(id), row$variant, info = id)
      expect_identical(uuid_version(id), v, info = id)
      expect_identical(uuid_is_rfc(id, v), row$is_rfc, info = id)
    }
  }
})
