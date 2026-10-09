using System.Buffers;
using System.Runtime.InteropServices;

namespace HyperUuid;

/// <summary>
/// RFC 9562 UUID generation (v4 random, v5 deterministic, v6/v7 time-sortable) calling directly
/// into the native <c>libhyperuuid</c> shared library via source-generated P/Invoke.
/// </summary>
/// <remarks>
/// No allocation beyond the fixed 16-byte stack buffers here — the underlying Rust core never
/// allocates for these calls either — confirmed empirically with BenchmarkDotNet's
/// <c>[MemoryDiagnoser]</c> in <c>HyperUuid.Benchmarks</c> (0 B for <c>NewV4</c>/<c>NewV5</c>/
/// <c>NewV6</c>/<c>NewV7</c>, including the <c>string</c>-based <see cref="NewV5(Guid, string)"/>
/// overload, which UTF-8-encodes into a 256-byte stack buffer with an <see cref="ArrayPool{T}"/>
/// fallback for longer names — the same pattern the batch methods already use, ported from this
/// project's own <c>SequentialGuid</c> library). AOT/trimming friendly: <see cref="LibraryImportAttribute"/>
/// is source-generated (no runtime reflection), so this type publishes cleanly under
/// <c>PublishAot</c>. Needs a platform-specific native binary — the package carries one per
/// supported RID under <c>runtimes/{rid}/native/</c> (this package's own README's Platform
/// support section has the list), and <see cref="IsAvailable"/> says whether one resolved.
/// One compiled assembly covers every platform including <c>browser-wasm</c> (Blazor) — no
/// separate build. Every native entry point is declared three times, unconditionally: once
/// against <c>"hyperuuid"</c> (resolved via <c>dlopen</c> on every real native platform), once
/// against <c>"*"</c> (resolves against the current module — the only thing that works for a
/// statically-linked WASM native, which has no separate module to dlopen), and once against
/// <c>"__Internal"</c> (the app's own executable, which is where .NET for iOS and Mac Catalyst
/// link a static library and the name their AOT compiler turns into a direct call), sharing
/// the same <see cref="LibraryImportAttribute.EntryPoint"/> so all three point at the
/// identical native symbol. <see cref="OperatingSystem.IsBrowser"/> and
/// <see cref="OperatingSystem.IsIOS"/> (true on Mac Catalyst as well) pick the right one at
/// the call site — real runtime checks, not just documentation, but ones the .NET linker
/// specifically knows how
/// to constant-fold per publish target (the same mechanism the BCL itself uses for
/// platform-conditional code), so a trimmed/published build still only ships the branch that
/// platform can actually reach, same as the old two-build split did — see
/// <c>HyperUuid.csproj</c>'s packaging targets for exactly how the single build lands in the
/// NuGet package. Proven working end-to-end in a real headless-browser session — see this
/// package's own README's WebAssembly (Blazor) section. WebAssembly is .NET 11 and later only.
/// </remarks>
public static partial class UuidGenerator
{
	[LibraryImport("hyperuuid", EntryPoint = "hyperuuid_version")]
	private static partial uint hyperuuid_version_native();
	[LibraryImport("*", EntryPoint = "hyperuuid_version")]
	private static partial uint hyperuuid_version_browser();
	[LibraryImport("__Internal", EntryPoint = "hyperuuid_version")]
	private static partial uint hyperuuid_version_internal();
	private static uint hyperuuid_version() =>
		OperatingSystem.IsBrowser() ? hyperuuid_version_browser()
			: OperatingSystem.IsIOS() ? hyperuuid_version_internal()
			: hyperuuid_version_native();

	[LibraryImport("hyperuuid", EntryPoint = "uuid_new_v4")]
	private static unsafe partial int uuid_new_v4_native(byte* outPtr);
	[LibraryImport("*", EntryPoint = "uuid_new_v4")]
	private static unsafe partial int uuid_new_v4_browser(byte* outPtr);
	[LibraryImport("__Internal", EntryPoint = "uuid_new_v4")]
	private static unsafe partial int uuid_new_v4_internal(byte* outPtr);
	private static unsafe int uuid_new_v4(byte* outPtr) =>
		OperatingSystem.IsBrowser() ? uuid_new_v4_browser(outPtr)
			: OperatingSystem.IsIOS() ? uuid_new_v4_internal(outPtr)
			: uuid_new_v4_native(outPtr);

	[LibraryImport("hyperuuid", EntryPoint = "uuid_new_v5")]
	private static unsafe partial int uuid_new_v5_native(byte* nsPtr, byte* namePtr, uint nameLen, byte* outPtr);
	[LibraryImport("*", EntryPoint = "uuid_new_v5")]
	private static unsafe partial int uuid_new_v5_browser(byte* nsPtr, byte* namePtr, uint nameLen, byte* outPtr);
	[LibraryImport("__Internal", EntryPoint = "uuid_new_v5")]
	private static unsafe partial int uuid_new_v5_internal(byte* nsPtr, byte* namePtr, uint nameLen, byte* outPtr);
	private static unsafe int uuid_new_v5(byte* nsPtr, byte* namePtr, uint nameLen, byte* outPtr) =>
		OperatingSystem.IsBrowser()
			? uuid_new_v5_browser(nsPtr, namePtr, nameLen, outPtr)
			: OperatingSystem.IsIOS()
				? uuid_new_v5_internal(nsPtr, namePtr, nameLen, outPtr)
				: uuid_new_v5_native(nsPtr, namePtr, nameLen, outPtr);

	[LibraryImport("hyperuuid", EntryPoint = "uuid_new_v6")]
	private static unsafe partial int uuid_new_v6_native(long unixMillis, byte* outPtr);
	[LibraryImport("*", EntryPoint = "uuid_new_v6")]
	private static unsafe partial int uuid_new_v6_browser(long unixMillis, byte* outPtr);
	[LibraryImport("__Internal", EntryPoint = "uuid_new_v6")]
	private static unsafe partial int uuid_new_v6_internal(long unixMillis, byte* outPtr);
	private static unsafe int uuid_new_v6(long unixMillis, byte* outPtr) =>
		OperatingSystem.IsBrowser() ? uuid_new_v6_browser(unixMillis, outPtr)
			: OperatingSystem.IsIOS() ? uuid_new_v6_internal(unixMillis, outPtr)
			: uuid_new_v6_native(unixMillis, outPtr);

	[LibraryImport("hyperuuid", EntryPoint = "uuid_v6_unix_millis")]
	private static unsafe partial ulong uuid_v6_unix_millis_native(byte* uuidPtr);
	[LibraryImport("*", EntryPoint = "uuid_v6_unix_millis")]
	private static unsafe partial ulong uuid_v6_unix_millis_browser(byte* uuidPtr);
	[LibraryImport("__Internal", EntryPoint = "uuid_v6_unix_millis")]
	private static unsafe partial ulong uuid_v6_unix_millis_internal(byte* uuidPtr);
	private static unsafe ulong uuid_v6_unix_millis(byte* uuidPtr) =>
		OperatingSystem.IsBrowser() ? uuid_v6_unix_millis_browser(uuidPtr)
			: OperatingSystem.IsIOS() ? uuid_v6_unix_millis_internal(uuidPtr)
			: uuid_v6_unix_millis_native(uuidPtr);

	[LibraryImport("hyperuuid", EntryPoint = "uuid_new_v6_batch")]
	private static unsafe partial int uuid_new_v6_batch_native(long unixMillis, uint count, byte* outPtr);
	[LibraryImport("*", EntryPoint = "uuid_new_v6_batch")]
	private static unsafe partial int uuid_new_v6_batch_browser(long unixMillis, uint count, byte* outPtr);
	[LibraryImport("__Internal", EntryPoint = "uuid_new_v6_batch")]
	private static unsafe partial int uuid_new_v6_batch_internal(long unixMillis, uint count, byte* outPtr);
	private static unsafe int uuid_new_v6_batch(long unixMillis, uint count, byte* outPtr) =>
		OperatingSystem.IsBrowser()
			? uuid_new_v6_batch_browser(unixMillis, count, outPtr)
			: OperatingSystem.IsIOS()
				? uuid_new_v6_batch_internal(unixMillis, count, outPtr)
				: uuid_new_v6_batch_native(unixMillis, count, outPtr);

	[LibraryImport("hyperuuid", EntryPoint = "uuid_new_v7")]
	private static unsafe partial int uuid_new_v7_native(long unixMillis, byte* outPtr);
	[LibraryImport("*", EntryPoint = "uuid_new_v7")]
	private static unsafe partial int uuid_new_v7_browser(long unixMillis, byte* outPtr);
	[LibraryImport("__Internal", EntryPoint = "uuid_new_v7")]
	private static unsafe partial int uuid_new_v7_internal(long unixMillis, byte* outPtr);
	private static unsafe int uuid_new_v7(long unixMillis, byte* outPtr) =>
		OperatingSystem.IsBrowser() ? uuid_new_v7_browser(unixMillis, outPtr)
			: OperatingSystem.IsIOS() ? uuid_new_v7_internal(unixMillis, outPtr)
			: uuid_new_v7_native(unixMillis, outPtr);

	[LibraryImport("hyperuuid", EntryPoint = "uuid_v7_unix_millis")]
	private static unsafe partial ulong uuid_v7_unix_millis_native(byte* uuidPtr);
	[LibraryImport("*", EntryPoint = "uuid_v7_unix_millis")]
	private static unsafe partial ulong uuid_v7_unix_millis_browser(byte* uuidPtr);
	[LibraryImport("__Internal", EntryPoint = "uuid_v7_unix_millis")]
	private static unsafe partial ulong uuid_v7_unix_millis_internal(byte* uuidPtr);
	private static unsafe ulong uuid_v7_unix_millis(byte* uuidPtr) =>
		OperatingSystem.IsBrowser() ? uuid_v7_unix_millis_browser(uuidPtr)
			: OperatingSystem.IsIOS() ? uuid_v7_unix_millis_internal(uuidPtr)
			: uuid_v7_unix_millis_native(uuidPtr);

