dns_example <- "2ed6657d-e927-568b-95e1-2665a8aea6a2"

test_that("uuid_v5 matches the RFC 9562 example", {
  expect_identical(uuid_v5(uuid_namespaces$dns, "www.example.com"), dns_example)
  expect_identical(uuid_v5(uuid_namespaces$dns, list(charToRaw("www.example.com"))), dns_example)
})

test_that("uuid_v5 is vectorised over names, and a missing name is NA", {
  out <- uuid_v5(uuid_namespaces$url, c("a", "b", NA))
  expect_length(out, 3L)
  expect_false(anyNA(out[1:2]))
  expect_true(is.na(out[3]))
  expect_identical(uuid_version(out[1:2]), c(5L, 5L))
})

test_that("a raw name list takes only raw elements", {
  expect_error(uuid_v5(uuid_namespaces$dns, list("a")), "name must be a list of raw vectors",
               fixed = TRUE)
  expect_identical(length(uuid_v5(uuid_namespaces$dns, list())), 0L)
})

test_that("the namespace is an argument and errors", {
  expect_error(uuid_v5(NA, "x"), "namespace is NA", fixed = TRUE)
  expect_error(uuid_v5("nope", "x"), "namespace is not a UUID", fixed = TRUE)
  expect_error(uuid_v5(c(uuid_namespaces$dns, uuid_namespaces$url), "x"),
               "namespace must be a single UUID", fixed = TRUE)
  expect_error(uuid_v5(character(0), "x"), "namespace must be a single UUID", fixed = TRUE)
})

test_that("a missing name is data and propagates NA", {
  expect_identical(uuid_v5(uuid_namespaces$dns, NA_character_), NA_character_)
})

test_that("uuid_namespaces holds the four RFC 9562 namespaces", {
  expect_identical(names(uuid_namespaces), c("dns", "url", "oid", "x500"))
  expect_identical(uuid_namespaces$dns, "6ba7b810-9dad-11d1-80b4-00c04fd430c8")
  expect_identical(uuid_namespaces$x500, "6ba7b814-9dad-11d1-80b4-00c04fd430c8")
  expect_true(all(unlist(lapply(uuid_namespaces, uuid_version)) == 1L))
})
