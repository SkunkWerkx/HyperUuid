using System.Runtime.CompilerServices;

namespace HyperUuid.Tests;

// The v7 stream is process-global. Prime it at the present once, before any test: a fixed
// old timestamp (the RFC vector) then mints out of stream and carries its own millisecond
// exactly, instead of seeding the stream in 2022 and leaving which tests are ordered up to
// their order of execution. The rule for every test in this assembly: no test supplies more than
// now + 500 ms, and current-time bounds allow + 1000 ms.
internal static class TestStream
{
#pragma warning disable CA2255 // Module initializer in a test assembly, on purpose.
	[ModuleInitializer]
	internal static void Prime() => UuidGenerator.NewV7(DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
#pragma warning restore CA2255
}
