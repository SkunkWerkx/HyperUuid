// Local-dev-only proof that UuidGenerator's FFM downcalls survive GraalVM Native Image
// ahead-of-time compilation — mirrors csharp/HyperUuid.AotSmokeTest. Not wired into CI on
// the Java side, just something a developer runs by hand under a GraalVM JAVA_HOME:
// `./gradlew :aot-smoke-test:nativeRun`. `./gradlew :aot-smoke-test:run` is the same program
// on an ordinary JVM, and `:aot-smoke-test:runFloor` the same on the floor JDK, where the
// jar's JNI base loads instead. The image keeps only the FFM path and no JNI library.
plugins {
    application
    id("org.graalvm.buildtools.native") version "1.1.14"
}

repositories {
    mavenCentral()
}

dependencies {
    implementation(rootProject)
}

application {
    mainClass.set("io.github.skunkwerkx.hyperuuid.aotsmoketest.Main")
    // `:aot-smoke-test:run` is the same program on an ordinary JVM, with the flag a consumer
    // passes (README.md's "Native access").
    applicationDefaultJvmArgs = listOf("--enable-native-access=ALL-UNNAMED")
}

// The library's own floor, from the root project so the two cannot drift. Gradle and every
// compiler stay on 25 (a JDK 21 javac rejects the library's java25 tree), so only the
// launcher below runs on the floor.
val jdkFloor: Int by rootProject.extra

java {
    sourceCompatibility = JavaVersion.toVersion(jdkFloor)
    targetCompatibility = JavaVersion.toVersion(jdkFloor)
}

// The library's own floor, enforced the same way it is there.
tasks.withType<JavaCompile>().configureEach {
    options.release = jdkFloor
}

// `run` on the floor JDK, through the toolchain: the jar's JNI base end to end, the same
// launcher as the library's testFloor.
tasks.register<JavaExec>("runFloor") {
    group = "application"
    description = "Runs the smoke test on the floor JDK ($jdkFloor)."
    classpath = sourceSets.main.get().runtimeClasspath
    mainClass = application.mainClass
    jvmArgs("--enable-native-access=ALL-UNNAMED")
    javaLauncher = javaToolchains.launcherFor {
        languageVersion = JavaLanguageVersion.of(jdkFloor)
    }
}

graalvmNative {
    binaries {
        named("main") {
            // Same restricted-method opt-in as the library's own test task, plus a build
            // report so a failed reachability/linking analysis is diagnosable from the build
            // output rather than just a bare non-zero exit code.
            buildArgs.add("--enable-native-access=ALL-UNNAMED")
            buildArgs.add("-H:+ReportExceptionStackTraces")
            // Deliberately no resources.includedPatterns override here: the hyperuuid
            // library's own packaged META-INF/native-image/.../reachability-metadata.json
            // carries the three core-library globs itself (never the JNI shim's), and this
            // module needs to prove that packaged metadata is sufficient on its own -- a
            // local override here would mask a regression in the packaged metadata (a
            // missing foreign.downcalls entry builds clean locally and only crashes at
            // runtime).
        }
    }
}