	[LibraryImport("hyperuuid", EntryPoint = "uuid_new_v7_batch")]
	private static unsafe partial int uuid_new_v7_batch_native(long unixMillis, uint count, byte* outPtr);
	[LibraryImport("*", EntryPoint = "uuid_new_v7_batch")]
	private static unsafe partial int uuid_new_v7_batch_browser(long unixMillis, uint count, byte* outPtr);
	[LibraryImport("__Internal", EntryPoint = "uuid_new_v7_batch")]
	private static unsafe partial int uuid_new_v7_batch_internal(long unixMillis, uint count, byte* outPtr);
	private static unsafe int uuid_new_v7_batch(long unixMillis, uint count, byte* outPtr) =>
		OperatingSystem.IsBrowser()
			? uuid_new_v7_batch_browser(unixMillis, count, outPtr)
			: OperatingSystem.IsIOS()
				? uuid_new_v7_batch_internal(unixMillis, count, outPtr)
				: uuid_new_v7_batch_native(unixMillis, count, outPtr);

	[LibraryImport("hyperuuid", EntryPoint = "uuid_v7_to_sql_order")]
	private static unsafe partial void uuid_v7_to_sql_order_native(byte* uuidPtr);
	[LibraryImport("*", EntryPoint = "uuid_v7_to_sql_order")]
	private static unsafe partial void uuid_v7_to_sql_order_browser(byte* uuidPtr);
	[LibraryImport("__Internal", EntryPoint = "uuid_v7_to_sql_order")]
	private static unsafe partial void uuid_v7_to_sql_order_internal(byte* uuidPtr);
	private static unsafe void uuid_v7_to_sql_order(byte* uuidPtr)
	{
		if (OperatingSystem.IsBrowser()) uuid_v7_to_sql_order_browser(uuidPtr);
		else if (OperatingSystem.IsIOS()) uuid_v7_to_sql_order_internal(uuidPtr);
		else uuid_v7_to_sql_order_native(uuidPtr);
	}

	[LibraryImport("hyperuuid", EntryPoint = "uuid_v7_to_rfc_order")]
	private static unsafe partial void uuid_v7_to_rfc_order_native(byte* uuidPtr);
	[LibraryImport("*", EntryPoint = "uuid_v7_to_rfc_order")]
	private static unsafe partial void uuid_v7_to_rfc_order_browser(byte* uuidPtr);
	[LibraryImport("__Internal", EntryPoint = "uuid_v7_to_rfc_order")]
	private static unsafe partial void uuid_v7_to_rfc_order_internal(byte* uuidPtr);
	private static unsafe void uuid_v7_to_rfc_order(byte* uuidPtr)
	{
		if (OperatingSystem.IsBrowser()) uuid_v7_to_rfc_order_browser(uuidPtr);
		else if (OperatingSystem.IsIOS()) uuid_v7_to_rfc_order_internal(uuidPtr);
		else uuid_v7_to_rfc_order_native(uuidPtr);
	}

	[LibraryImport("hyperuuid", EntryPoint = "uuid_v6_to_sql_order")]
	private static unsafe partial void uuid_v6_to_sql_order_native(byte* uuidPtr);
	[LibraryImport("*", EntryPoint = "uuid_v6_to_sql_order")]
	private static unsafe partial void uuid_v6_to_sql_order_browser(byte* uuidPtr);
	[LibraryImport("__Internal", EntryPoint = "uuid_v6_to_sql_order")]
	private static unsafe partial void uuid_v6_to_sql_order_internal(byte* uuidPtr);
	private static unsafe void uuid_v6_to_sql_order(byte* uuidPtr)
	{
		if (OperatingSystem.IsBrowser()) uuid_v6_to_sql_order_browser(uuidPtr);
		else if (OperatingSystem.IsIOS()) uuid_v6_to_sql_order_internal(uuidPtr);
		else uuid_v6_to_sql_order_native(uuidPtr);
	}

	[LibraryImport("hyperuuid", EntryPoint = "uuid_v6_to_rfc_order")]
	private static unsafe partial void uuid_v6_to_rfc_order_native(byte* uuidPtr);
	[LibraryImport("*", EntryPoint = "uuid_v6_to_rfc_order")]
	private static unsafe partial void uuid_v6_to_rfc_order_browser(byte* uuidPtr);
	[LibraryImport("__Internal", EntryPoint = "uuid_v6_to_rfc_order")]
	private static unsafe partial void uuid_v6_to_rfc_order_internal(byte* uuidPtr);
	private static unsafe void uuid_v6_to_rfc_order(byte* uuidPtr)
	{
		if (OperatingSystem.IsBrowser()) uuid_v6_to_rfc_order_browser(uuidPtr);
		else if (OperatingSystem.IsIOS()) uuid_v6_to_rfc_order_internal(uuidPtr);
		else uuid_v6_to_rfc_order_native(uuidPtr);
	}

	[LibraryImport("hyperuuid", EntryPoint = "uuid_version")]
	private static unsafe partial uint uuid_version_native(byte* uuidPtr, uint layoutCode);
	[LibraryImport("*", EntryPoint = "uuid_version")]
	private static unsafe partial uint uuid_version_browser(byte* uuidPtr, uint layoutCode);
	[LibraryImport("__Internal", EntryPoint = "uuid_version")]
	private static unsafe partial uint uuid_version_internal(byte* uuidPtr, uint layoutCode);
	private static unsafe uint uuid_version(byte* uuidPtr, uint layoutCode) =>
		OperatingSystem.IsBrowser() ? uuid_version_browser(uuidPtr, layoutCode)
			: OperatingSystem.IsIOS() ? uuid_version_internal(uuidPtr, layoutCode)
			: uuid_version_native(uuidPtr, layoutCode);

	[LibraryImport("hyperuuid", EntryPoint = "uuid_variant")]
	private static unsafe partial uint uuid_variant_native(byte* uuidPtr);
	[LibraryImport("*", EntryPoint = "uuid_variant")]
	private static unsafe partial uint uuid_variant_browser(byte* uuidPtr);
	[LibraryImport("__Internal", EntryPoint = "uuid_variant")]
	private static unsafe partial uint uuid_variant_internal(byte* uuidPtr);
	private static unsafe uint uuid_variant(byte* uuidPtr) =>
		OperatingSystem.IsBrowser() ? uuid_variant_browser(uuidPtr)
			: OperatingSystem.IsIOS() ? uuid_variant_internal(uuidPtr)
			: uuid_variant_native(uuidPtr);

	[LibraryImport("hyperuuid", EntryPoint = "uuid_is_rfc")]
	private static unsafe partial uint uuid_is_rfc_native(byte* uuidPtr, uint version, uint layoutCode);
	[LibraryImport("*", EntryPoint = "uuid_is_rfc")]
	private static unsafe partial uint uuid_is_rfc_browser(byte* uuidPtr, uint version, uint layoutCode);
	[LibraryImport("__Internal", EntryPoint = "uuid_is_rfc")]
	private static unsafe partial uint uuid_is_rfc_internal(byte* uuidPtr, uint version, uint layoutCode);
	private static unsafe uint uuid_is_rfc(byte* uuidPtr, uint version, uint layoutCode) =>
		OperatingSystem.IsBrowser() ? uuid_is_rfc_browser(uuidPtr, version, layoutCode)
			: OperatingSystem.IsIOS() ? uuid_is_rfc_internal(uuidPtr, version, layoutCode)
			: uuid_is_rfc_native(uuidPtr, version, layoutCode);

	[LibraryImport("hyperuuid", EntryPoint = "uuid_get_timestamp")]
	private static unsafe partial uint uuid_get_timestamp_native(byte* uuidPtr, uint layoutCode, ulong* millisOut);
	[LibraryImport("*", EntryPoint = "uuid_get_timestamp")]
	private static unsafe partial uint uuid_get_timestamp_browser(byte* uuidPtr, uint layoutCode, ulong* millisOut);
	[LibraryImport("__Internal", EntryPoint = "uuid_get_timestamp")]
	private static unsafe partial uint uuid_get_timestamp_internal(byte* uuidPtr, uint layoutCode, ulong* millisOut);
	private static unsafe uint uuid_get_timestamp(byte* uuidPtr, uint layoutCode, ulong* millisOut) =>
		OperatingSystem.IsBrowser() ? uuid_get_timestamp_browser(uuidPtr, layoutCode, millisOut)
			: OperatingSystem.IsIOS() ? uuid_get_timestamp_internal(uuidPtr, layoutCode, millisOut)
			: uuid_get_timestamp_native(uuidPtr, layoutCode, millisOut);

	[LibraryImport("hyperuuid", EntryPoint = "uuid_v6_unix_millis_in")]
	private static unsafe partial ulong uuid_v6_unix_millis_in_native(byte* uuidPtr, uint layoutCode);
	[LibraryImport("*", EntryPoint = "uuid_v6_unix_millis_in")]
	private static unsafe partial ulong uuid_v6_unix_millis_in_browser(byte* uuidPtr, uint layoutCode);
	[LibraryImport("__Internal", EntryPoint = "uuid_v6_unix_millis_in")]
	private static unsafe partial ulong uuid_v6_unix_millis_in_internal(byte* uuidPtr, uint layoutCode);
	private static unsafe ulong uuid_v6_unix_millis_in(byte* uuidPtr, uint layoutCode) =>
		OperatingSystem.IsBrowser() ? uuid_v6_unix_millis_in_browser(uuidPtr, layoutCode)
			: OperatingSystem.IsIOS() ? uuid_v6_unix_millis_in_internal(uuidPtr, layoutCode)
			: uuid_v6_unix_millis_in_native(uuidPtr, layoutCode);

