test_that("the native version is the package version", {
  expect_s3_class(hyperuuid_native_version(), "package_version")
  expect_equal(hyperuuid_native_version(), packageVersion("hyperuuid"))
})
