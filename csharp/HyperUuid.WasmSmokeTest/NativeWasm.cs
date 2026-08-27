using System.Runtime.InteropServices;

namespace HyperUuid.WasmSmokeTest;

// SPIKE-only: HyperUuid.csproj's UuidGenerator uses [LibraryImport("hyperuuid")], which tries
// to dlopen a separate named library — correct for every real native platform, but wrong for
// a statically-linked WASM native: there is no separate "hyperuuid" module to open, its
// functions are already part of the SAME dotnet.native.wasm module this app itself runs in.
// "*" tells the runtime to resolve the symbol against the current module instead of dlopen'ing
// anything. Confirmed by hitting System.DllNotFoundException: hyperuuid with the shared
// attribute first, then fixing it here — not guessed.
internal static partial class NativeWasm
{
    [LibraryImport("*")]
    internal static unsafe partial int uuid_new_v4(byte* outPtr);

    [LibraryImport("*")]
    internal static unsafe partial int uuid_new_v5(byte* nsPtr, byte* namePtr, uint nameLen, byte* outPtr);

    [LibraryImport("*")]
    internal static unsafe partial int uuid_new_v7(long unixMillis, byte* outPtr);

    [LibraryImport("*")]
    internal static unsafe partial ulong uuid_v7_unix_millis(byte* uuidPtr);
}