	[LibraryImport("hyperuuid", EntryPoint = "uuid_v7_unix_millis_in")]
	private static unsafe partial ulong uuid_v7_unix_millis_in_native(byte* uuidPtr, uint layoutCode);
	[LibraryImport("*", EntryPoint = "uuid_v7_unix_millis_in")]
	private static unsafe partial ulong uuid_v7_unix_millis_in_browser(byte* uuidPtr, uint layoutCode);
	[LibraryImport("__Internal", EntryPoint = "uuid_v7_unix_millis_in")]
	private static unsafe partial ulong uuid_v7_unix_millis_in_internal(byte* uuidPtr, uint layoutCode);
	private static unsafe ulong uuid_v7_unix_millis_in(byte* uuidPtr, uint layoutCode) =>
		OperatingSystem.IsBrowser() ? uuid_v7_unix_millis_in_browser(uuidPtr, layoutCode)
			: OperatingSystem.IsIOS() ? uuid_v7_unix_millis_in_internal(uuidPtr, layoutCode)
			: uuid_v7_unix_millis_in_native(uuidPtr, layoutCode);

	// Batch calls marshal through a byte scratch buffer rather than Span<Guid> directly —
	// Guid's in-memory field layout isn't RFC-byte-order (it's mixed-endian and not
	// guaranteed stable across runtimes), so each 16-byte chunk still needs the same
	// `new Guid(chunk, bigEndian: true)` conversion the single-item calls already do.
	// Fixed-size stackalloc + ArrayPool-above-threshold mirrors GuidV7.Fill in the
	// SequentialGuid library this crate is ported from.
	const int BatchStackThresholdBytes = 256;

	static void RequireWholeUuids(Span<byte> destination, string paramName)
	{
		if (destination.Length % 16 != 0)
			throw new ArgumentException(
				$"Destination length must be a multiple of 16 (one whole UUID per 16 bytes); got {destination.Length}.",
				paramName);
	}

	// A Span<Guid> fill marshals through a byte scratch buffer of Length * 16 bytes; past this
	// many elements that product no longer fits an int, and no single buffer could hold it.
	const int MaxGuidsPerFill = int.MaxValue / 16;

	static void RequireFillable(Span<Guid> destination, string paramName)
	{
		if (destination.Length > MaxGuidsPerFill)
			throw new ArgumentException(
				$"Destination holds {destination.Length} UUIDs; a single fill takes at most {MaxGuidsPerFill}.",
				paramName);
	}

	static void ThrowOnBatchFailure(int rc, string entryPoint, string outOfRangeMessage, string countParamName)
	{
		if (rc == 0)
			return;
		throw rc switch
		{
			2 => new ArgumentOutOfRangeException("unixMilliseconds", outOfRangeMessage),
			3 => new ArgumentException("The batch is too large to address on this platform.", countParamName),
			4 => new ArgumentOutOfRangeException(countParamName,
				$"A single version 7 batch takes at most {MaxV7Batch} UUIDs (the 26-bit counter space)."),
			_ => new InvalidOperationException($"{entryPoint} failed with code {rc} (random source failure)."),
		};
	}

	/// <summary>
	/// The most UUIDs one version 7 batch (<see cref="FillV7(Span{Guid}, long)"/>,
	/// <see cref="NewV7Batch(int, long)"/> and their overloads) mints: 67,108,864, the size of
	/// the 26-bit counter that orders UUIDs within a millisecond.
	/// </summary>
	/// <remarks>
	/// Every batch up to this size is in strictly increasing order. The counter is one
	/// process-wide sequence, so a batch can straddle the point where it wraps back to 0; the
	/// UUIDs from there on carry a timestamp one millisecond later than the one supplied rather
	/// than sorting before the ones ahead of them. A larger batch would have to reuse counter
	/// values within one millisecond, so it is refused: the throwing forms throw
	/// <see cref="ArgumentOutOfRangeException"/> and the <c>Try</c> forms return
	/// <see langword="false"/>. Version 6 has no counter and no such limit.
	/// <para>
	/// The roll-forward orders one batch, not the stream. The next batch or <c>NewV7</c> call in
	/// the same real millisecond starts its counter just past the wrap and carries the supplied
	/// timestamp, so it sorts before the previous batch's tail, stamped a millisecond later; two
	/// single calls either side of the wrap in one millisecond sort in reverse the same way.
	/// It happens at most once per <see cref="MaxV7Batch"/> UUIDs the process mints.
	/// </para>
	/// </remarks>
	public const int MaxV7Batch = 1 << 26;

	static readonly Lazy<Version?> _nativeVersion = new(ProbeNativeVersion, LazyThreadSafetyMode.PublicationOnly);

	// The one place the binding catches: loading is the caller's environment, not their
	// data, and the point of the probe is to answer "did the native library resolve" without
	// making the first real UUID the thing that finds out. Every other method lets a load
	// failure propagate — the Try* forms included, which report the native layer's own
	// return codes and nothing else.
	static Version? ProbeNativeVersion()
	{
		try
		{
			var packed = hyperuuid_version();
			return new Version((int)(packed >> 16), (int)((packed >> 8) & 0xFF), (int)(packed & 0xFF));
		}
		catch (Exception e) when (e is DllNotFoundException or EntryPointNotFoundException
			or BadImageFormatException or PlatformNotSupportedException or TypeInitializationException)
		{
			return null;
		}
	}

	/// <summary>
	/// <see langword="true"/> when the native library resolved and answered the version
	/// probe. Probed once, then cached; a <see langword="false"/> is permanent for the
	/// process. A consumer keeping a managed fallback (<see cref="Guid.NewGuid"/>,
	/// <see cref="Guid.CreateVersion7()"/>) for platforms the package does not cover gates on
	/// this instead of catching <see cref="DllNotFoundException"/> around its first call.
	/// </summary>
	/// <remarks>
	/// Also <see langword="false"/> when a library did load but predates the
	/// <c>hyperuuid_version</c> export (0.3.0 and earlier) — a stale binary beside a newer
	/// binding is the mismatch this probe exists to name. It says nothing about <em>which</em>
	/// version answered; that is <see cref="NativeVersion"/>.
	/// </remarks>
	public static bool IsAvailable => _nativeVersion.Value is not null;

	/// <summary>
	/// The native core's own version — <c>major.minor.patch</c> as the library reports it,
	/// or <see langword="null"/> when it did not load (see <see cref="IsAvailable"/>).
	/// Compare against this assembly's version to name a mismatch before the first UUID.
	/// </summary>
	public static Version? NativeVersion => _nativeVersion.Value;

	/// <summary>Well-known namespace UUIDs defined in RFC 9562 Section 6.6.</summary>
	public static class Namespaces
	{
		/// <summary>The DNS namespace UUID.</summary>
		public static readonly Guid Dns = new("6ba7b810-9dad-11d1-80b4-00c04fd430c8");
		/// <summary>The URL namespace UUID.</summary>
		public static readonly Guid Url = new("6ba7b811-9dad-11d1-80b4-00c04fd430c8");
		/// <summary>The ISO OID namespace UUID.</summary>
		public static readonly Guid Oid = new("6ba7b812-9dad-11d1-80b4-00c04fd430c8");
		/// <summary>The X.500 DN namespace UUID.</summary>
		public static readonly Guid X500 = new("6ba7b814-9dad-11d1-80b4-00c04fd430c8");
	}

	/// <summary>The RFC 9562 §5.9 Nil UUID — all 128 bits zero. Equivalent to <see cref="Guid.Empty"/>.</summary>
	public static readonly Guid Nil = Guid.Empty;

	/// <summary>The RFC 9562 §5.10 Max UUID — all 128 bits one.</summary>
	public static readonly Guid Max = new(new byte[]
	{
		0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
		0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
	});

	/// <summary>Creates a random UUID version 4 (RFC 9562 §5.4).</summary>
	public static Guid NewV4()
	{
		var rc = CoreNewV4(out var result);
		if (rc != 0)
			throw new InvalidOperationException($"uuid_new_v4 failed with code {rc} (random source failure).");
		return result;
	}

	/// <summary>
	/// Non-throwing counterpart to <see cref="NewV4"/> — returns <see langword="false"/> and
	/// leaves <paramref name="result"/> as <see cref="Guid.Empty"/> if the native random source
	/// fails, instead of throwing.
	/// </summary>
	/// <remarks>
	/// No exception ever crosses the P/Invoke boundary in either form — the native layer signals
	/// failure with an <c>int</c> return code (0 success, 1 random-source failure, 2 timestamp out
	/// of range; see <c>rust/src/ffi.rs</c>) and it's the managed wrapper that decides whether to
	/// translate that code into a <see langword="throw"/>. This overload simply doesn't, which is
	/// what a <c>Result</c>-shaped call site wants: a failure it can branch on without paying for
	/// a managed exception or wrapping every call in a <c>try</c>/<c>catch</c>.
	/// </remarks>
	public static bool TryNewV4(out Guid result) => CoreNewV4(out result) == 0;

	static unsafe int CoreNewV4(out Guid result)
	{
		Span<byte> buf = stackalloc byte[16];
		int rc;
		fixed (byte* p = buf)
		{
			rc = uuid_new_v4(p);
		}
		result = rc == 0 ? new Guid(buf, bigEndian: true) : default;
		return rc;
	}

	/// <summary>
	/// Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a name,
	/// hashed as its UTF-8 encoding.
	/// </summary>
	/// <exception cref="ArgumentNullException"><paramref name="name"/> is <see langword="null"/>.</exception>
	public static Guid NewV5(Guid namespaceId, string name)
	{
		ArgumentNullException.ThrowIfNull(name);
		return NewV5(namespaceId, name.AsSpan());
	}

