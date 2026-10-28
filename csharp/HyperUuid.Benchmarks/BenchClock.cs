namespace HyperUuid.Benchmarks;

/// <summary>
/// The current Unix millisecond for the v7 benchmarks, refreshed every 100 ms by a timer so no
/// clock read lands in a timed call. A fixed timestamp would be clamped onto one millisecond of
/// the v7 stream and, held for a whole benchmark process, split it forward until it failed with
/// the exhaustion error; this keeps every call on the stream path at the present.
/// </summary>
static class BenchClock
{
	static long s_ms = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();

	// Held in a field so the timer is never collected.
	static readonly Timer s_timer = new(
		_ => Volatile.Write(ref s_ms, DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()),
		null,
		TimeSpan.FromMilliseconds(100),
		TimeSpan.FromMilliseconds(100));

	public static long Ms => Volatile.Read(ref s_ms);
}
