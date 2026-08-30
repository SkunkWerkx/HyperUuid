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

/** 1000 individual calls vs one {@code newV6Batch}/{@code newV7Batch(1000)} downcall. */
@State(Scope.Thread)
@BenchmarkMode(Mode.AverageTime)
@OutputTimeUnit(TimeUnit.MICROSECONDS)
public class BatchBenchmarks {

    private static final long TS = 1_645_557_742_000L;

    @Benchmark
    public void v6IndividualX1000(Blackhole bh) {
        for (int i = 0; i < 1000; i++) {
            bh.consume(UuidGenerator.newV6(TS));
        }
    }

    @Benchmark
    public void v6Batch1000(Blackhole bh) {
        UUID[] ids = UuidGenerator.newV6Batch(1000, TS);
        bh.consume(ids);
    }

    @Benchmark
    public void v7IndividualX1000(Blackhole bh) {
        for (int i = 0; i < 1000; i++) {
            bh.consume(UuidGenerator.newV7(TS));
        }
    }

    @Benchmark
    public void v7Batch1000(Blackhole bh) {
        UUID[] ids = UuidGenerator.newV7Batch(1000, TS);
        bh.consume(ids);
    }
}
