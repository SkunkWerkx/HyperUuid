import java.util.jar.JarFile
import org.gradle.external.javadoc.StandardJavadocDocletOptions

plugins {
    `java-library`
    `maven-publish`
    id("com.vanniktech.maven.publish") version "0.37.0"
    id("com.diffplug.spotless") version "8.10.4"
}

// io.github.skunkwerkx — the SkunkWerkx org's own auto-verified Central Portal namespace
// (approved by Central Support after an email request; io.github.buvinghausen was the interim
// personal-account namespace used for the very first real Maven Central publish, proving the
// token auth + GPG signing path end to end — that coordinate stays live on Central permanently
// (no delete), this is where every publish from here on happens).
group = "io.github.skunkwerkx"
// The committed version, which prepare-release.yml bumps (the Forge's hyper_config.py
// matches this line exactly). HYPERUUID_VERSION overrides it for a one-off build; nothing in
// CI sets it for Gradle, so the Central bundle release.yml packs always carries the
// committed version.
version = System.getenv("HYPERUUID_VERSION") ?: "1.0.0"

repositories {
    mavenCentral()
}

// The one source of the JDK floor: the base of the multi-release jar (UuidGenerator and the
// JNI NativeCore) compiles with --release jdkFloor, and src/main/java25 (the FFM NativeCore)
// with --release 25. Gradle, Spotless, javadoc and publish all run on JDK 25.
val jdkFloor by extra(21)

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

dependencies {
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
// library, and it has to be staged as linux-musl-*. Anything else stages nothing.
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
    // The JNI shim beside the core: `cargo build --release --locked --lib` in jni/ (after
    // `cargo cdylib` in ../rust), under the same placed/unsupported exclusion.
    from("jni/target/release") {
        include("libhyperuuid_jni.so", "libhyperuuid_jni.dylib", "hyperuuid_jni.dll")
        if (nativePlaced || nativeRid == "unsupported") {
            exclude("**")
        }
    }
    into(layout.buildDirectory.dir("generated-resources/native/$nativeRid"))

    // Always runs: the guards below read mtimes, which Gradle's up-to-date check (content
    // hashes of the declared inputs) does not see, so a touched source must still fail here.
    outputs.upToDateWhen { false }
    doFirst {
        if (nativePlaced || nativeRid == "unsupported") return@doFirst
        val coreDir = file("../rust/target/release")
        val shimDir = file("jni/target/release")
        val cores = listOf("libhyperuuid.so", "libhyperuuid.dylib", "hyperuuid.dll").map { File(coreDir, it) }.filter { it.isFile }
        val shims = listOf("libhyperuuid_jni.so", "libhyperuuid_jni.dylib", "hyperuuid_jni.dll").map { File(shimDir, it) }.filter { it.isFile }
        // The pair, or neither (CI placed them, or nothing is built): one without the other
        // is a jar that would load on one JDK and fail on the other.
        if (cores.isNotEmpty() && shims.isEmpty()) {
            throw GradleException(
                "${cores.first().name} is built but the JNI shim is not: run `cargo build --release --locked --lib` in java/jni " +
                    "(after `cargo cdylib` in rust/) so the jar carries the pair",
            )
        }
        if (shims.isNotEmpty() && cores.isEmpty()) {
            throw GradleException(
                "${shims.first().name} is built but the core is not: run `cargo cdylib` in rust/ so the jar carries the pair",
            )
        }
        // What cargo itself read: the dep-info file beside each library lists the sources rustc
        // opened, which is exactly what makes cargo rebuild. Walking whole source trees would
        // fail forever on a file the build never reads (python_ext.rs, say). The shim holds the
        // core inside it, so its dep-info covers the core's sources too. Dep-info lists no manifests,
        // so the core's Cargo.toml is an extra for both.
        fun depInfo(dep: File): List<File>? {
            if (!dep.isFile) return null
            val line = dep.readLines().firstOrNull { it.contains(": ") } ?: return null
            return line.substringAfter(": ").trim().split(Regex("(?<!\\\\) +"))
                .filter { it.isNotBlank() }.map { File(it.replace("\\ ", " ")) }
        }
        fun stale(lib: File, dep: File, extra: List<File>, remedy: String) {
            val sources = depInfo(dep) ?: throw GradleException("${lib.name}: no dep-info at ${dep.relativeTo(projectDir)}: $remedy")
            val newer = (sources + extra).filter { it.exists() && it.lastModified() > lib.lastModified() }
            if (newer.isNotEmpty()) {
                throw GradleException("${lib.name} is older than ${newer.first().relativeTo(projectDir)}: $remedy")
            }
        }
        val rebuildCore = "run `cargo cdylib` in rust/"
        val rebuildShim = "run `cargo cdylib` in rust/, then `cargo build --release --locked --lib` in java/jni"
        for (core in cores) {
            stale(core, File(coreDir, core.nameWithoutExtension + ".d"), listOf(file("../rust/Cargo.toml")), rebuildCore)
        }
        for (shim in shims) {
            stale(shim, File(shimDir, shim.nameWithoutExtension + ".d"), listOf(file("../rust/Cargo.toml"), file("jni/Cargo.toml"), file("jni/Cargo.lock")), rebuildShim)
        }
    }
}

