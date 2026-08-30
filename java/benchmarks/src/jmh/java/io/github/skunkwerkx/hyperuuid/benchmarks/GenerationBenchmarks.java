package io.github.skunkwerkx.hyperuuid.benchmarks;

import io.github.skunkwerkx.hyperuuid.UuidGenerator;
import java.util.UUID;
import java.util.concurrent.TimeUnit;
import org.openjdk.jmh.annotations.Benchmark;
import org.openjdk.jmh.annotations.BenchmarkMode;
import org.openjdk.jmh.annotations.Mode;
import org.openjdk.jmh.annotations.OutputTimeUnit;
import org.openjdk.jmh.annotations.Scope;
import org.openjdk.jmh.annotations.State;
import org.openjdk.jmh.infra.Blackhole;

/**
 * Single-item generation, including {@link UUID#randomUUID()} as the baseline this jar's FFM
 * downcall has to cross the JNI/native boundary to even compete with — {@code java.util.UUID}
 * has no v5/v6/v7 equivalent to benchmark against.
 */
@State(Scope.Thread)
@BenchmarkMode(Mode.AverageTime)
@OutputTimeUnit(TimeUnit.NANOSECONDS)
public class GenerationBenchmarks {

    private static final UUID NAMESPACE = UuidGenerator.Namespaces.DNS;
    private static final long TS = 1_645_557_742_000L;

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
    public void newV6(Blackhole bh) {
        bh.consume(UuidGenerator.newV6(TS));
    }

    @Benchmark
    public void newV7(Blackhole bh) {
        bh.consume(UuidGenerator.newV7(TS));
    }
}
