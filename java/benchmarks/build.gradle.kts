// Local-dev-only JMH benchmarks — mirrors rust/benches (Criterion), csharp/HyperUuid.Benchmarks
// (BenchmarkDotNet), and go/uuidgen_bench_test.go (-benchmem): single-item generation for
// v4/v5/v7, plus batch-vs-1000-individual-calls, run by hand with `./gradlew :benchmarks:jmh`.
plugins {
    java
    id("me.champeau.jmh") version "0.7.3"
}

repositories {
    mavenCentral()
}

dependencies {
    implementation(rootProject)
}

val jdkFloor: Int by rootProject.extra

java {
    sourceCompatibility = JavaVersion.toVersion(jdkFloor)
    targetCompatibility = JavaVersion.toVersion(jdkFloor)
}

// The library's own floor, enforced the same way it is there.
tasks.withType<JavaCompile>().configureEach {
    options.release = jdkFloor
}

// The jmh jar bundles the library's classes; without this attribute the JDK reads only the
// jar's base and a 25 JVM silently benchmarks the JNI path. The @Setup checks fail on that.
tasks.named<Jar>("jmhJar") {
    manifest {
        attributes("Multi-Release" to "true")
    }
}

jmh {
    warmupIterations.set(3)
    iterations.set(5)
    fork.set(1)
    // The iteration *counts* above are meaningless without these: JMH's default time per
    // iteration is 10 seconds, so 3 warmup + 5 measurement is 80s per benchmark, and this
    // suite of ~13 benchmarks took over 15 minutes to produce numbers that had already
    // converged. Observed directly: warmup settles by the second iteration and the five
    // measurements then land within 0.4% of each other.
    //
    // One second per iteration still samples tens of thousands of operations (a 35 us batch
    // op runs ~28,000 times in a second), and this workload is an unusually safe candidate
    // for a short run: every benchmark here is an FFM downcall writing into a caller's
    // buffer across a native boundary, so the JIT effects JMH's long warmup exists to
    // out-wait -- dead-code elimination, constant folding, hoisting the call out of the loop
    // -- cannot apply to it. Raise these again if a benchmark is ever added that measures
    // pure Java work, where those hazards are real.
    warmupForks.set(0)
    timeOnIteration.set("1s")
    warmup.set("1s")
    // Allocation per call is a receipt here, not a claim — the same role [MemoryDiagnoser]
    // plays for the C# suite. gc.alloc.rate.norm is the B/op column the README quotes, so
    // the plain `:benchmarks:jmh` the README names as its reproduction has to print it.
    profilers.set(listOf("gc"))
    // UuidGenerator's FFM downcalls are a "restricted method" — the JMH-forked JVM needs the
    // same opt-in the library's own test task already sets.
    jvmArgsAppend.add("--enable-native-access=ALL-UNNAMED")
    // -PjmhJdk=<feature>: fork the benchmark JVMs on that JDK through the toolchain (the
    // floor, say), the Gradle JVM staying on 25. The default is the Gradle JVM itself.
    if (project.hasProperty("jmhJdk")) {
        val launcher = javaToolchains.launcherFor {
            languageVersion = JavaLanguageVersion.of(project.property("jmhJdk").toString().toInt())
        }
        jvm.set(launcher.map { it.executablePath.asFile.absolutePath })
    }
    // -PjmhInclude=<regex>: a subset of the suite, JMH's own include syntax.
    if (project.hasProperty("jmhInclude")) {
        includes.set(listOf(project.property("jmhInclude") as String))
    }
}

// A benchmark that throws (a @Setup backend check, say) leaves JMH printing the exception and
// the build green: the plugin's failOnError hands JMH `-foe 1`, which JMH reads as false
// (measured: the property set, the exception printed, exit 0), and it exposes no way to pass
// a raw `-foe true`. A run that measured nothing writes a results file with no rows, so the
// task fails on that.
tasks.named("jmh") {
    val results = layout.buildDirectory.file("results/jmh/results.txt")
    doLast {
        val rows = results.get().asFile.readLines().count { Regex("""^\S+\s+(avgt|thrpt|sample|ss)\s+\d+""").containsMatchIn(it) }
        check(rows > 0) { "jmh measured nothing; a benchmark failed (see the exception above)" }
    }
}