sourceSets.main {
    resources.srcDir(layout.buildDirectory.dir("generated-resources"))
}

// The JDK 25 half of the multi-release jar: the FFM NativeCore, compiled --release 25 against
// main's output (UuidGenerator and NativePlatform) and packed under META-INF/versions/25.
val java25 = sourceSets.create("java25") {
    java.setSrcDirs(listOf("src/main/java25"))
    compileClasspath += sourceSets.main.get().output
}

tasks.processResources {
    dependsOn(stageNativeLibrary)
}

// sourcesJar packages the main source set, and `generated-resources` is one of its resource
// dirs (above) — so it reads the staging tasks' output too, and Gradle fails the build
// outright on the undeclared dependency rather than risk a task-order-dependent jar. Only
// the publish path builds sourcesJar (`./gradlew test` never does), which is how HyperCast
// first hit this against Maven Central and not in CI. withType/configureEach rather than
// tasks.named("sourcesJar"): the sources and javadoc jars are registered by the vanniktech
// publish plugin, so they don't exist yet at this point in configuration.
tasks.withType<Jar>().configureEach {
    dependsOn(stageNativeLibrary)
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
    // the automatic name is all a consumer on the module path needs from this jar.
    // Multi-Release: JDK 25 and later read META-INF/versions/25 (the FFM NativeCore); the
    // base is the JNI NativeCore, which 21 through 24 run.
    manifest {
        attributes(
            "Automatic-Module-Name" to "io.github.skunkwerkx.hyperuuid",
            "Multi-Release" to "true",
        )
    }
    into("META-INF/versions/25") {
        from(java25.output)
    }
}

// The same contents without the native libraries, for the test that runs with no core on the
// classpath. Never published: nothing attaches it to a publication.
val noNativeJar = tasks.register<Jar>("noNativeJar") {
    archiveClassifier = "nonative"
    from(sourceSets.main.get().output) {
        exclude("native/**")
    }
    into("META-INF/versions/25") {
        from(java25.output)
    }
    manifest {
        attributes(
            "Automatic-Module-Name" to "io.github.skunkwerkx.hyperuuid",
            "Multi-Release" to "true",
        )
    }
}

// The sources jar mirrors the layout: the FFM tree under META-INF/versions/25.
tasks.withType<Jar>().matching { it.name == "sourcesJar" }.configureEach {
    into("META-INF/versions/25") {
        from(java25.allJava)
    }
    manifest {
        attributes(
            "Automatic-Module-Name" to "io.github.skunkwerkx.hyperuuid",
            "Multi-Release" to "true",
        )
    }
}

