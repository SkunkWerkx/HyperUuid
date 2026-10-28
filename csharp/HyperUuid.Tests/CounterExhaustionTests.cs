using Shouldly;

namespace HyperUuid.Tests;

public sealed class CounterExhaustionTests
{
	const string Message = "version 7 counter exhausted: the stream cannot advance without running more than 1000 ms ahead of the supplied timestamp or past its last millisecond";

	// A mapping test, not an end-to-end one: .NET has no cheap in-suite process isolation, and
	// exhausting the process-global stream here would break every later test. The C ABI end to
	// end (code 6 out of uuid_new_v7 and uuid_new_v7_batch) is cdylib_smoke.py's.
	[Fact]
	public void CodeSixIsCounterExhaustion()
	{
		// The single site (NewV7 throws what V7Failure gives it).
		var single = UuidGenerator.V7Failure(6, "uuid_new_v7");
		single.GetType().ShouldBe(typeof(InvalidOperationException));
		single.Message.ShouldBe(Message);

		// The batch site (FillV7 and NewV7Batch, Guid and byte forms alike).
		var batch = Should.Throw<Exception>(() => UuidGenerator.ThrowOnBatchFailure(6, "uuid_new_v7_batch", "range", "destination"));
		batch.GetType().ShouldBe(typeof(InvalidOperationException));
		batch.Message.ShouldBe(Message);
	}

	[Fact]
	public void OtherCodesKeepTheirMapping()
	{
		UuidGenerator.V7Failure(2, "uuid_new_v7").GetType().ShouldBe(typeof(ArgumentOutOfRangeException));
		var random = UuidGenerator.V7Failure(1, "uuid_new_v7");
		random.GetType().ShouldBe(typeof(InvalidOperationException));
		random.Message.ShouldContain("random source failure");
	}
}
