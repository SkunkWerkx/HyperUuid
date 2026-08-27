rootProject.name = "hyperuuid"

// A local-dev-only GraalVM Native Image smoke test, mirroring csharp/HyperUuid.AotSmokeTest
// (also not wired into CI) — proves UuidGenerator's FFM downcalls survive ahead-of-time
// compilation to a real native binary, no JVM required to run it.
include(":aot-smoke-test")
