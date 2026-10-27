v7_id <- "017f22e2-79b0-7cc3-98c4-dc0c0c07398f"
squash <- function(x) gsub("-", "", x)

test_that("corpus rows convert both ways", {
  expect_identical(squash(uuid_to_sql_order("00000000-0000-7000-8000-000000000000")),
                   "00000000000000808000000000000000")
  expect_identical(squash(uuid_from_sql_order("00000000000000808000000000000000")),
                   "00000000000070008000000000000000")
  expect_identical(squash(uuid_to_sql_order("00000000000070008007010203040506")),
                   "03040506010200878000000000000000")
  expect_identical(squash(uuid_from_sql_order("03040506010200878000000000000000")),
                   "00000000000070008007010203040506")
})

test_that("a v7 round-trips", {
  id <- uuid_v7(5)
  expect_identical(uuid_from_sql_order(uuid_to_sql_order(id)), id)
  expect_identical(uuid_version(uuid_to_sql_order(v7_id), layout = "sql"), 7L)
})

test_that("NA and malformed give NA", {
  out <- uuid_to_sql_order(c(v7_id, NA, "nope"))
  expect_false(is.na(out[1]))
  expect_true(all(is.na(out[2:3])))
  expect_true(all(is.na(uuid_from_sql_order(c(NA_character_, "nope")))))
})

test_that("a value of the wrong version errors", {
  msg <- "the value is not a UUID of the version this conversion takes"
  expect_error(uuid_to_sql_order(uuid_nil), msg, fixed = TRUE)
  expect_error(uuid_from_sql_order(uuid_nil), msg, fixed = TRUE)
  expect_error(uuid_to_sql_order(c(v7_id, NA, uuid_nil)), paste0(msg, " (element 3)"),
               fixed = TRUE)
})
