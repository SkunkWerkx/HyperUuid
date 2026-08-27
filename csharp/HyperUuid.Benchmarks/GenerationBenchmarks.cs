using BenchmarkDotNet.Attributes;

namespace HyperUuid.Benchmarks;

/// <summary>
/// Compares each single-item generator against the <see cref="Guid.NewGuid"/> baseline, and
/// verifies the "allocation-free" claim empirically: <see cref="MemoryDiagnoserAttribute"/>
/// reports bytes allocated per operation, and <c>stackalloc</c> (what every UuidGenerator
/// call uses internally) correctly shows as 0 B here — it's a real check, not a red herring.
/// Run with: dotnet run -c Release --project csharp/HyperUuid.Benchmarks -- --filter *Generation*
/// </summary>
[MemoryDiagnoser]
public class GenerationBenchmarks
{
    const long RfcTestVectorMs = 1_645_557_742_000;

    [Benchmark(Baseline = true, Description = "Guid.NewGuid")]
    public Guid SystemGuidNewGuid() => Guid.NewGuid();

    [Benchmark(Description = "UuidGenerator.NewV4")]
    public Guid NewV4() => UuidGenerator.NewV4();

    [Benchmark(Description = "UuidGenerator.NewV5")]
    public Guid NewV5() => UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "www.example.com");

    [Benchmark(Description = "UuidGenerator.NewV6")]
    public Guid NewV6() => UuidGenerator.NewV6(RfcTestVectorMs);

    [Benchmark(Description = "UuidGenerator.NewV7")]
    public Guid NewV7() => UuidGenerator.NewV7(RfcTestVectorMs);
}
