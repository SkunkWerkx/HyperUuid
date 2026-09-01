using BenchmarkDotNet.Attributes;

namespace HyperUuid.Benchmarks;

/// <summary>
/// Compares the three batch shapes against the equivalent count of individual calls.
/// Run with: dotnet run -c Release --project csharp/HyperUuid.Benchmarks -- --filter *Batch*
/// </summary>
/// <remarks>
/// The point of separating <c>Fill</c> from <c>NewBatch</c> is that they amortize different
/// things. <c>NewV7Batch</c> amortizes the FFI call but still allocates the result array;
/// <c>FillV7(Span&lt;Guid&gt;)</c> drops the allocation but still pays a
/// <c>new Guid(chunk, bigEndian: true)</c> conversion per element, because Guid's in-memory
/// layout is mixed-endian and isn't RFC byte order; <c>FillV7(Span&lt;byte&gt;)</c> drops the
/// conversion too, since the native core already writes RFC-ordered bytes contiguously into the
/// caller's own buffer. Only the last one is a true single-native-call-zero-managed-work path,
/// which is what a destination-buffer API has to be to beat a hand-rolled loop.
/// </remarks>
[MemoryDiagnoser]
public class BatchBenchmarks
{
    const long RfcTestVectorMs = 1_645_557_742_000;
    const int Count = 1000;

    Guid[] _guidDestination = [];
    byte[] _byteDestination = [];

    [GlobalSetup]
    public void Setup()
    {
        _guidDestination = new Guid[Count];
        _byteDestination = new byte[Count * 16];
    }

    [Benchmark(Baseline = true, Description = "NewV7 x1000 individually")]
    public void V7Individual()
    {
        for (var i = 0; i < Count; i++)
            UuidGenerator.NewV7(RfcTestVectorMs);
    }

    [Benchmark(Description = "NewV7Batch(1000) -> new Guid[]")]
    public Guid[] V7Batch() => UuidGenerator.NewV7Batch(Count, RfcTestVectorMs);

    [Benchmark(Description = "FillV7(Span<Guid>) into existing array")]
    public void V7Fill() => UuidGenerator.FillV7(_guidDestination, RfcTestVectorMs);

    [Benchmark(Description = "FillV7(Span<byte>) into existing buffer")]
    public void V7FillBytes() => UuidGenerator.FillV7(_byteDestination, RfcTestVectorMs);

    [Benchmark(Description = "NewV6 x1000 individually")]
    public void V6Individual()
    {
        for (var i = 0; i < Count; i++)
            UuidGenerator.NewV6(RfcTestVectorMs);
    }

    [Benchmark(Description = "NewV6Batch(1000) -> new Guid[]")]
    public Guid[] V6Batch() => UuidGenerator.NewV6Batch(Count, RfcTestVectorMs);

    [Benchmark(Description = "FillV6(Span<Guid>) into existing array")]
    public void V6Fill() => UuidGenerator.FillV6(_guidDestination, RfcTestVectorMs);

    [Benchmark(Description = "FillV6(Span<byte>) into existing buffer")]
    public void V6FillBytes() => UuidGenerator.FillV6(_byteDestination, RfcTestVectorMs);
}