	/// <summary>
	/// Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a name held
	/// as UTF-16 text, hashed as its UTF-8 encoding — the same result as the <see cref="string"/>
	/// overload for the same characters, without materializing a <see cref="string"/> first.
	/// </summary>
	/// <remarks>
	/// An empty name is valid: RFC 9562 hashes the namespace followed by zero name bytes. A
	/// lone surrogate has no UTF-8 encoding and is hashed as U+FFFD, exactly as the
	/// <see cref="string"/> overload always has; pass bytes to
	/// <see cref="NewV5(Guid, ReadOnlySpan{byte})"/> when the name is not text.
	/// </remarks>
	public static Guid NewV5(Guid namespaceId, ReadOnlySpan<char> name)
	{
		var maxByteCount = System.Text.Encoding.UTF8.GetMaxByteCount(name.Length);
		Span<byte> stackBuf = stackalloc byte[BatchStackThresholdBytes];
		byte[]? rented = null;
		var buffer = maxByteCount <= BatchStackThresholdBytes
			? stackBuf[..maxByteCount]
			: (rented = ArrayPool<byte>.Shared.Rent(maxByteCount)).AsSpan(0, maxByteCount);
		try
		{
			var len = System.Text.Encoding.UTF8.GetBytes(name, buffer);
			return NewV5(namespaceId, buffer[..len]);
		}
		finally
		{
			if (rented is not null) ArrayPool<byte>.Shared.Return(rented);
		}
	}

	/// <summary>
	/// Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and raw name
	/// bytes — hashed exactly as given, with no encoding step and no validation, so a name
	/// that is not text at all is as good as one that is. An empty name is valid.
	/// </summary>
	public static unsafe Guid NewV5(Guid namespaceId, ReadOnlySpan<byte> name)
	{
		Span<byte> ns = stackalloc byte[16];
		namespaceId.TryWriteBytes(ns, bigEndian: true, out _);
		Span<byte> outBuf = stackalloc byte[16];

		// No return code to translate: version 5 is a hash of the caller's own bytes, with no
		// random source and no timestamp, so the native call has no failure mode and always
		// returns 0 (see rust/src/ffi.rs). An empty name crosses as a null pointer, which the
		// core never dereferences when the length is 0.
		fixed (byte* nsPtr = ns)
		fixed (byte* namePtr = name)
		fixed (byte* outPtr = outBuf)
		{
			uuid_new_v5(nsPtr, name.IsEmpty ? null : namePtr, (uint)name.Length, outPtr);
		}
		return new Guid(outBuf, bigEndian: true);
	}

	/// <summary>
	/// Creates a time-sortable UUID version 6 (RFC 9562 §5.6), a field-compatible reordering
	/// of version 1 for better sort/index locality, using the current UTC time.
	/// </summary>
	public static Guid NewV6() => NewV6(DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());

	/// <summary>Creates a time-sortable UUID version 6 (RFC 9562 §5.6) from a <see cref="DateTimeOffset"/>.</summary>
	public static Guid NewV6(DateTimeOffset timestamp) => NewV6(timestamp.ToUnixTimeMilliseconds());

	/// <summary>
	/// Creates a time-sortable UUID version 6 (RFC 9562 §5.6) from a Unix-epoch millisecond
	/// timestamp. <c>clock_seq</c> and <c>node</c> are randomly generated on every call —
	/// unlike version 7, there is no monotonic counter, so calls within the same millisecond
	/// are not guaranteed to sort in creation order.
	/// </summary>
	/// <remarks>
	/// The v6 timestamp field counts from the 1582 Gregorian epoch and could hold an instant
	/// before 1970, but this API is Unix-millisecond only, on purpose: a negative
	/// <paramref name="unixMilliseconds"/> is rejected rather than encoded, the same floor
	/// <see cref="NewV7(long)"/> has.
	/// </remarks>
	/// <exception cref="ArgumentOutOfRangeException">
	/// <paramref name="unixMilliseconds"/> is negative, or past what the 60-bit v6 timestamp
	/// field can hold (around the year 5236).
	/// </exception>
	/// <exception cref="InvalidOperationException">The native random source failed.</exception>
	public static Guid NewV6(long unixMilliseconds)
	{
		var rc = CoreNewV6(unixMilliseconds, out var result);
		if (rc != 0)
		{
			throw rc switch
			{
				2 => new ArgumentOutOfRangeException(nameof(unixMilliseconds),
					"Unix millisecond timestamp must be non-negative and fit the 60-bit v6 timestamp field."),
				_ => new InvalidOperationException($"uuid_new_v6 failed with code {rc} (random source failure)."),
			};
		}
		return result;
	}

	/// <summary>
	/// Non-throwing counterpart to <see cref="NewV6(long)"/> — returns <see langword="false"/> for
	/// both failure modes the native layer reports (random-source failure, and a
	/// <paramref name="unixMilliseconds"/> that is negative or doesn't fit the 60-bit v6 timestamp
	/// field) rather than throwing. See <see cref="TryNewV4"/> for why this is the cheaper shape at a
	/// <c>Result</c>-style call site.
	/// </summary>
	public static bool TryNewV6(long unixMilliseconds, out Guid result) =>
		CoreNewV6(unixMilliseconds, out result) == 0;

	/// <summary>
	/// Non-throwing counterpart to <see cref="NewV6()"/>, using the current UTC time.
	/// </summary>
	public static bool TryNewV6(out Guid result) =>
		TryNewV6(DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(), out result);

	static unsafe int CoreNewV6(long unixMilliseconds, out Guid result)
	{
		Span<byte> buf = stackalloc byte[16];
		int rc;
		fixed (byte* p = buf)
		{
			rc = uuid_new_v6(unixMilliseconds, p);
		}
		result = rc == 0 ? new Guid(buf, bigEndian: true) : default;
		return rc;
	}

	/// <summary>
	/// Recovers the Unix-epoch millisecond timestamp embedded in a version 6 UUID's timestamp
	/// field. Only meaningful when <paramref name="uuid"/>'s version nibble is 6 — the RFC
	/// 9562 bit layout doesn't distinguish "not a v6 UUID" from "v6 UUID with a very early
	/// timestamp", so the caller is responsible for checking that first if it matters.
	/// </summary>
	/// <remarks>
	/// Saturates to 0 for an embedded timestamp before 1970. That is a legitimate v6 value —
	/// the field counts from 1582 — but one only another generator can have minted, since
	/// <see cref="NewV6(long)"/> rejects a negative timestamp; it reads back as the Unix epoch
	/// rather than as a negative count, matching this API's Unix-millisecond-only surface.
	/// </remarks>
	public static unsafe long V6UnixMillis(Guid uuid)
	{
		Span<byte> bytes = stackalloc byte[16];
		uuid.TryWriteBytes(bytes, bigEndian: true, out _);
		fixed (byte* p = bytes)
		{
			return (long)uuid_v6_unix_millis(p);
		}
	}

	/// <summary>
	/// Recovers the UTC timestamp embedded in a version 6 UUID as a <see cref="DateTimeOffset"/>.
	/// Unlike <see cref="V7Timestamp(Guid)"/>, this can't throw <see cref="ArgumentOutOfRangeException"/>:
	/// v6's 60-bit tick count, offset from the 1582 UUID epoch rather than 1970, tops out
	/// around the year 5236 — well short of <see cref="DateTimeOffset"/>'s own year-9999 ceiling.
	/// A timestamp before 1970 reads back as the Unix epoch; see <see cref="V6UnixMillis(Guid)"/>.
	/// </summary>
	public static DateTimeOffset V6Timestamp(Guid uuid) =>
		DateTimeOffset.FromUnixTimeMilliseconds(V6UnixMillis(uuid));

	/// <summary>
	/// Fills <paramref name="destination"/> with time-sortable version 6 UUIDs sharing one
	/// timestamp capture — one native call and one random-bytes fetch instead of
	/// <paramref name="destination"/>'s length worth of each.
	/// </summary>
	/// <exception cref="ArgumentException">
	/// <paramref name="destination"/> is longer than a single fill takes (more than
	/// <c>int.MaxValue / 16</c> elements).
	/// </exception>
	/// <exception cref="ArgumentOutOfRangeException">
	/// <paramref name="unixMilliseconds"/> is out of range (see <see cref="NewV6(long)"/>).
	/// </exception>
	public static void FillV6(Span<Guid> destination, long unixMilliseconds)
	{
		RequireFillable(destination, nameof(destination));
		ThrowOnBatchFailure(CoreFillV6(destination, unixMilliseconds), "uuid_new_v6_batch",
			"Unix millisecond timestamp must be non-negative and fit the 60-bit v6 timestamp field.",
			nameof(destination));
	}

	/// <summary>
	/// Non-throwing counterpart to <see cref="FillV6(Span{Guid}, long)"/> — returns
	/// <see langword="false"/> instead of throwing when the native call reports a random-source
	/// failure or an out-of-range <paramref name="unixMilliseconds"/>, and for a
	/// <paramref name="destination"/> too long for a single fill. On failure
	/// <paramref name="destination"/> is left untouched. See <see cref="TryNewV4"/> for why.
	/// </summary>
	public static bool TryFillV6(Span<Guid> destination, long unixMilliseconds) =>
		destination.Length <= MaxGuidsPerFill && CoreFillV6(destination, unixMilliseconds) == 0;

