v7_re <- "^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"
v4_re <- "^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"
t <- as.POSIXct(1645557742, origin = "1970-01-01", tz = "UTC")
prefix <- function(x) substr(x, 1, 13)

test_that("uuid_v4 returns random version 4 strings", {
  id <- uuid_v4()
  expect_type(id, "character")
  expect_length(id, 1L)
  expect_match(id, v4_re)
  expect_false(anyDuplicated(uuid_v4(1000)) > 0)
  expect_identical(uuid_v4(0), character(0))
})

test_that("uuid_v4_raw is a 16 x n raw matrix", {
  r <- uuid_v4_raw(3)
  expect_type(r, "raw")
  expect_identical(dim(r), c(16L, 3L))
  expect_true(all(bitwShiftR(as.integer(r[7, ]), 4L) == 4L))
  expect_identical(dim(uuid_v4_raw(0)), c(16L, 0L))
})

test_that("uuid_v7 embeds the requested time", {
  expect_match(uuid_v7(time = t), v7_re)
  expect_identical(prefix(uuid_v7(time = t)), "017f22e2-79b0")
  expect_identical(prefix(uuid_v7(time = 1645557742)), "017f22e2-79b0")
  expect_identical(prefix(uuid_v7(time = as.POSIXlt(t))), "017f22e2-79b0")
  expect_identical(prefix(uuid_v7(time = as.Date("2022-02-22"))), "017f1eba-4c00")
  expect_identical(
    prefix(uuid_v7(2, time = c(t, t + 1))),
    c("017f22e2-79b0", "017f22e2-7d98")
  )
})

test_that("a batch is strictly increasing and unique", {
  ids <- uuid_v7(1000, time = t)
  expect_false(is.unsorted(ids, strictly = TRUE))
  expect_false(anyDuplicated(ids) > 0)
  expect_identical(uuid_v7(0), character(0))
})

test_that("bad arguments error with pinned messages", {
  expect_error(uuid_v7(3, time = c(t, t)), "time must have length 1 or n (3), not 2", fixed = TRUE)
  expect_error(uuid_v7(time = NA), "time element 1 is NA or not finite", fixed = TRUE)
  expect_error(uuid_v7(time = TRUE), "time must be a POSIXct, POSIXlt, Date or numeric",
               fixed = TRUE)
  expect_error(uuid_v7(2, time = c(TRUE, NA)), "time must be a POSIXct", fixed = TRUE)
  expect_error(uuid_v7(time = Inf), "time element 1 is NA or not finite", fixed = TRUE)
  expect_error(uuid_v7_raw(2, time = c(t, NA)), "time element 2 is NA or not finite", fixed = TRUE)
  expect_error(
    uuid_v7(uuid_v7_max_batch + 1, time = t),
    "batch count exceeds the 26-bit counter space (67108864 per call)",
    fixed = TRUE
  )
  expect_error(uuid_v7(-1), "n must be a single non-negative whole number", fixed = TRUE)
  expect_error(uuid_v7(1.5), "n must be a single non-negative whole number", fixed = TRUE)
  expect_error(uuid_v4(-1), "n must be a single non-negative whole number", fixed = TRUE)
  expect_error(
    uuid_v7(time = -1),
    "unix_millis must be non-negative and at most 253402300799999 (9999-12-31T23:59:59.999Z)",
    fixed = TRUE
  )
})

