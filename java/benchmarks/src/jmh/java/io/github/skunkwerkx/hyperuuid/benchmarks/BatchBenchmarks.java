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

/** 1000 individual calls vs one {@code newV7Batch(1000)} downcall. */
@State(Scope.Thread)
@BenchmarkMode(Mode.AverageTime)
@OutputTimeUnit(TimeUnit.MICROSECONDS)
public class BatchBenchmarks {

    // The current Unix millisecond, read once per measurement iteration, outside the timed
    // region. A fixed timestamp would be clamped onto one millisecond of the v7 stream and,
    // held for a whole fork, split it forward until it failed with the exhaustion error; one
    // re-read every iteration keeps every call on the stream path at the present.
    private long ts;

    @Setup(Level.Iteration)
    public void readClock() {
        ts = System.currentTimeMillis();
    }

    @Benchmark
    public void v7IndividualX1000(Blackhole bh) {
        for (int i = 0; i < 1000; i++) {
            bh.consume(UuidGenerator.newV7(ts));
        }
    }

    @Benchmark
    public void v7Batch1000(Blackhole bh) {
        UUID[] ids = UuidGenerator.newV7Batch(1000, ts);
        bh.consume(ids);
    }

    // Destination-buffer forms: the arrays are allocated once, in setup, because reusing a
    // buffer the caller already owns is the entire point of these APIs. Allocating one per
    // invocation here would measure exactly the cost they exist to avoid.
    private UUID[] uuidDestination;
    private byte[] byteDestination;

    @Setup
    public void setup() {
        uuidDestination = new UUID[1000];
        byteDestination = new byte[1000 * 16];
    }

    @Benchmark
    public void v7Fill1000(Blackhole bh) {
        UuidGenerator.fillV7(uuidDestination, ts);
        bh.consume(uuidDestination);
    }

    @Benchmark
    public void v7FillBytes1000(Blackhole bh) {
        UuidGenerator.fillV7(byteDestination, ts);
        bh.consume(byteDestination);
    }
}
