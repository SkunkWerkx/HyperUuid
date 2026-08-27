# HyperUuid

High-performance, allocation-free RFC 9562 UUID generation. One Rust core, direct native FFI into C#, Java, Go, Swift, Ruby, PHP, and Python — no runtime bridge, no reflection. Runs on the server and all the way out to the browser.

A single `cdylib` (`rust/`) exports a plain C ABI (`uuid_new_v4`/`v5`/`v6`/`v7`); every language binds straight to it — P/Invoke, FFM, `purego`/`dlopen`, `Fiddle`, PHP's `FFI`, or `ctypes` — rather than going through a serialization layer or embedded runtime. Covers every standard version RFC 9562 itself still recommends generating (v1 and v3 are superseded by v6/v5 respectively, so they're skipped), plus the Nil and Max special-value UUIDs.

## State of the union

Every language, on every platform, proven for real: `.github/workflows/build-packages.yml`'s `build-native` matrix builds the Rust core fresh on each of 6 real-hardware legs, then runs that language's actual test suite against that leg's freshly-built native library — not just that it compiles.

| Language | linux-x64 | linux-arm64 | osx-x64 | osx-arm64 | win-x64 | win-arm64 |
| --- | :---: | :---: | :---: | :---: | :---: | :---: |
| [Rust](rust/) (core) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| [C#](csharp/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| [Java](java/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| [Go](go/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| [Swift](swift/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| [Ruby](ruby/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| [PHP](php/) | ✅ | ✅ | ✅ | ✅ | ✅ | — |
| [Python](python/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |

PHP skips win-arm64 deliberately: PHP has never shipped a native Windows ARM64 build, so it always runs under x64 emulation there regardless of host CPU — already exercised for real by the win-x64 leg.

**Published:** C# ([NuGet](https://github.com/SkunkWerkx/HyperUuid/packages), via GitHub Packages) and Java ([Maven](https://github.com/SkunkWerkx/HyperUuid/packages), via GitHub Packages). The JVM binding is plain Java, not Kotlin — `kotlin-stdlib` would otherwise be a real transitive dependency for every consumer, unlike every other binding here — and its AOT story is proven the same way C#'s is: a local GraalVM Native Image smoke test (`java/aot-smoke-test/`, `./gradlew :aot-smoke-test:nativeRun`) that produces a genuine standalone native binary, no JVM required to run it.

**Proven, not yet published:** Go, Swift, Ruby, PHP, and Python are all CI-green on every platform above but don't have a registered `SkunkWerkx`/`buvinghausen` presence on their respective registries yet (pkg.go.dev, Swift Package Registry, RubyGems, Packagist, PyPI) — see each language's own README for how to consume it directly in the meantime.