	/// <summary>
	/// Fills <paramref name="destination"/> with raw RFC 9562-ordered version 6 UUID bytes —
	/// 16 per UUID, contiguous, no <see cref="Guid"/> anywhere on the path.
	/// </summary>
	/// <remarks>
	/// This is the allocation-free, conversion-free form of
	/// <see cref="FillV6(Span{Guid}, long)"/>: the native core already writes the batch as one
	/// contiguous block of RFC-ordered bytes, so handing it the caller's own buffer means one
	/// native call and <em>zero</em> managed per-element work. The <see cref="Guid"/> overload has
	/// to marshal through a scratch buffer and run a
	/// <c>new Guid(chunk, bigEndian: true)</c> conversion per element, because
	/// <see cref="Guid"/>'s in-memory field layout is mixed-endian and isn't the RFC byte order.
	/// Prefer this overload when the destination is a wire buffer, a database parameter, or
	/// anything else that wants RFC bytes rather than <see cref="Guid"/> values.
	/// <para>
	/// <paramref name="destination"/>'s length must be an exact multiple of 16; anything else is a
	/// caller error and throws.
	/// </para>
	/// </remarks>
	/// <exception cref="ArgumentException">
	/// <paramref name="destination"/>'s length is not a multiple of 16.
	/// </exception>
	public static void FillV6(Span<byte> destination, long unixMilliseconds)
	{
		RequireWholeUuids(destination, nameof(destination));
		ThrowOnBatchFailure(CoreFillV6Bytes(destination, unixMilliseconds), "uuid_new_v6_batch",
			"Unix millisecond timestamp must be non-negative and fit the 60-bit v6 timestamp field.",
			nameof(destination));
	}

	/// <summary>
	/// Non-throwing counterpart to <see cref="FillV6(Span{byte}, long)"/>. Returns
	/// <see langword="false"/> — rather than throwing — for a native failure <em>and</em> for a
	/// <paramref name="destination"/> whose length isn't a multiple of 16, matching the BCL's own
	/// <c>Try</c> convention (<see cref="Guid.TryWriteBytes(Span{byte})"/> likewise returns
	/// <see langword="false"/> for a badly sized destination rather than throwing).
	/// </summary>
	public static bool TryFillV6(Span<byte> destination, long unixMilliseconds) =>
		destination.Length % 16 == 0 && CoreFillV6Bytes(destination, unixMilliseconds) == 0;

	static unsafe int CoreFillV6(Span<Guid> destination, long unixMilliseconds)
	{
		if (destination.IsEmpty)
			return 0;

		int totalBytes = destination.Length * 16;
		Span<byte> stackBuf = stackalloc byte[BatchStackThresholdBytes];
		byte[]? rented = null;
		Span<byte> buf = totalBytes <= BatchStackThresholdBytes
			? stackBuf[..totalBytes]
			: (rented = ArrayPool<byte>.Shared.Rent(totalBytes)).AsSpan(0, totalBytes);
		try
		{
			int rc;
			fixed (byte* p = buf)
			{
				rc = uuid_new_v6_batch(unixMilliseconds, (uint)destination.Length, p);
			}
			if (rc != 0)
				return rc;
			for (int i = 0; i < destination.Length; i++)
			{
				destination[i] = new Guid(buf.Slice(i * 16, 16), bigEndian: true);
			}
			return 0;
		}
		finally
		{
			if (rented is not null)
				ArrayPool<byte>.Shared.Return(rented);
		}
	}

	static unsafe int CoreFillV6Bytes(Span<byte> destination, long unixMilliseconds)
	{
		if (destination.IsEmpty)
			return 0;
		fixed (byte* p = destination)
		{
			return uuid_new_v6_batch(unixMilliseconds, (uint)(destination.Length / 16), p);
		}
	}

	/// <summary>
	/// Fills <paramref name="destination"/> with time-sortable version 6 UUIDs using the
	/// current UTC time.
	/// </summary>
	public static void FillV6(Span<Guid> destination) =>
		FillV6(destination, DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());

	/// <summary>
	/// Creates an array of <paramref name="count"/> time-sortable version 6 UUIDs sharing one
	/// timestamp capture. <c>clock_seq</c> and <c>node</c> are randomly generated per item —
	/// unlike version 7, there is no monotonic counter, so items are not guaranteed to sort
	/// in creation order.
	/// </summary>
	/// <exception cref="ArgumentOutOfRangeException">
	/// <paramref name="count"/> is negative, or <paramref name="unixMilliseconds"/> is out of
	/// range (see <see cref="NewV6(long)"/>).
	/// </exception>
	public static Guid[] NewV6Batch(int count, long unixMilliseconds)
	{
		ArgumentOutOfRangeException.ThrowIfNegative(count);
		var result = new Guid[count];
		FillV6(result, unixMilliseconds);
		return result;
	}

	/// <summary>
	/// Creates an array of <paramref name="count"/> time-sortable version 6 UUIDs using the
	/// current UTC time.
	/// </summary>
	public static Guid[] NewV6Batch(int count) =>
		NewV6Batch(count, DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());

	/// <summary>Creates a time-sortable UUID version 7 (RFC 9562 §6.2) using the current UTC time.</summary>
	public static Guid NewV7() => NewV7(DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());

	/// <summary>Creates a time-sortable UUID version 7 (RFC 9562 §6.2) from a <see cref="DateTimeOffset"/>.</summary>
	public static Guid NewV7(DateTimeOffset timestamp) => NewV7(timestamp.ToUnixTimeMilliseconds());

	/// <summary>Creates a time-sortable UUID version 7 (RFC 9562 §6.2) from a Unix-epoch millisecond timestamp.</summary>
	public static Guid NewV7(long unixMilliseconds)
	{
		var rc = CoreNewV7(unixMilliseconds, out var result);
		if (rc != 0)
		{
			throw rc switch
			{
				2 => new ArgumentOutOfRangeException(nameof(unixMilliseconds),
					"Unix millisecond timestamp must be non-negative and fit within 48 bits."),
				_ => new InvalidOperationException($"uuid_new_v7 failed with code {rc} (random source failure)."),
			};
		}
		return result;
	}

	/// <summary>
	/// Non-throwing counterpart to <see cref="NewV7(long)"/> — returns <see langword="false"/> for
	/// both failure modes the native layer reports (random-source failure, and a
	/// <paramref name="unixMilliseconds"/> that is negative or doesn't fit 48 bits) rather than
	/// throwing. See <see cref="TryNewV4"/> for why this is the cheaper shape at a
	/// <c>Result</c>-style call site.
	/// </summary>
	public static bool TryNewV7(long unixMilliseconds, out Guid result) =>
		CoreNewV7(unixMilliseconds, out result) == 0;

	/// <summary>
	/// Non-throwing counterpart to <see cref="NewV7()"/>, using the current UTC time.
	/// </summary>
	public static bool TryNewV7(out Guid result) =>
		TryNewV7(DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(), out result);

	static unsafe int CoreNewV7(long unixMilliseconds, out Guid result)
	{
		Span<byte> buf = stackalloc byte[16];
		int rc;
		fixed (byte* p = buf)
		{
			rc = uuid_new_v7(unixMilliseconds, p);
		}
		result = rc == 0 ? new Guid(buf, bigEndian: true) : default;
		return rc;
	}

	/// <summary>
	/// Recovers the Unix-epoch millisecond timestamp embedded in a version 7 UUID's
	/// <c>unix_ts_ms</c> field. Only meaningful when <paramref name="uuid"/>'s version nibble
	/// is 7 — the RFC 9562 bit layout doesn't distinguish "not a v7 UUID" from "v7 UUID with a
	/// very early timestamp", so the caller is responsible for checking that first if it matters.
	/// </summary>
	public static unsafe long V7UnixMillis(Guid uuid)
	{
		Span<byte> bytes = stackalloc byte[16];
		uuid.TryWriteBytes(bytes, bigEndian: true, out _);
		fixed (byte* p = bytes)
		{
			return (long)uuid_v7_unix_millis(p);
		}
	}

	/// <summary>
	/// Recovers the UTC timestamp embedded in a version 7 UUID as a <see cref="DateTimeOffset"/>.
	/// </summary>
	/// <exception cref="ArgumentOutOfRangeException">
	/// Thrown for a (spec-valid) embedded timestamp past year 9999 — the RFC's 48-bit
	/// millisecond field holds values up to the year 10889, but <see cref="DateTimeOffset"/>
	/// cannot represent a year beyond 9999.
	/// </exception>
	public static DateTimeOffset V7Timestamp(Guid uuid) =>
		DateTimeOffset.FromUnixTimeMilliseconds(V7UnixMillis(uuid));

	/// <summary>
	/// Recovers the UTC timestamp embedded in <paramref name="uuid"/>, or <see langword="null"/>
	/// if it isn't an RFC 9562 version 6 or 7 UUID: the variant is checked as well as the version
	/// nibble (see <see cref="IsRfc(Guid, int)"/>), so a 6 or 7 under another variant carries no
	/// timestamp. Unlike <see cref="V6Timestamp(Guid)"/>/<see cref="V7Timestamp(Guid)"/>,
	/// this reads the version itself first, so a caller doesn't need to already know (or
	/// separately check) which version <paramref name="uuid"/> is before asking — the core
	/// answers both, with no bit-layout logic here.
	/// Both of their edges come with it: a v6 timestamp before 1970 reads back as the Unix
	/// epoch, and a v7 timestamp past year 9999 throws <see cref="ArgumentOutOfRangeException"/>.
	/// </summary>
	public static DateTimeOffset? GetTimestamp(Guid uuid) => GetTimestamp(uuid, UuidLayout.Rfc9562);

