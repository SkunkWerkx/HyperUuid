// Local-dev-only proof that UuidGenerator's FFM downcalls survive GraalVM Native Image
// ahead-of-time compilation — mirrors csharp/HyperUuid.AotSmokeTest (also not wired into
// CI, just something a developer runs by hand): `./gradlew :aot-smoke-test:nativeRun`.
plugins {
    application
    id("org.graalvm.buildtools.native") version "1.1.10"
}

dependencies {
    implementation(rootProject)
}

application {
    mainClass.set("io.github.buvinghausen.hyperuuid.aotsmoketest.Main")
}

java {
    sourceCompatibility = JavaVersion.VERSION_22
    targetCompatibility = JavaVersion.VERSION_22
}

graalvmNative {
    binaries {
        named("main") {
            // Same restricted-method opt-in as the library's own test task, plus a build
            // report so a failed reachability/linking analysis is diagnosable from the build
            // output rather than just a bare non-zero exit code.
            buildArgs.add("--enable-native-access=ALL-UNNAMED")
            buildArgs.add("-H:+ReportExceptionStackTraces")
            // Native Image doesn't embed classpath resources by default — without this,
            // UuidGenerator's getResourceAsStream("/native/{rid}/{lib}") finds nothing at
            // runtime even though the build itself succeeds.
            resources {
                includedPatterns.add("native/.*")
            }
        }
    }
}
