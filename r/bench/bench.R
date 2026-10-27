# hyperuuid against the uuid package, one id and 1000 ids per call, for v4 and v7.
# Run from the package root against an installed copy:
#   R CMD INSTALL --library=<lib> . && R_LIBS=<lib> Rscript bench/bench.R
# Results are machine-specific: the header names the date, R and the machine.
library(hyperuuid)
if (!requireNamespace("uuid", quietly = TRUE) || !requireNamespace("bench", quietly = TRUE)) {
  stop("bench/bench.R needs the uuid and bench packages: install.packages(c(\"uuid\", \"bench\"))")
}

machine <- function() {
  if (Sys.info()[["sysname"]] == "Darwin") {
    cpu <- system("sysctl -n machdep.cpu.brand_string", intern = TRUE)
    model <- system("sysctl -n hw.model", intern = TRUE)
    return(sprintf("%s (%s)", cpu, model))
  }
  if (file.exists("/proc/cpuinfo")) {
    model <- grep("^model name", readLines("/proc/cpuinfo"), value = TRUE)
    if (length(model) > 0) return(sub("^model name\\s*:\\s*", "", model[[1]]))
  }
  Sys.info()[["machine"]]
}

cat(sprintf("%s, %s, %s\n", format(Sys.Date()), R.version.string, machine()))
cat(sprintf("hyperuuid %s, uuid %s, bench %s\n\n", packageVersion("hyperuuid"),
            packageVersion("uuid"), packageVersion("bench")))

results <- rbind(
  bench::mark(
    `hyperuuid::uuid_v4()` = hyperuuid::uuid_v4(),
    `uuid::UUIDgenerate()` = uuid::UUIDgenerate(),
    `hyperuuid::uuid_v7()` = hyperuuid::uuid_v7(),
    `uuid::UUIDgenerate(use.time = TRUE)` = uuid::UUIDgenerate(use.time = TRUE),
    check = FALSE
  ),
  bench::mark(
    `hyperuuid::uuid_v4(1000)` = hyperuuid::uuid_v4(1000),
    `uuid::UUIDgenerate(n = 1000)` = uuid::UUIDgenerate(n = 1000),
    `hyperuuid::uuid_v7(1000)` = hyperuuid::uuid_v7(1000),
    `uuid::UUIDgenerate(use.time = TRUE, n = 1000)` =
      uuid::UUIDgenerate(use.time = TRUE, n = 1000),
    check = FALSE
  )
)

table <- data.frame(
  expression = as.character(results$expression),
  median = format(results$median),
  `itr/sec` = round(results$`itr/sec`),
  mem_alloc = format(results$mem_alloc),
  check.names = FALSE
)
print(table, row.names = FALSE, right = FALSE)
