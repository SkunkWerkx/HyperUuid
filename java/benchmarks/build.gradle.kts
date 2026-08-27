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
    // UuidGenerator's FFM downcalls are a "restricted method" — the JMH-forked JVM needs the
    // same opt-in the library's own test task already sets.
    jvmArgsAppend.add("--enable-native-access=ALL-UNNAMED")
}