	/// <summary>
	/// <see cref="GetTimestamp(Guid)"/> for a <paramref name="uuid"/> held in
	/// <paramref name="layout"/>'s byte order — in <see cref="UuidLayout.SqlServer"/>, the
	/// timestamp of a value straight from <see cref="V7ToSqlOrder(Guid)"/> or
	/// <see cref="V6ToSqlOrder(Guid)"/> (or a <c>uniqueidentifier</c> column), read from its
	/// permuted bytes in one native call with no conversion back first, or
	/// <see langword="null"/> for anything that isn't a SQL-ordered version 6 or 7 UUID (see
	/// <see cref="Version(Guid, UuidLayout)"/> for how the two are told apart). Same edges as
	/// <see cref="GetTimestamp(Guid)"/>.
	/// </summary>
	/// <exception cref="ArgumentOutOfRangeException"><paramref name="layout"/> is not a defined layout.</exception>
	public static unsafe DateTimeOffset? GetTimestamp(Guid uuid, UuidLayout layout)
	{
		var code = LayoutCode(layout);
		Span<byte> bytes = stackalloc byte[16];
		WriteBytes(uuid, layout, bytes);
		ulong millis;
		fixed (byte* p = bytes)
		{
			if (uuid_get_timestamp(p, code, &millis) == 0)
				return null;
		}
		return DateTimeOffset.FromUnixTimeMilliseconds((long)millis);
	}

	/// <summary>
	/// <see cref="V7UnixMillis(Guid)"/> for a <paramref name="uuid"/> held in
	/// <paramref name="layout"/>'s byte order, reading a SQL-ordered value's permuted bytes
	/// directly. Meaningful only for a genuine version 7 UUID in that layout;
	/// <see cref="IsRfc(Guid, int, UuidLayout)"/> is the check.
	/// </summary>
	/// <exception cref="ArgumentOutOfRangeException"><paramref name="layout"/> is not a defined layout.</exception>
	public static unsafe long V7UnixMillis(Guid uuid, UuidLayout layout)
	{
		var code = LayoutCode(layout);
		Span<byte> bytes = stackalloc byte[16];
		WriteBytes(uuid, layout, bytes);
		fixed (byte* p = bytes)
		{
			return (long)uuid_v7_unix_millis_in(p, code);
		}
	}

	/// <summary>
	/// <see cref="V7Timestamp(Guid)"/> for a <paramref name="uuid"/> held in
	/// <paramref name="layout"/>'s byte order; see <see cref="V7UnixMillis(Guid, UuidLayout)"/>.
	/// </summary>
	/// <exception cref="ArgumentOutOfRangeException">
	/// <paramref name="layout"/> is not a defined layout, or the embedded timestamp is past
	/// year 9999 (see <see cref="V7Timestamp(Guid)"/>).
	/// </exception>
	public static DateTimeOffset V7Timestamp(Guid uuid, UuidLayout layout) =>
		DateTimeOffset.FromUnixTimeMilliseconds(V7UnixMillis(uuid, layout));

	/// <summary>
	/// <see cref="V6UnixMillis(Guid)"/> for a <paramref name="uuid"/> held in
	/// <paramref name="layout"/>'s byte order, reading a SQL-ordered value's permuted bytes
	/// directly. Meaningful only for a genuine version 6 UUID in that layout;
	/// <see cref="IsRfc(Guid, int, UuidLayout)"/> is the check.
	/// </summary>
	/// <exception cref="ArgumentOutOfRangeException"><paramref name="layout"/> is not a defined layout.</exception>
	public static unsafe long V6UnixMillis(Guid uuid, UuidLayout layout)
	{
		var code = LayoutCode(layout);
		Span<byte> bytes = stackalloc byte[16];
		WriteBytes(uuid, layout, bytes);
		fixed (byte* p = bytes)
		{
			return (long)uuid_v6_unix_millis_in(p, code);
		}
	}

	/// <summary>
	/// <see cref="V6Timestamp(Guid)"/> for a <paramref name="uuid"/> held in
	/// <paramref name="layout"/>'s byte order; see <see cref="V6UnixMillis(Guid, UuidLayout)"/>.
	/// </summary>
	/// <exception cref="ArgumentOutOfRangeException"><paramref name="layout"/> is not a defined layout.</exception>
	public static DateTimeOffset V6Timestamp(Guid uuid, UuidLayout layout) =>
		DateTimeOffset.FromUnixTimeMilliseconds(V6UnixMillis(uuid, layout));

	// ---- Inspection ---------------------------------------------------------------------
	//
	// The layout knowledge lives in the core: these only get a Guid's bytes into the order
	// its layout means. That is the one subtle step, the same asymmetry the SQL-order
	// transforms below explain: an RFC-ordered Guid is read with `bigEndian: true`, but a
	// SQL-ordered one came from `new Guid(bytes)` (no conversion) and is read back the same
	// way, so its bytes are the SQL Server wire bytes the core expects.

	static uint LayoutCode(UuidLayout layout) => layout switch
	{
		UuidLayout.Rfc9562 => 1,
		UuidLayout.SqlServer => 2,
		_ => throw new ArgumentOutOfRangeException(nameof(layout), layout,
			"Layout must be UuidLayout.Rfc9562 or UuidLayout.SqlServer."),
	};

	static void WriteBytes(Guid uuid, UuidLayout layout, Span<byte> bytes)
	{
		if (layout == UuidLayout.SqlServer)
			uuid.TryWriteBytes(bytes);
		else
			uuid.TryWriteBytes(bytes, bigEndian: true, out _);
	}

	static UuidVariant VariantOf(uint code) => code switch
	{
		1 => UuidVariant.Ncs,
		2 => UuidVariant.Rfc9562,
		3 => UuidVariant.Microsoft,
		4 => UuidVariant.Future,
		_ => UuidVariant.Unspecified,
	};

	static void RequireSingleUuid(ReadOnlySpan<byte> uuid, string paramName)
	{
		if (uuid.Length != 16)
			throw new ArgumentException($"A UUID is exactly 16 bytes; got {uuid.Length}.", paramName);
	}

	/// <summary>
	/// The RFC 9562 version nibble of <paramref name="uuid"/>, 0 through 15 — 0 for
	/// <see cref="Nil"/>, 15 for <see cref="Max"/>. Says nothing about the variant: use
	/// <see cref="IsRfc(Guid, int)"/> when the question is "an RFC 9562 UUID of version N".
	/// </summary>
	/// <remarks>
	/// Reads <paramref name="uuid"/> as RFC 9562 order. For a value from
	/// <see cref="V7ToSqlOrder(Guid)"/> or <see cref="V6ToSqlOrder(Guid)"/>, or read back from a
	/// <c>uniqueidentifier</c> column, pass <see cref="UuidLayout.SqlServer"/> to
	/// <see cref="Version(Guid, UuidLayout)"/>: this overload reads the wrong bytes there.
	/// </remarks>
	public static int Version(Guid uuid) => Version(uuid, UuidLayout.Rfc9562);

	/// <summary>
	/// The version of a <paramref name="uuid"/> held in <paramref name="layout"/>'s byte order.
	/// </summary>
	/// <remarks>
	/// In <see cref="UuidLayout.Rfc9562"/> this is <see cref="Version(Guid)"/>.
	/// <see cref="UuidLayout.SqlServer"/> is defined only for the two versions that have a SQL
	/// Server order, and answers 6, 7, or 0 for anything that isn't a SQL-ordered version 6 or
	/// 7 RFC 9562 UUID. The version nibble lands at a different byte for each, and the other
	/// version's random bits can mimic it there, so the core checks the variant bits too, where
	/// each version puts them, and the answer never confuses the two. What it cannot know is
	/// whether <paramref name="uuid"/> really is in that layout; that is the caller's to track.
	/// An RFC-ordered value can happen to form a valid SQL-ordered version 7 (a random v4 does
	/// one time in 16), and is then reported as one.
	/// </remarks>
	/// <exception cref="ArgumentOutOfRangeException"><paramref name="layout"/> is not a defined layout.</exception>
	public static unsafe int Version(Guid uuid, UuidLayout layout)
	{
		var code = LayoutCode(layout);
		Span<byte> bytes = stackalloc byte[16];
		WriteBytes(uuid, layout, bytes);
		fixed (byte* p = bytes)
		{
			return (int)uuid_version(p, code);
		}
	}

	/// <summary>
	/// <see cref="Version(Guid, UuidLayout)"/> over 16 raw bytes already in
	/// <paramref name="layout"/>'s order (RFC 9562 network order by default).
	/// </summary>
	/// <exception cref="ArgumentException"><paramref name="uuid"/> is not exactly 16 bytes.</exception>
	/// <exception cref="ArgumentOutOfRangeException"><paramref name="layout"/> is not a defined layout.</exception>
	public static unsafe int Version(ReadOnlySpan<byte> uuid, UuidLayout layout = UuidLayout.Rfc9562)
	{
		var code = LayoutCode(layout);
		RequireSingleUuid(uuid, nameof(uuid));
		fixed (byte* p = uuid)
		{
			return (int)uuid_version(p, code);
		}
	}

	/// <summary>
	/// The variant field of <paramref name="uuid"/> (RFC 9562 §4.1): <see cref="UuidVariant.Ncs"/>
	/// for <see cref="Nil"/>, <see cref="UuidVariant.Future"/> for <see cref="Max"/>, and
	/// <see cref="UuidVariant.Rfc9562"/> for anything this library or
	/// <see cref="Guid.NewGuid"/>/<see cref="Guid.CreateVersion7()"/> mints. Never
	/// <see cref="UuidVariant.Unspecified"/>.
	/// </summary>
	/// <remarks>
	/// RFC 9562 order only, and there is no layout overload: in SQL Server order the variant
	/// sits at a different byte for each version. Don't feed it a <c>uniqueidentifier</c>
	/// read-back or a <see cref="V7ToSqlOrder(Guid)"/> result; to validate one of those, use
	/// <see cref="IsRfc(Guid, int, UuidLayout)"/> with <see cref="UuidLayout.SqlServer"/>, which
	/// checks the variant where that version puts it.
	/// </remarks>
	public static unsafe UuidVariant Variant(Guid uuid)
	{
		Span<byte> bytes = stackalloc byte[16];
		uuid.TryWriteBytes(bytes, bigEndian: true, out _);
		fixed (byte* p = bytes)
		{
			return VariantOf(uuid_variant(p));
		}
	}

