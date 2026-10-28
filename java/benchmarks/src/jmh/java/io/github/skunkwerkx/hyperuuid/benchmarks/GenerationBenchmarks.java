package io.github.skunkwerkx.hyperuuid.benchmarks;

import io.github.skunkwerkx.hyperuuid.UuidGenerator;
import java.util.UUID;
import java.util.concurrent.TimeUnit;
import org.openjdk.jmh.annotations.Benchmark;
import org.openjdk.jmh.annotations.BenchmarkMode;
import org.openjdk.jmh.annotations.Level;
import org.openjdk.jmh.annotations.Mode;
import org.openjdk.jmh.annotations.OutputTimeUnit;
import org.openjdk.jmh.annotations.Scope;
import org.openjdk.jmh.annotations.Setup;
import org.openjdk.jmh.annotations.State;
import org.openjdk.jmh.infra.Blackhole;

/**
 * Single-item generation, including {@link UUID#randomUUID()} as the baseline this jar's FFM
 * downcall has to cross the JNI/native boundary to even compete with — {@code java.util.UUID}
 * has no v5/v7 equivalent to benchmark against.
 */
@State(Scope.Thread)
@BenchmarkMode(Mode.AverageTime)
@OutputTimeUnit(TimeUnit.NANOSECONDS)
public class GenerationBenchmarks {

    private static final UUID NAMESPACE = UuidGenerator.Namespaces.DNS;
    // The current Unix millisecond, read once per measurement iteration, outside the timed
    // region. A fixed timestamp would be clamped onto one millisecond of the v7 stream and,
    // held for a whole fork, split it forward until it failed with the exhaustion error; one
    // re-read every iteration keeps every call on the stream path at the present.
    private long ts;

    @Setup(Level.Trial)
    public void requireExpectedBackend() {
        BackendCheck.require();
    }

    @Setup(Level.Iteration)
    public void readClock() {
        ts = System.currentTimeMillis();
    }

    @Benchmark
    public void javaUtilRandomUuid(Blackhole bh) {
        bh.consume(UUID.randomUUID());
    }

    @Benchmark
    public void newV4(Blackhole bh) {
        bh.consume(UuidGenerator.newV4());
    }

    @Benchmark
    public void newV5(Blackhole bh) {
        bh.consume(UuidGenerator.newV5(NAMESPACE, "example.com"));
    }

    @Benchmark
    public void newV7(Blackhole bh) {
        bh.consume(UuidGenerator.newV7(ts));
    }
}
