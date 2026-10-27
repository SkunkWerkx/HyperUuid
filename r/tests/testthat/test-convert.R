v7_id <- "017f22e2-79b0-7cc3-98c4-dc0c0c07398f"

test_that("uuid_to_raw gives a 16 x n raw matrix", {
  m <- uuid_to_raw(v7_id)
  expect_type(m, "raw")
  expect_equal(dim(m), c(16L, 1L))
  expect_equal(as.integer(m[1:4, 1]), c(0x01, 0x7f, 0x22, 0xe2))
  two <- uuid_to_raw(c(v7_id, v7_id))
  expect_identical(two[, 2], uuid_to_raw(v7_id)[, 1])
})

test_that("text comes back canonical", {
  expect_identical(uuid_from_raw(uuid_to_raw(v7_id)), v7_id)
  expect_identical(uuid_from_raw(uuid_to_raw(toupper(v7_id))), v7_id)
  expect_identical(uuid_from_raw(uuid_to_raw(gsub("-", "", v7_id))), v7_id)
})

test_that("the raw doors are strict", {
  expect_error(uuid_to_raw(NA_character_), "uuid_to_raw(): element 1 is NA", fixed = TRUE)
  expect_error(uuid_to_raw(c(v7_id, "nope")), "uuid_to_raw(): element 2 is not a UUID",
               fixed = TRUE)
  expect_error(uuid_to_raw("nope"), "uuid_to_raw(): element 1 is not a UUID", fixed = TRUE)
  expect_error(uuid_to_raw(1), "x must be a character vector of UUIDs", fixed = TRUE)
  expect_identical(uuid_to_raw(uuid_nil), matrix(as.raw(0), 16, 1))
  expect_identical(uuid_from_raw(matrix(as.raw(0), 16, 1)), uuid_nil)
})

test_that("empty input round-trips", {
  e <- uuid_to_raw(character(0))
  expect_type(e, "raw")
  expect_equal(dim(e), c(16L, 0L))
  expect_identical(uuid_from_raw(raw(0)), character(0))
  expect_identical(uuid_from_raw(e), character(0))
})

test_that("uuid_from_raw takes a plain vector and rejects bad shapes", {
  expect_length(uuid_from_raw(as.raw(1:32)), 2L)
  msg <- paste("uuid_from_raw(): expected a raw matrix with 16 rows or a raw vector whose",
               "length is a multiple of 16")
  expect_error(uuid_from_raw(as.raw(1:17)), msg, fixed = TRUE)
  expect_error(uuid_from_raw(matrix(as.raw(0), 8, 2)), msg, fixed = TRUE)
  expect_error(uuid_from_raw("x"), msg, fixed = TRUE)
})
