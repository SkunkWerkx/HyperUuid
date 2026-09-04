// Local-dev-only proof that UuidGenerator's FFM downcalls survive GraalVM Native Image
// ahead-of-time compilation — mirrors csharp/HyperUuid.AotSmokeTest (also not wired into
// CI, just something a developer runs by hand): `./gradlew :aot-smoke-test:nativeRun`.
plugins {
    application
    id("org.graalvm.buildtools.native") version "1.1.10"
}

repositories {
    mavenCentral()
}

dependencies {
    implementation(rootProject)
    // -Pwasm: the same smoke test with GraalWasm on the classpath and the binary run with
    // -Dhyperuuid.backend=wasm, so the wasm path is proven under Native Image by a task
    // anyone can rerun rather than by the one-off hand build the README's 181 ns row came
    // from — the library's own reachability-metadata.json (the WasmBackend reflection entry,
    // the native/*/* resource glob) is what has to carry it, exactly as for the FFM path.
    // Off by default: the plain run keeps proving the FFM path with nothing extra linked in.
    if (project.hasProperty("wasm")) {
        runtimeOnly("org.graalvm.polyglot:polyglot:25.3.4.1")
        runtimeOnly("org.graalvm.polyglot:wasm:25.3.4.1")
    }
}

application {
    mainClass.set("io.github.skunkwerkx.hyperuuid.aotsmoketest.Main")
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
            if (project.hasProperty("wasm")) {
                runtimeArgs.add("-Dhyperuuid.backend=wasm")
            }
            // Deliberately no resources.includedPatterns override here anymore: the
            // hyperuuid library's own packaged META-INF/native-image/.../
            // reachability-metadata.json now carries a resources glob covering
            // native/*/* itself, and this module needs to prove that packaged metadata is
            // actually sufficient on its own -- a local override here would mask a real
            // regression in the packaged metadata the way it already did once (a missing
            // foreign.downcalls entry built clean locally and only crashed at runtime,
            // undetected until this smoke test was finally run for real).
        }
    }
}
