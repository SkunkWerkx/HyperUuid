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
    "unix_millis must be non-negative and fit within 48 bits",
    fixed = TRUE
  )
})

test_that("uuid_v7_raw matches the string form's shape", {
  r <- uuid_v7_raw(2, time = t)
  expect_identical(dim(r), c(16L, 2L))
  hex <- apply(r, 2L, function(col) paste(sprintf("%02x", as.integer(col)), collapse = ""))
  expect_identical(substr(hex, 1, 12), rep("017f22e279b0", 2L))
  expect_identical(substr(hex, 13, 13), c("7", "7"))
})

test_that("uuid_v7_max_batch is the 26-bit counter space", {
  expect_identical(uuid_v7_max_batch, 67108864L)
})
