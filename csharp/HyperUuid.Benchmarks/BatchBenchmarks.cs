using BenchmarkDotNet.Attributes;

namespace HyperUuid.Benchmarks;

/// <summary>
/// Compares <c>NewV6Batch</c>/<c>NewV7Batch</c> (one native call, one random-bytes fetch, one
/// counter reservation for the whole batch) against the equivalent count of individual calls.
/// Run with: dotnet run -c Release --project csharp/HyperUuid.Benchmarks -- --filter *Batch*
/// </summary>
[MemoryDiagnoser]
public class BatchBenchmarks
{
    const long RfcTestVectorMs = 1_645_557_742_000;
    const int Count = 1000;

    [Benchmark(Baseline = true, Description = "NewV6 x1000 individually")]
    public void V6Individual()
    {
        for (var i = 0; i < Count; i++)
            UuidGenerator.NewV6(RfcTestVectorMs);
    }

    [Benchmark(Description = "NewV6Batch(1000)")]
    public Guid[] V6Batch() => UuidGenerator.NewV6Batch(Count, RfcTestVectorMs);

    [Benchmark(Description = "NewV7 x1000 individually")]
    public void V7Individual()
    {
        for (var i = 0; i < Count; i++)
            UuidGenerator.NewV7(RfcTestVectorMs);
    }

    [Benchmark(Description = "NewV7Batch(1000)")]
    public Guid[] V7Batch() => UuidGenerator.NewV7Batch(Count, RfcTestVectorMs);
}