	/// <summary><see cref="Variant(Guid)"/> over 16 raw RFC 9562-ordered bytes.</summary>
	/// <exception cref="ArgumentException"><paramref name="uuid"/> is not exactly 16 bytes.</exception>
	public static unsafe UuidVariant Variant(ReadOnlySpan<byte> uuid)
	{
		RequireSingleUuid(uuid, nameof(uuid));
		fixed (byte* p = uuid)
		{
			return VariantOf(uuid_variant(p));
		}
	}

	/// <summary>
	/// Whether <paramref name="uuid"/> is an RFC 9562 UUID of version
	/// <paramref name="version"/> — the RFC variant and that version nibble, in one native
	/// call. The guard to run before trusting a value's version-specific fields, such as a
	/// version 7's timestamp.
	/// </summary>
	/// <remarks>
	/// Reads <paramref name="uuid"/> as RFC 9562 order. A SQL-ordered value — from
	/// <see cref="V7ToSqlOrder(Guid)"/>, or read back from a <c>uniqueidentifier</c> column —
	/// needs <see cref="IsRfc(Guid, int, UuidLayout)"/> with <see cref="UuidLayout.SqlServer"/>:
	/// this overload reads the wrong bytes there and answers for whatever they happen to hold.
	/// There is deliberately no layout-agnostic form: the caller holding the value knows its
	/// order, and only the layout it names says which bytes to read.
	/// </remarks>
	public static bool IsRfc(Guid uuid, int version) => IsRfc(uuid, version, UuidLayout.Rfc9562);

	/// <summary>
	/// <see cref="IsRfc(Guid, int)"/> for a <paramref name="uuid"/> held in
	/// <paramref name="layout"/>'s byte order — in <see cref="UuidLayout.SqlServer"/>, only
	/// versions 6 and 7 can be <see langword="true"/> (see <see cref="Version(Guid, UuidLayout)"/>).
	/// A <paramref name="version"/> outside 0-15 is simply never matched.
	/// </summary>
	/// <exception cref="ArgumentOutOfRangeException"><paramref name="layout"/> is not a defined layout.</exception>
	public static unsafe bool IsRfc(Guid uuid, int version, UuidLayout layout)
	{
		var code = LayoutCode(layout);
		Span<byte> bytes = stackalloc byte[16];
		WriteBytes(uuid, layout, bytes);
		fixed (byte* p = bytes)
		{
			return uuid_is_rfc(p, (uint)version, code) != 0;
		}
	}

	/// <summary>
	/// <see cref="IsRfc(Guid, int, UuidLayout)"/> over 16 raw bytes already in
	/// <paramref name="layout"/>'s order (RFC 9562 network order by default).
	/// </summary>
	/// <exception cref="ArgumentException"><paramref name="uuid"/> is not exactly 16 bytes.</exception>
	/// <exception cref="ArgumentOutOfRangeException"><paramref name="layout"/> is not a defined layout.</exception>
	public static unsafe bool IsRfc(ReadOnlySpan<byte> uuid, int version, UuidLayout layout = UuidLayout.Rfc9562)
	{
		var code = LayoutCode(layout);
		RequireSingleUuid(uuid, nameof(uuid));
		fixed (byte* p = uuid)
		{
			return uuid_is_rfc(p, (uint)version, code) != 0;
		}
	}

	/// <summary>
	/// Converts an RFC 9562-ordered version 7 <paramref name="uuid"/> to the byte order SQL
	/// Server's <c>uniqueidentifier</c> needs on the wire to sort by creation order.
	/// </summary>
	/// <remarks>
	/// <see cref="System.Data.SqlTypes.SqlGuid"/> comparison — and therefore T-SQL
	/// <c>ORDER BY</c> on a <c>uniqueidentifier</c> column — doesn't compare a GUID's 16 bytes
	/// left to right; it uses a fixed, non-sequential byte significance order. This moves the
	/// timestamp and counter (the two fields that determine creation order) into that
	/// comparison's most-significant bytes, and moves the trailing entropy, which carries no
	/// ordering information, into the least-significant ones as one intact block. The result
	/// is exactly what <see cref="Guid.ToByteArray()"/> on the returned value needs to produce
	/// to sort correctly once written to SQL Server — pass the result straight through
	/// ADO.NET as you would any other <see cref="Guid"/> parameter. Same permutation this
	/// project's own <see href="https://github.com/NorseArchitecture/Svartalfheim">Svartalfheim</see>
	/// implements, ported here from the native Rust core instead of reimplemented in C#, so
	/// every binding in this repo (not just this one) gets it from one verified source.
	/// Meaningful only for a genuine version 7 UUID; see <see cref="V6ToSqlOrder(Guid)"/> for v6.
	/// </remarks>
	public static unsafe Guid V7ToSqlOrder(Guid uuid)
	{
		Span<byte> bytes = stackalloc byte[16];
		uuid.TryWriteBytes(bytes, bigEndian: true, out _);
		fixed (byte* p = bytes)
		{
			uuid_v7_to_sql_order(p);
		}
		// Not bigEndian: true — the native call already rewrote these bytes into the exact
		// layout Guid.ToByteArray() needs to reproduce for SQL Server, so the default
		// constructor (no further byte-order conversion) is the correct one here.
		return new Guid(bytes);
	}

	/// <summary>
	/// Inverse of <see cref="V7ToSqlOrder(Guid)"/> — converts a SQL-Server-ordered version 7
	/// <paramref name="uuid"/> (as read back via <see cref="Guid.ToByteArray()"/>) back to
	/// RFC 9562 order.
	/// </summary>
	public static unsafe Guid V7FromSqlOrder(Guid uuid)
	{
		Span<byte> bytes = stackalloc byte[16];
		// Not bigEndian: true — read back the same native layout V7ToSqlOrder wrote.
		uuid.TryWriteBytes(bytes);
		fixed (byte* p = bytes)
		{
			uuid_v7_to_rfc_order(p);
		}
		return new Guid(bytes, bigEndian: true);
	}

	/// <summary>
	/// Converts an RFC 9562-ordered version 6 <paramref name="uuid"/> to the byte order SQL
	/// Server's <c>uniqueidentifier</c> needs on the wire to sort by creation order.
	/// </summary>
	/// <remarks>
	/// Same <see cref="System.Data.SqlTypes.SqlGuid"/> significance order as
	/// <see cref="V7ToSqlOrder(Guid)"/>, applied to v6's very different field layout. v6 has no
	/// monotonic counter the way v7 does; the only field that determines its creation order is
	/// the 60-bit timestamp itself, so this moves that whole timestamp — most significant
	/// chunk first — into the comparison's most significant bytes, and relocates
	/// <c>clock_seq</c>/<c>node</c> (no ordering value — randomly generated per call, not a
	/// counter) into the remaining bytes. Version and variant end up at different byte offsets
	/// than <see cref="V7ToSqlOrder(Guid)"/>'s result (octet 8's top nibble and octet 6's top two
	/// bits here, not 7/8) — fine, since the two versions are separate methods and a caller
	/// always knows which one it's calling.
	/// <para>
	/// Unlike v7, two version 6 UUIDs minted at the same millisecond have identical timestamp
	/// bits — <c>clock_seq</c>/<c>node</c> are independently random, not a counter — so this
	/// doesn't (and can't) make same-millisecond v6 UUIDs sort in creation order any more than
	/// plain RFC order already does. Distinct timestamps sort correctly; same-timestamp ties
	/// don't, by the RFC's own v6 design, not a limitation introduced here.
	/// </para>
	/// Meaningful only for a genuine version 6 UUID.
	/// </remarks>
	public static unsafe Guid V6ToSqlOrder(Guid uuid)
	{
		Span<byte> bytes = stackalloc byte[16];
		uuid.TryWriteBytes(bytes, bigEndian: true, out _);
		fixed (byte* p = bytes)
		{
			uuid_v6_to_sql_order(p);
		}
		return new Guid(bytes);
	}

	/// <summary>
	/// Inverse of <see cref="V6ToSqlOrder(Guid)"/> — converts a SQL-Server-ordered version 6
	/// <paramref name="uuid"/> (as read back via <see cref="Guid.ToByteArray()"/>) back to
	/// RFC 9562 order.
	/// </summary>
	public static unsafe Guid V6FromSqlOrder(Guid uuid)
	{
		Span<byte> bytes = stackalloc byte[16];
		uuid.TryWriteBytes(bytes);
		fixed (byte* p = bytes)
		{
			uuid_v6_to_rfc_order(p);
		}
		return new Guid(bytes, bigEndian: true);
	}

	// ---- Raw-byte SQL-order transforms -------------------------------------------------
	//
	// The four overloads below are the same native permutations as the Guid-taking methods
	// above, but operating directly on a caller's 16-byte buffer. They exist because the Guid
	// round trip is the one genuinely subtle part of this file: the Guid form reads its input
	// with `bigEndian: true` (RFC order) and then constructs its result with plain
	// `new Guid(bytes)` — no byte-order conversion — precisely so that a later
	// Guid.ToByteArray() reproduces the SQL-order bytes. That asymmetry is correct and
	// load-bearing, but it is only explicable in prose.
	//
	// On these overloads there is no asymmetry to explain, because there is no Guid: RFC-ordered
	// bytes go in, SQL-ordered bytes come out, in place. That makes them the form a byte-level
	// correctness oracle can be pointed at directly — notably this project's own
	// SequentialGuidBytes tests in Svartalfheim, which compare raw 16-byte permutations and
	// would otherwise have to model Guid's mixed-endian field layout just to compare results.
	// It also makes them the right call when the value is headed for a wire format or a database
	// parameter that wants bytes anyway, since the Guid detour is pure overhead there.

