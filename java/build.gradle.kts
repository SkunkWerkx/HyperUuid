import org.gradle.external.javadoc.StandardJavadocDocletOptions

plugins {
    `java-library`
    `maven-publish`
    id("com.vanniktech.maven.publish") version "0.37.0"
}

// io.github.skunkwerkx — the SkunkWerkx org's own auto-verified Central Portal namespace
// (approved by Central Support after an email request; io.github.buvinghausen was the interim
// personal-account namespace used for the very first real Maven Central publish, proving the
// token auth + GPG signing path end to end — that coordinate stays live on Central permanently
// (no delete), this is where every publish from here on happens).
group = "io.github.skunkwerkx"
// The real, committed version — same story as every other binding's manual bump: 0.0.1
// proves this coordinate's own first real Maven Central publish, ahead of the coordinated
// v0.1.0 release. CI overrides this (0.1.0-ci.<run_number>) via HYPERUUID_VERSION for repeated
// manual workflow_dispatch runs against the GitHub Packages feed during testing, so those
// don't collide with an already-published version — the real Maven Central publish
// (release.yml, tag-triggered) never sets that env var, so it always uses this committed
// version as-is.
version = System.getenv("HYPERUUID_VERSION") ?: "0.3.0"

repositories {
    mavenCentral()
}

// GraalWasm, the wasm backend's runtime, is deliberately compileOnly: this jar's POM carries no
// dependency on it, so a consumer on the default FFM path downloads nothing extra. Opting into
// the wasm path means adding both artifacts (polyglot for the API, wasm for the engine — a
// POM-type dependency that fans out into Truffle) to their own build; see README.md's
// WebAssembly section. Tests get both on the runtime classpath so the whole suite can run a
// second time through the wasm module (the testWasm task below).
val graalPolyglotVersion = "25.3.4.1"

dependencies {
    compileOnly("org.graalvm.polyglot:polyglot:$graalPolyglotVersion")
    testRuntimeOnly("org.graalvm.polyglot:polyglot:$graalPolyglotVersion")
    testRuntimeOnly("org.graalvm.polyglot:wasm:$graalPolyglotVersion")
    testImplementation(platform("org.junit:junit-bom:6.1.3"))
    testImplementation("org.junit.jupiter:junit-jupiter")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

// Local dev loop, ported from HyperCast (which had it from its first release, mirroring the
// C# csproj's copy of the freshly-built core): when the Rust cdylib exists in-repo (a
// release-profile cargo build in ../rust), stage it as the classpath resource
// /native/{rid}/{lib} the loader expects, so `./gradlew test` needs nothing copied by hand.
// CI overlays every platform's build into the same layout before packaging.
val nativeRid = run {
    val osName = System.getProperty("os.name").lowercase()
    val isArm = System.getProperty("os.arch").lowercase().let { it.contains("aarch64") || it.contains("arm") }
    when {
        osName.contains("win") -> if (isArm) "win-arm64" else "win-x64"
        osName.contains("mac") || osName.contains("darwin") -> if (isArm) "osx-arm64" else "osx-x64"
        else -> if (isArm) "linux-arm64" else "linux-x64"
    }
}

// Only when this platform's library has NOT been placed under src/main/resources
// explicitly. The forge builds the PyO3 extension (cargo build --features python) into the
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
        if (nativePlaced) {
            exclude("**")
        }
    }
    into(layout.buildDirectory.dir("generated-resources/native/$nativeRid"))
}

// The same dev loop for the wasm32-wasip1 module the GraalWasm backend runs: a
// `cargo build --release --target wasm32-wasip1` in ../rust (from inside rust/, so its
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
}

tasks.test {
    useJUnitPlatform()
    // UuidGenerator's FFM downcalls are a "restricted method" — silences the runtime
    // warning today and avoids them being blocked outright in a future JDK.
    jvmArgs("--enable-native-access=ALL-UNNAMED")
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
    shouldRunAfter(tasks.test)
}

tasks.check {
    dependsOn(testWasm)
}

java {
    // 22 (not 21, as the prior Kotlin build targeted): java.lang.foreign is a stable, non-
    // preview API only from JDK 22 (JEP 454) onward — plain javac (unlike kotlinc, which
    // doesn't gate on the JDK's own @PreviewFeature markers the same way) enforces that at
    // this project's own source/target level, not just the compiling JDK's.
    sourceCompatibility = JavaVersion.VERSION_22
    targetCompatibility = JavaVersion.VERSION_22
    withSourcesJar()
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
// dead OSSRH/Nexus staging API — io.github.skunkwerkx is now an approved, Central-Support-
// verified org namespace (io.github.buvinghausen was the interim personal-account namespace
// that auto-verified on its own, used only for this package's very first real publish).
// Credentials
// (mavenCentralUsername/mavenCentralPassword, from the Central Portal's own token generator —
// not a raw Sonatype account password) and the signing key come from
// ORG_GRADLE_PROJECT_-prefixed env vars in CI, ~/.gradle/gradle.properties locally; neither
// lives in this file.
mavenPublishing {
    publishToMavenCentral()
    signAllPublications()

    pom {
        name.set("hyperuuid")
        description.set(
            "RFC 9562 UUID v4/v5/v6/v7 generation — high-performance, allocation-free " +
                "FFM bindings straight into a native Rust core (libhyperuuid). " +
                "No runtime bridge, no reflection, no extra dependency."
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
