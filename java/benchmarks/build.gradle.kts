// Local-dev-only JMH benchmarks — mirrors rust/benches (Criterion), csharp/HyperUuid.Benchmarks
// (BenchmarkDotNet), and go/uuidgen_bench_test.go (-benchmem): single-item generation for
// v4/v5/v6/v7, plus batch-vs-1000-individual-calls, run by hand with `./gradlew :benchmarks:jmh`.
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

java {
    sourceCompatibility = JavaVersion.VERSION_22
    targetCompatibility = JavaVersion.VERSION_22
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
    // UuidGenerator's FFM downcalls are a "restricted method" — the JMH-forked JVM needs the
    // same opt-in the library's own test task already sets.
    jvmArgsAppend.add("--enable-native-access=ALL-UNNAMED")
}