// What the multi-release layout must be, checked on the built jar: the JDK's own validator,
// the manifest attribute, the class file major versions (65 for the 21 base, 69 under
// versions/25), a NativeCore in both trees, and both libraries in every RID directory. A jar
// with only the local RID passes; staging puts the pair in each one.
val validateMultiReleaseJar = tasks.register("validateMultiReleaseJar") {
    val jarFile = tasks.jar.flatMap { it.archiveFile }
    dependsOn(tasks.jar)
    inputs.file(jarFile)
    doLast {
        val jar = jarFile.get().asFile
        val jarTool = File(System.getProperty("java.home"), "bin/jar" + if (System.getProperty("os.name").lowercase().startsWith("windows")) ".exe" else "")
        val validate = ProcessBuilder(jarTool.path, "--validate", "--file", jar.path).redirectErrorStream(true).start()
        val validateOutput = validate.inputStream.readBytes().decodeToString()
        check(validate.waitFor() == 0) { "jar --validate failed on ${jar.name}:\n$validateOutput" }

        JarFile(jar).use { zip ->
            check(zip.manifest?.mainAttributes?.getValue("Multi-Release") == "true") {
                "${jar.name}: the manifest lacks Multi-Release: true"
            }
            val names = zip.entries().asSequence().map { it.name }.toList()
            for (name in names.filter { it.endsWith(".class") && !it.endsWith("module-info.class") }) {
                val head = zip.getInputStream(zip.getEntry(name)).use { it.readNBytes(8) }
                val major = ((head[6].toInt() and 0xFF) shl 8) or (head[7].toInt() and 0xFF)
                val want = if (name.startsWith("META-INF/versions/25/")) 69 else jdkFloor + 44
                check(major == want) { "${jar.name}: $name is class file major $major, expected $want" }
            }
            val nativeCore = "io/github/skunkwerkx/hyperuuid/NativeCore.class"
            for (entry in listOf(nativeCore, "META-INF/versions/25/$nativeCore")) {
                check(entry in names) { "${jar.name}: $entry is missing" }
            }
            val core = setOf("libhyperuuid.so", "libhyperuuid.dylib", "hyperuuid.dll")
            val shim = setOf("libhyperuuid_jni.so", "libhyperuuid_jni.dylib", "hyperuuid_jni.dll")
            val byRid = names.filter { it.startsWith("native/") && !it.endsWith("/") }
                .groupBy({ it.removePrefix("native/").substringBefore('/') }, { it.substringAfterLast('/') })
            for ((rid, files) in byRid) {
                check(files.any { it in core }) { "${jar.name}: native/$rid has no core library ($files)" }
                check(files.any { it in shim }) { "${jar.name}: native/$rid has no JNI library ($files)" }
            }
        }
    }
}

// InspectionTest fills one version 7 batch of exactly UuidGenerator.MAX_V7_BATCH (1 GiB of
// bytes), and refuses one a UUID past it from a caller array that size: past the test JVM's
// 512 MiB default either way.
tasks.withType<Test>().configureEach {
    maxHeapSize = "3g"
}

// Both suites run the same classes against the jar, which is the unit under test: the
// multi-release layout picks the FFM NativeCore on a 25 JVM and the JNI one on 21.
// `test` runs on Gradle's own JDK (25), `testFloor` on the floor.
fun Test.suite(backend: String, feature: Int) {
    useJUnitPlatform()
    testClassesDirs = sourceSets.test.get().output.classesDirs
    classpath = files(tasks.jar) + sourceSets.test.get().output + configurations.testRuntimeClasspath.get()
    dependsOn(validateMultiReleaseJar, noNativeJar)
    inputs.file(noNativeJar.flatMap { it.archiveFile })
    // The no-core child JVM runs against this. Lazy: the task is resolved when the JVM starts,
    // not when the build is configured.
    jvmArgumentProviders.add(
        CommandLineArgumentProvider {
            listOf("-Dhyperuuid.noNativeJar=" + noNativeJar.get().archiveFile.get().asFile.absolutePath)
        },
    )
    systemProperty("hyperuuid.expectBackend", backend)
    systemProperty("hyperuuid.expectFeature", feature.toString())
    // NativeCore's FFM downcalls are a "restricted method", and on JDK 24+ so is the JNI
    // path's System.load: this silences the runtime warning today and avoids them being
    // blocked outright in a future JDK. -Xcheck:jni makes the VM check every JNI call the
    // shim makes; its findings print natively, past Gradle's listeners, so CheckJniTest
    // reads them from a child JVM of its own.
    jvmArgs("--enable-native-access=ALL-UNNAMED", "-Xcheck:jni")
    // This binding's own version, so the suite can pin UuidGenerator.nativeVersion() to it:
    // the core and the jar move together (prepare-release.yml bumps both), and the probe
    // exists to prove exactly that.
    systemProperty("hyperuuid.version", version)
}

