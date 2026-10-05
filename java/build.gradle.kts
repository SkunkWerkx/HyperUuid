import org.gradle.external.javadoc.StandardJavadocDocletOptions

plugins {
    `java-library`
    `maven-publish`
    id("com.vanniktech.maven.publish") version "0.37.0"
    id("com.diffplug.spotless") version "8.10.3"
}

// io.github.skunkwerkx — the SkunkWerkx org's own auto-verified Central Portal namespace
// (approved by Central Support after an email request; io.github.buvinghausen was the interim
// personal-account namespace used for the very first real Maven Central publish, proving the
// token auth + GPG signing path end to end — that coordinate stays live on Central permanently
// (no delete), this is where every publish from here on happens).
group = "io.github.skunkwerkx"
// CI overrides this (0.1.0-ci.<run_number>) via HYPERUUID_VERSION so repeated manual
// workflow_dispatch runs during testing don't collide with an already-published version —
// the real Maven Central publish (release.yml, tag-triggered) never sets that env var, so
// it always uses this committed version as-is.
version = System.getenv("HYPERUUID_VERSION") ?: "0.6.1"

repositories {
    mavenCentral()
}

// The formatter, over every Java file in the build: the library, its tests, the AOT smoke
// test and the JMH benchmarks. palantir-java-format is google-java-format's rules at a
// 4-space indent and 120 columns, the shape this code was already written in.
// `./gradlew spotlessApply` formats; `./gradlew spotlessCheck` (CI) fails on any drift.
spotless {
    java {
        target("src/**/*.java", "aot-smoke-test/src/**/*.java", "benchmarks/src/**/*.java")
        palantirJavaFormat("2.102.0")
    }
}

// GraalWasm, the wasm backend's runtime, is deliberately compileOnly: this jar's POM carries no
// dependency on it, so a consumer on the default FFM path downloads nothing extra. Opting into
// the wasm path means adding both artifacts (polyglot for the API, wasm for the engine — a
// POM-type dependency that fans out into Truffle) to their own build; see README.md's
// WebAssembly section. Tests get both on the runtime classpath so the whole suite can run a
// second time through the wasm module (the testWasm task below).
val graalPolyglotVersion = "25.4.4.1.1"