test_that("v7 timestamps stop at the last millisecond of 9999 in both directions", {
  # The cap is the last instant .NET's DateTime and Python's datetime can name. One millisecond
  # more errors; a version 7 another generator dated past the cap has no timestamp at
  # uuid_timestamp, as a version 6 has none, while uuid_v7_unix_millis trusts the caller and
  # reads the 48-bit field as it is.
  last <- as.POSIXct("9999-12-31 23:59:59.999", tz = "UTC")
  expect_equal(uuid_v7_unix_millis(uuid_v7(time = last)), 253402300799999, tolerance = 0)
  expect_equal(uuid_v7_unix_millis(uuid_v7(time = 253402300799.999)), 253402300799999, tolerance = 0)
  expect_equal(as.numeric(uuid_timestamp(uuid_v7(time = last))), as.numeric(last), tolerance = 1e-3)
  expect_error(
    uuid_v7(time = 253402300800),
    "unix_millis must be non-negative and at most 253402300799999 (9999-12-31T23:59:59.999Z)",
    fixed = TRUE
  )
  past <- "ffffffff-ffff-7000-8000-010203040506"
  expect_true(uuid_is_rfc(past, 7))
  expect_true(is.na(uuid_timestamp(past)))
  expect_equal(uuid_v7_unix_millis(past), 281474976710655, tolerance = 0)
})

test_that("uuid_v7_raw matches the string form's shape", {
  # Earlier tests moved the process-wide stream to t + 1 s; a time over 11 s behind it is
  # minted out of stream and carries its own millisecond exactly.
  r <- uuid_v7_raw(2, time = t - 60)
  expect_identical(dim(r), c(16L, 2L))
  hex <- apply(r, 2L, function(col) paste(sprintf("%02x", as.integer(col)), collapse = ""))
  expect_identical(substr(hex, 1, 12), rep("017f22e18f50", 2L))
  expect_identical(substr(hex, 13, 13), c("7", "7"))
})

test_that("uuid_v7_max_batch is the 26-bit counter space", {
  expect_identical(uuid_v7_max_batch, 67108864L)
})

test_that("counter exhaustion errors", {
  # base::, because load_all shims system.file() to answer for the source tree. The child
  # loads whatever copy is installed, so install this checkout first: an older copy runs the
  # loop below without an error and fails as "batch site: got NULL".
  skip_if(!nzchar(base::system.file(package = "hyperuuid", lib.loc = .libPaths())),
          "needs an installed hyperuuid; the child process loads it")
  script <- tempfile(fileext = ".R")
  on.exit(unlink(script), add = TRUE)
  # The child is told the library paths in its script, not through system2(env =), which
  # Windows does not apply to Rscript.
  writeLines(c(
    paste0(".libPaths(", paste(deparse(.libPaths()), collapse = ""), ")"),
    "library(hyperuuid)",
    "msg <- paste0('version 7 counter exhausted: the stream cannot advance without running ',",
    "  'more than 1000 ms ahead of the supplied timestamp or past its last millisecond')",
    "m <- .POSIXct(17592186044.414, tz = 'UTC')",
    "fails <- function(expr) tryCatch({ expr; NULL }, error = function(e) conditionMessage(e))",
    "must <- function(got, where) {",
    "  if (!identical(got, msg)) stop(where, ': got ', deparse(got), call. = FALSE)",
    "}",
    "invisible(uuid_v7(time = m))",
    "got <- NULL",
    "for (i in 1:1025) if (!is.null(got <- fails(uuid_v7_raw(65536, time = m)))) break",
    "must(got, 'batch site')",
    "got <- NULL",
    "for (i in 1:1024) if (!is.null(got <- fails(uuid_v7(time = m)))) break",
    "must(got, 'batch site, count 1')",
    "must(fails(uuid_v7(2, time = c(m, m))), 'single site, uuid_v7 per-element')",
    "must(fails(uuid_v7_raw(2, time = c(m, m))), 'single site, uuid_v7_raw per-element')",
    "must(fails(uuid_v7(3, time = m)), 'uuid_v7 batch')",
    "must(fails(uuid_v7_raw(1, time = m)), 'uuid_v7_raw')",
    "cat('OK\\n')"
  ), script)
  out <- suppressWarnings(system2(
    file.path(R.home("bin"), "Rscript"), c("--vanilla", shQuote(script)),
    stdout = TRUE, stderr = TRUE
  ))
  expect_null(attr(out, "status"), info = paste(out, collapse = "\n"))
  expect_identical(out[length(out)], "OK")
})