tasks.test {
    suite("ffm", 25)
}

// The same suite on the floor JDK, through the toolchain: the JNI path end to end.
val testFloor = tasks.register<Test>("testFloor") {
    suite("jni", jdkFloor)
    javaLauncher = javaToolchains.launcherFor {
        languageVersion = JavaLanguageVersion.of(jdkFloor)
    }
}

// Both suites must have run the same tests, none skipped: a test that quietly does not run on
// one JDK is a path nobody checked.
val checkSuiteParity = tasks.register("checkSuiteParity") {
    dependsOn(tasks.test, testFloor)
    val results = layout.buildDirectory.dir("test-results")
    doLast {
        fun read(suite: String): Pair<Set<String>, List<String>> {
            val dir = results.get().dir(suite).asFile
            val files = dir.listFiles { f -> f.name.endsWith(".xml") }.orEmpty()
            check(files.isNotEmpty()) { "no test results in $dir" }
            val ran = sortedSetOf<String>()
            val skipped = mutableListOf<String>()
            for (file in files) {
                val doc = javax.xml.parsers.DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(file)
                val cases = doc.getElementsByTagName("testcase")
                for (i in 0 until cases.length) {
                    val case = cases.item(i) as org.w3c.dom.Element
                    val id = case.getAttribute("classname") + "#" + case.getAttribute("name")
                    ran += id
                    if (case.getElementsByTagName("skipped").length > 0) skipped += id
                }
            }
            return ran to skipped
        }
        val (onTest, skippedTest) = read("test")
        val (onFloor, skippedFloor) = read("testFloor")
        check(skippedTest.isEmpty() && skippedFloor.isEmpty()) {
            "skipped tests: test=$skippedTest testFloor=$skippedFloor"
        }
        check(onTest == onFloor) {
            "the suites differ: only in test=${onTest - onFloor}, only in testFloor=${onFloor - onTest}"
        }
        logger.lifecycle("suite parity: ${onTest.size} tests on each JDK, none skipped")
    }
}

tasks.check {
    dependsOn(testFloor, checkSuiteParity)
}

java {
    // 21 is the floor: the oldest long-term-support JDK Temurin still ships for every platform
    // this jar bundles a library for (no JDK 17 for Windows arm64 or Alpine arm64), and it
    // runs the JNI NativeCore; 25 and later take the FFM one from META-INF/versions/25.
    sourceCompatibility = JavaVersion.toVersion(jdkFloor)
    targetCompatibility = JavaVersion.toVersion(jdkFloor)
    withSourcesJar()
}

// --release, not just the -source/-target pair above: it also compiles against the target
// JDK's own API signatures whatever JDK is running the build (25), so it cannot let a newer
// API slip into a jar that claims 21. Only the java25 tree compiles against 25.
tasks.withType<JavaCompile>().configureEach {
    options.release = if (name == "compileJava25Java") 25 else jdkFloor
}

// javadoc's own doclint already flags a missing comment/@param/@return as a WARNING by
// default — -Xwerror promotes those warnings to build-failing errors, so an undocumented
// public member can't ship again silently. Central Portal requires a javadoc jar for every
// artifact anyway (see mavenPublishing below), so this is enforcing a real publish
// prerequisite, not just style.
tasks.javadoc {
    (options as StandardJavadocDocletOptions).addBooleanOption("Xwerror", true)
    // The documented source is the 21 base, run on the 25 tool: document it as 21.
    (options as StandardJavadocDocletOptions).addStringOption("-release", jdkFloor.toString())
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
            "RFC 9562 UUID v4/v5/v7 generation, with a version 8 SQL Server order for v7 — " +
                "high-performance bindings " +
                "straight into a native Rust core (libhyperuuid) that never allocates. " +
                "JDK 21+: JNI on 21–24, FFM on 25+. No runtime bridge, no extra dependency."
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
