# Converts `time` (POSIXct, POSIXlt, Date, or numeric seconds since the Unix epoch) to
# whole Unix milliseconds, a double of length 1 or n. NA and non-finite elements error here;
# the range check, 0 to 253402300799999 (9999-12-31T23:59:59.999Z), lives in C.
unix_millis <- function(time, n) {
  if (inherits(time, "POSIXlt")) {
    time <- as.POSIXct(time)
  } else if (inherits(time, "Date")) {
    time <- as.POSIXct(time, tz = "UTC") # midnight UTC, not local midnight
  }
  # A logical is accepted only as missing (a bare NA), so it reaches the NA error below.
  if (!(inherits(time, "POSIXct") || is.numeric(time) ||
          (is.logical(time) && all(is.na(time)))) || is.factor(time)) {
    stop("time must be a POSIXct, POSIXlt, Date or numeric", call. = FALSE)
  }
  secs <- as.numeric(time)
  if (length(secs) != 1L && length(secs) != n) {
    stop(sprintf("time must have length 1 or n (%d), not %d", n, length(secs)), call. = FALSE)
  }
  bad <- which(!is.finite(secs))
  if (length(bad) > 0L) {
    stop(sprintf("time element %d is NA or not finite", bad[[1L]]), call. = FALSE)
  }
  floor(secs * 1000)
}

# Validates a count argument: one non-negative whole number that fits an R integer.
as_count <- function(n) {
  if (!is.numeric(n) || length(n) != 1L || is.na(n) || !is.finite(n) || n < 0 ||
        n != floor(n) || n > .Machine$integer.max) {
    stop("n must be a single non-negative whole number", call. = FALSE)
  }
  as.integer(n)
}