	/// <summary>
	/// In-place raw-byte form of <see cref="V7ToSqlOrder(Guid)"/> — rewrites the 16 RFC
	/// 9562-ordered version 7 bytes in <paramref name="uuid"/> into SQL Server
	/// <c>uniqueidentifier</c> sort order.
	/// </summary>
	/// <exception cref="ArgumentException"><paramref name="uuid"/> is not exactly 16 bytes.</exception>
	public static unsafe void V7ToSqlOrder(Span<byte> uuid)
	{
		RequireSingleUuid(uuid, nameof(uuid));
		fixed (byte* p = uuid) { uuid_v7_to_sql_order(p); }
	}

	/// <summary>
	/// In-place raw-byte form of <see cref="V7FromSqlOrder(Guid)"/> — rewrites the 16
	/// SQL-Server-ordered version 7 bytes in <paramref name="uuid"/> back into RFC 9562 order.
	/// </summary>
	/// <exception cref="ArgumentException"><paramref name="uuid"/> is not exactly 16 bytes.</exception>
	public static unsafe void V7FromSqlOrder(Span<byte> uuid)
	{
		RequireSingleUuid(uuid, nameof(uuid));
		fixed (byte* p = uuid) { uuid_v7_to_rfc_order(p); }
	}

	/// <summary>
	/// In-place raw-byte form of <see cref="V6ToSqlOrder(Guid)"/> — rewrites the 16 RFC
	/// 9562-ordered version 6 bytes in <paramref name="uuid"/> into SQL Server
	/// <c>uniqueidentifier</c> sort order.
	/// </summary>
	/// <exception cref="ArgumentException"><paramref name="uuid"/> is not exactly 16 bytes.</exception>
	public static unsafe void V6ToSqlOrder(Span<byte> uuid)
	{
		RequireSingleUuid(uuid, nameof(uuid));
		fixed (byte* p = uuid) { uuid_v6_to_sql_order(p); }
	}

	/// <summary>
	/// In-place raw-byte form of <see cref="V6FromSqlOrder(Guid)"/> — rewrites the 16
	/// SQL-Server-ordered version 6 bytes in <paramref name="uuid"/> back into RFC 9562 order.
	/// </summary>
	/// <exception cref="ArgumentException"><paramref name="uuid"/> is not exactly 16 bytes.</exception>
	public static unsafe void V6FromSqlOrder(Span<byte> uuid)
	{
		RequireSingleUuid(uuid, nameof(uuid));
		fixed (byte* p = uuid) { uuid_v6_to_rfc_order(p); }
	}


	/// <summary>
	/// Fills <paramref name="destination"/> with time-sortable version 7 UUIDs sharing one
	/// timestamp capture and one contiguous block of the monotonic counter — one native call
	/// and one random-bytes fetch instead of <paramref name="destination"/>'s length worth of
	/// each. In strictly increasing order however it lands on the counter; see
	/// <see cref="MaxV7Batch"/>.
	/// </summary>
	/// <exception cref="ArgumentOutOfRangeException">
	/// <paramref name="destination"/> holds more than <see cref="MaxV7Batch"/> elements, or
	/// <paramref name="unixMilliseconds"/> is negative or does not fit within 48 bits.
	/// </exception>
	public static void FillV7(Span<Guid> destination, long unixMilliseconds)
	{
		ThrowOnBatchFailure(CoreFillV7(destination, unixMilliseconds), "uuid_new_v7_batch",
			"Unix millisecond timestamp must be non-negative and fit within 48 bits.",
			nameof(destination));
	}

	/// <summary>
	/// Non-throwing counterpart to <see cref="FillV7(Span{Guid}, long)"/> — returns
	/// <see langword="false"/> instead of throwing when the native call reports a random-source
	/// failure or an out-of-range <paramref name="unixMilliseconds"/>, and for a
	/// <paramref name="destination"/> longer than <see cref="MaxV7Batch"/>. On failure
	/// <paramref name="destination"/> is left untouched. See <see cref="TryNewV4"/> for why.
	/// </summary>
	public static bool TryFillV7(Span<Guid> destination, long unixMilliseconds) =>
		CoreFillV7(destination, unixMilliseconds) == 0;

	/// <summary>
	/// Fills <paramref name="destination"/> with raw RFC 9562-ordered version 7 UUID bytes —
	/// 16 per UUID, contiguous, no <see cref="Guid"/> anywhere on the path.
	/// </summary>
	/// <remarks>
	/// This is the allocation-free, conversion-free form of
	/// <see cref="FillV7(Span{Guid}, long)"/>: the native core already writes the batch as one
	/// contiguous block of RFC-ordered bytes, so handing it the caller's own buffer means one
	/// native call and <em>zero</em> managed per-element work. The <see cref="Guid"/> overload has
	/// to marshal through a scratch buffer and run a
	/// <c>new Guid(chunk, bigEndian: true)</c> conversion per element, because
	/// <see cref="Guid"/>'s in-memory field layout is mixed-endian and isn't the RFC byte order.
	/// Prefer this overload when the destination is a wire buffer, a database parameter, or
	/// anything else that wants RFC bytes rather than <see cref="Guid"/> values.
	/// <para>
	/// <paramref name="destination"/>'s length must be an exact multiple of 16; anything else is a
	/// caller error and throws.
	/// </para>
	/// </remarks>
	/// <exception cref="ArgumentException">
	/// <paramref name="destination"/>'s length is not a multiple of 16.
	/// </exception>
	/// <exception cref="ArgumentOutOfRangeException">
	/// <paramref name="destination"/> holds more than <see cref="MaxV7Batch"/> UUIDs, or
	/// <paramref name="unixMilliseconds"/> is negative or does not fit within 48 bits.
	/// </exception>
	public static void FillV7(Span<byte> destination, long unixMilliseconds)
	{
		RequireWholeUuids(destination, nameof(destination));
		ThrowOnBatchFailure(CoreFillV7Bytes(destination, unixMilliseconds), "uuid_new_v7_batch",
			"Unix millisecond timestamp must be non-negative and fit within 48 bits.",
			nameof(destination));
	}

	/// <summary>
	/// Non-throwing counterpart to <see cref="FillV7(Span{byte}, long)"/>. Returns
	/// <see langword="false"/> — rather than throwing — for a native failure <em>and</em> for a
	/// <paramref name="destination"/> whose length isn't a multiple of 16, matching the BCL's own
	/// <c>Try</c> convention (<see cref="Guid.TryWriteBytes(Span{byte})"/> likewise returns
	/// <see langword="false"/> for a badly sized destination rather than throwing).
	/// </summary>
	public static bool TryFillV7(Span<byte> destination, long unixMilliseconds) =>
		destination.Length % 16 == 0 && CoreFillV7Bytes(destination, unixMilliseconds) == 0;

	static unsafe int CoreFillV7(Span<Guid> destination, long unixMilliseconds)
	{
		if (destination.IsEmpty)
			return 0;
		// The core refuses this too (code 4), but only after a scratch buffer of up to 2 GiB
		// had been rented to hand it; answer for it here instead.
		if (destination.Length > MaxV7Batch)
			return 4;

		int totalBytes = destination.Length * 16;
		Span<byte> stackBuf = stackalloc byte[BatchStackThresholdBytes];
		byte[]? rented = null;
		Span<byte> buf = totalBytes <= BatchStackThresholdBytes
			? stackBuf[..totalBytes]
			: (rented = ArrayPool<byte>.Shared.Rent(totalBytes)).AsSpan(0, totalBytes);
		try
		{
			int rc;
			fixed (byte* p = buf)
			{
				rc = uuid_new_v7_batch(unixMilliseconds, (uint)destination.Length, p);
			}
			if (rc != 0)
				return rc;
			for (int i = 0; i < destination.Length; i++)
			{
				destination[i] = new Guid(buf.Slice(i * 16, 16), bigEndian: true);
			}
			return 0;
		}
		finally
		{
			if (rented is not null)
				ArrayPool<byte>.Shared.Return(rented);
		}
	}

	static unsafe int CoreFillV7Bytes(Span<byte> destination, long unixMilliseconds)
	{
		if (destination.IsEmpty)
			return 0;
		fixed (byte* p = destination)
		{
			return uuid_new_v7_batch(unixMilliseconds, (uint)(destination.Length / 16), p);
		}
	}

	/// <summary>
	/// Fills <paramref name="destination"/> with time-sortable version 7 UUIDs using the
	/// current UTC time.
	/// </summary>
	public static void FillV7(Span<Guid> destination) =>
		FillV7(destination, DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());

	/// <summary>
	/// Creates an array of <paramref name="count"/> time-sortable version 7 UUIDs sharing one
	/// timestamp capture, in strictly increasing order (see <see cref="MaxV7Batch"/>).
	/// </summary>
	/// <exception cref="ArgumentOutOfRangeException">
	/// <paramref name="count"/> is negative or greater than <see cref="MaxV7Batch"/>, or
	/// <paramref name="unixMilliseconds"/> is negative or does not fit within 48 bits.
	/// </exception>
	public static Guid[] NewV7Batch(int count, long unixMilliseconds)
	{
		ArgumentOutOfRangeException.ThrowIfNegative(count);
		ArgumentOutOfRangeException.ThrowIfGreaterThan(count, MaxV7Batch);
		var result = new Guid[count];
		FillV7(result, unixMilliseconds);
		return result;
	}

	/// <summary>
	/// Creates an array of <paramref name="count"/> time-sortable version 7 UUIDs using the
	/// current UTC time.
	/// </summary>
	public static Guid[] NewV7Batch(int count) =>
		NewV7Batch(count, DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
}
