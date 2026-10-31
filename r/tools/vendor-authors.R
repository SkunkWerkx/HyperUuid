# Writes inst/AUTHORS: one line per vendored crate (the dependency closure of the rcore
# wrapper, without rcore itself and the hyperuuid core, which is this project's own), from
# cargo metadata. Run from the package root by tools/vendor.sh, which leaves src/rust/vendor
# in place for it: the metadata is read offline against that directory, since the lock names
# the crates.io source for the core at a version that is not published while a release packs
# this package.
# The metadata is a single JSON line of about 60 KB, and it goes through a file: on Windows
# `system2(stdout = TRUE)` splits captured lines at 8095 bytes, and the break lands inside a
# string.
metadata <- tempfile(fileext = ".json")
vendored <- c("--offline",
              "--config", shQuote('source.crates-io.replace-with="vendored-sources"'),
              "--config", shQuote('source.vendored-sources.directory="src/rust/vendor"'))
status <- system2("cargo", c("metadata", "--format-version", "1", "--locked", vendored,
                             "--manifest-path", "src/rust/Cargo.toml"), stdout = metadata)
if (status != 0L) stop("vendor-authors.R: cargo metadata exited with status ", status)
meta <- jsonlite::fromJSON(metadata, simplifyVector = FALSE)
pkgs <- Filter(function(p) !p$name %in% c("rcore", "hyperuuid"), meta$packages)
pkgs <- pkgs[order(vapply(pkgs, function(p) p$name, ""))]
# Crates whose Cargo.toml declares no authors, credited as their own license files name them.
undeclared <- c(
  libc = "The Rust Project Developers",
  typenum = "Paho Lurie-Gregg",
  `r-efi` = paste("The r-efi contributors named in its AUTHORS file (copyright Red Hat, Inc.,",
                  "Microsoft Corporation and David Rheinsberg)")
)
field <- function(x, missing) {
  if (is.null(x) || !length(x)) missing else paste(unlist(x), collapse = ", ")
}
credit <- function(p) {
  if (is.null(p$authors) || !length(p$authors)) {
    if (!p$name %in% names(undeclared)) {
      stop("vendor-authors.R: ", p$name, " declares no authors; add it to `undeclared`")
    }
    return(undeclared[[p$name]])
  }
  paste(unlist(p$authors), collapse = ", ")
}
lines <- vapply(pkgs, function(p) {
  sprintf("%s %s: %s (%s, %s)", p$name, p$version, credit(p),
          field(p$license, "no license field"), field(p$repository, "no repository field"))
}, "")
dir.create("inst", showWarnings = FALSE)
writeLines(lines, "inst/AUTHORS", useBytes = TRUE)