dependencies {
    compileOnly("org.graalvm.polyglot:polyglot:$graalPolyglotVersion")
    testRuntimeOnly("org.graalvm.polyglot:polyglot:$graalPolyglotVersion")
    testRuntimeOnly("org.graalvm.polyglot:wasm:$graalPolyglotVersion")
    testImplementation(platform("org.junit:junit-bom:6.1.3"))
    testImplementation("org.junit.jupiter:junit-jupiter")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

// Local dev loop, ported from HyperCast (which had it from its first release, mirroring the
// C# csproj's copy of the freshly-built core): when the Rust cdylib exists in-repo
// (`cargo cdylib` in ../rust), stage it as the classpath resource
// /native/{rid}/{lib} the loader expects, so `./gradlew test` needs nothing copied by hand.
// CI overlays every platform's build into the same layout before packaging.
//
// The same resolution NativePlatform.java does at runtime, so the library lands under the
// RID the loader will ask for: x64 or arm64 only, and on Linux the musl family when this
// (Gradle's own) JVM has musl's loader mapped — `cargo cdylib` on Alpine produces a musl
// library, and it has to be staged as linux-musl-*. Anything else stages nothing, and the
// suite then runs through the wasm module exactly as a consumer on that platform would.
val nativeRid = run {
    val osName = System.getProperty("os.name").lowercase()
    val arch = when (System.getProperty("os.arch").lowercase()) {
        "amd64", "x86_64", "x64" -> "x64"
        "aarch64", "arm64" -> "arm64"
        else -> null
    }
    val musl = runCatching {
        file("/proc/self/maps").readLines(Charsets.ISO_8859_1)
            .any { it.contains("ld-musl-") || it.contains("libc.musl-") }
    }.getOrDefault(false)
    when {
        arch == null -> "unsupported"
        osName.startsWith("windows") -> "win-$arch"
        osName.startsWith("mac") || osName.startsWith("darwin") -> "osx-$arch"
        osName.startsWith("linux") -> if (musl) "linux-musl-$arch" else "linux-$arch"
        else -> "unsupported"
    }
}

// Only when this platform's library has NOT been placed under src/main/resources
// explicitly. The forge builds the PyO3 extension (the `python` feature's cdylib) into the
// same rust/target/release/ BEFORE the Java leg runs, so staging from there in CI would put
// an extension with unresolved Py* imports beside the correctly placed one (HyperCast's
// first collapsed-job run failed every Linux leg exactly so). Explicit placement is the
// signal that the right bytes are already on the classpath.
//
// A Sync that stages nothing, rather than a Copy that is skipped: a skipped task leaves
// whatever it staged last time in generated-resources, and the moment a library is placed
// explicitly on top of that, processResources fails on the duplicate entry (found by
// staging, then placing, in one working tree). Sync removes what it did not stage.
val nativePlaced = file("src/main/resources/native/$nativeRid").exists()
val stageNativeLibrary = tasks.register<Sync>("stageNativeLibrary") {
    from("../rust/target/release") {
        include("libhyperuuid.so", "libhyperuuid.dylib", "hyperuuid.dll")
        if (nativePlaced || nativeRid == "unsupported") {
            exclude("**")
        }
    }
    into(layout.buildDirectory.dir("generated-resources/native/$nativeRid"))
}

// The same dev loop for the wasm32-wasip1 module the GraalWasm backend runs: a
// `cargo wasm-module` in ../rust (from inside rust/, so its
// .cargo/config.toml export flags apply) lands at /native/wasm32-wasip1/hyperuuid.wasm on
// the classpath, beside the platform library. Same explicit-placement yield as above.
val wasmPlaced = file("src/main/resources/native/wasm32-wasip1").exists()
val stageWasmModule = tasks.register<Sync>("stageWasmModule") {
    from("../rust/target/wasm32-wasip1/release") {
        include("hyperuuid.wasm")
        if (wasmPlaced) {
            exclude("**")
        }
    }
    into(layout.buildDirectory.dir("generated-resources/native/wasm32-wasip1"))
}

sourceSets.main {
    resources.srcDir(layout.buildDirectory.dir("generated-resources"))
}

tasks.processResources {
    dependsOn(stageNativeLibrary, stageWasmModule)
}

// sourcesJar packages the main source set, and `generated-resources` is one of its resource
// dirs (above) — so it reads the staging tasks' output too, and Gradle fails the build
// outright on the undeclared dependency rather than risk a task-order-dependent jar. Only
// the publish path builds sourcesJar (`./gradlew test` never does), which is how HyperCast
// first hit this against Maven Central and not in CI. withType/configureEach rather than
// tasks.named("sourcesJar"): the sources and javadoc jars are registered by the vanniktech
// publish plugin, so they don't exist yet at this point in configuration.
tasks.withType<Jar>().configureEach {
    dependsOn(stageNativeLibrary, stageWasmModule)
}

// Ships the license text and this binding's README inside the jar, under META-INF/ (the
// conventional home for both). Gradle copies from anywhere on disk, so the repo root's
// LICENSE is referenced directly — no local copy, unlike the gem and the wheel, whose
// packers both reject a parent path outright. The POM's <licenses> block stays the
// machine-readable declaration; this is the text itself, for consumers who vendor the jar.
tasks.jar {
    metaInf {
        from("../LICENSE")
        from("README.md")
    }
    // A stable module name for a consumer on the module path — without it the name is
    // derived from the jar's file name — and therefore something exact to hand
    // --enable-native-access (README.md's "Native access" section). Not a module-info.java:
    // GraalWasm is an optional dependency this jar has to load without, and a module
    // descriptor would have to declare it one way or the other.
    manifest {
        attributes("Automatic-Module-Name" to "io.github.skunkwerkx.hyperuuid")
    }
}

tasks.test {
    useJUnitPlatform()
    // UuidGenerator's FFM downcalls are a "restricted method" — silences the runtime
    // warning today and avoids them being blocked outright in a future JDK.
    jvmArgs("--enable-native-access=ALL-UNNAMED")
    // This binding's own version, so the suite can pin UuidGenerator.nativeVersion() to it:
    // the core and the jar move together (prepare-release.yml bumps both), and the probe
    // exists to prove exactly that.
    systemProperty("hyperuuid.version", version)
}

// The identical suite, forced through the GraalWasm backend (-Dhyperuuid.backend=wasm), so
// both interop paths are held to the same assertions on every build. --enable-native-access
// is for Truffle's own System.load, not this binding; WarnInterpreterOnly=false silences the
// engine's fallback-runtime notice on a non-GraalVM JDK, which is what CI and most dev boxes
// run — the numbers in README.md say what that fallback costs, this just keeps the test log
// readable.
val testWasm = tasks.register<Test>("testWasm") {
    description = "Runs the test suite against the bundled wasm32-wasip1 module via GraalWasm."
    group = "verification"
    testClassesDirs = sourceSets.test.get().output.classesDirs
    classpath = sourceSets.test.get().runtimeClasspath
    useJUnitPlatform()
    jvmArgs("--enable-native-access=ALL-UNNAMED", "-Dpolyglot.engine.WarnInterpreterOnly=false")
    systemProperty("hyperuuid.backend", "wasm")
    systemProperty("hyperuuid.version", version)
    shouldRunAfter(tasks.test)
}

tasks.check {
    dependsOn(testWasm)
}

java {
    // 25 is the floor: the first long-term-support JDK with the final java.lang.foreign API
    // (JEP 454 finalized it in 22, and 22 through 24 are all past end of life). Only
    // upstream-supported runtimes, the same rule every other binding in this repo holds to.
    sourceCompatibility = JavaVersion.VERSION_25
    targetCompatibility = JavaVersion.VERSION_25
    withSourcesJar()
}

// --release, not just the -source/-target pair above: it also compiles against JDK 25's own
// API signatures whatever JDK is running the build, so a newer JDK on a CI leg cannot let a
// newer API slip into a jar that claims 25.
tasks.withType<JavaCompile>().configureEach {
    options.release = 25
}

// javadoc's own doclint already flags a missing comment/@param/@return as a WARNING by
// default (that's how the 47 gaps that used to exist on this class's public surface were
// found) — -Xwerror promotes those warnings to build-failing errors, so an undocumented
// public member can't ship again silently. Central Portal requires a javadoc jar for every
// artifact anyway (see mavenPublishing below), so this is enforcing a real publish
// prerequisite, not just style.
tasks.javadoc {
    (options as StandardJavadocDocletOptions).addBooleanOption("Xwerror", true)
}

// mavenPublishing {} (com.vanniktech.maven.publish) owns the "maven" publication itself —
// sources/javadoc jars, POM, and the Central Portal repository target all come from here, not
// from a manually created MavenPublication (that would collide: the plugin creates one named
// "maven" too). publishToMavenCentral() targets the new Central Publisher Portal, not the
// dead OSSRH/Nexus staging API. Credentials (mavenCentralUsername/mavenCentralPassword, from
// the Central Portal's own token generator — not a raw Sonatype account password) and the
// signing key come from ORG_GRADLE_PROJECT_-prefixed env vars in CI,
// ~/.gradle/gradle.properties locally; neither lives in this file.
mavenPublishing {
    publishToMavenCentral()
    signAllPublications()

    pom {
        name.set("hyperuuid")
        description.set(
            "RFC 9562 UUID v4/v5/v6/v7 generation — high-performance FFM bindings " +
                "straight into a native Rust core (libhyperuuid) that never allocates. " +
                "JDK 25+. No runtime bridge, no extra dependency."
        )
        url.set("https://github.com/SkunkWerkx/HyperUuid")
        licenses {
            license {
                name.set("MIT")
                url.set("https://opensource.org/license/mit")
                distribution.set("repo")
            }
        }
        developers {
            developer {
                id.set("buvinghausen")
                name.set("Brian Buvinghausen")
                url.set("https://github.com/buvinghausen/")
            }
        }
        scm {
            url.set("https://github.com/SkunkWerkx/HyperUuid")
            connection.set("scm:git:git://github.com/SkunkWerkx/HyperUuid.git")
            developerConnection.set("scm:git:ssh://git@github.com/SkunkWerkx/HyperUuid.git")
        }
    }
}

publishing {
    repositories {
        // This repo's GitHub Packages Maven registry (private by default, repo-scoped —
        // github.com/SkunkWerkx/HyperUuid/packages). Credentials come from CI's own
        // GITHUB_ACTOR/GITHUB_TOKEN; empty locally, which only matters if you actually run
        // `./gradlew publish` (publishToMavenLocal doesn't touch this repository). Independent
        // of mavenPublishing {} above — this stays as a second, separate target on the same
        // "maven" publication, not a competing one.
        maven {
            name = "GitHubPackages"
            url = uri("https://maven.pkg.github.com/SkunkWerkx/HyperUuid")
            credentials {
                username = System.getenv("GITHUB_ACTOR")
                password = System.getenv("GITHUB_TOKEN")
            }
        }
    }
}
