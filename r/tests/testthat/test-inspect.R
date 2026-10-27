v7_id <- "017f22e2-79b0-7cc3-98c4-dc0c0c07398f"
v1_id <- "c232ab00-9414-11ec-b3c8-9f6bdeced846"

test_that("uuid_version", {
  expect_identical(uuid_version(c(v7_id, v1_id, uuid_nil, NA, "nope")), c(7L, 1L, 0L, NA, NA))
  expect_identical(uuid_version(v7_id, "sql"), 0L)
  expect_identical(uuid_version(factor(v7_id)), 7L)
  expect_identical(uuid_version(character(0)), integer(0))
})

test_that("uuid_variant", {
  expect_identical(uuid_variant(c(v7_id, uuid_nil, uuid_max, NA)),
                   c("rfc9562", "ncs", "future", NA))
  expect_identical(uuid_variant("6ba7b810-9dad-11d1-80b4-00c04fd430c8"), "rfc9562")
  expect_identical(uuid_variant("6ba7b810-9dad-11d1-c0b4-00c04fd430c8"), "microsoft")
})

test_that("uuid_is_rfc", {
  expect_true(uuid_is_rfc(v7_id, 7))
  expect_false(uuid_is_rfc(v7_id, 4))
  expect_false(uuid_is_rfc(v7_id, 7, "sql"))
  expect_identical(uuid_is_rfc(NA, 7), NA)
  expect_error(uuid_is_rfc(v7_id, 16), "version")
  msg <- "version must be a single whole number from 0 to 15"
  expect_error(uuid_is_rfc(v7_id, c(7, 4)), msg, fixed = TRUE)
  expect_error(uuid_is_rfc(v7_id, 7.5), msg, fixed = TRUE)
  expect_error(uuid_is_rfc(v7_id, "7"), msg, fixed = TRUE)
  expect_true(uuid_is_rfc(v7_id, 7L))
})

test_that("uuid_timestamp and uuid_v7_unix_millis", {
  expect_equal(uuid_timestamp(v7_id), as.POSIXct(1645557742, origin = "1970-01-01", tz = "UTC"))
  expect_identical(attr(uuid_timestamp(v7_id), "tzone"), "UTC")
  expect_true(is.na(uuid_timestamp(v1_id)))
  expect_true(all(is.na(uuid_timestamp(c(NA, "nope")))))
  expect_s3_class(uuid_timestamp(character(0)), "POSIXct")
  expect_identical(uuid_v7_unix_millis(v7_id), 1645557742000)
  expect_identical(uuid_v7_unix_millis(c(NA, v7_id)), c(NA, 1645557742000))
})

test_that("bad arguments error", {
  expect_error(uuid_version(1), "x must be a character vector of UUIDs", fixed = TRUE)
  expect_error(uuid_version(v7_id, "nope"))
})
