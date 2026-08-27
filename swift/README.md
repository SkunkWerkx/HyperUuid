# HyperUuid

RFC 9562 UUID v4 (random), v5 (deterministic), and v7 (time-sortable) generation,
calling directly into the native `libhyperuuid` shared library via `dlopen`/`dlsym`
(Linux/macOS) or `LoadLibraryW`/`GetProcAddress` (Windows) plus an `@convention(c)`
function-pointer cast — no runtime bridge, no shim. Bundles a native build for every
supported platform (linux/macOS/Windows × x64/arm64) since SwiftPM's native binary-
distribution mechanism (a `binaryTarget`/XCFramework) is Apple-only; picks the right
one at compile time.

```swift
import HyperUuid

let id = try UuidGenerator.newV4()
let id2 = try UuidGenerator.newV5(namespace: Namespaces.dns, name: "example.com")
let id3 = try UuidGenerator.newV7()
```

Returns Foundation's `UUID`. `Namespaces.dns`/`url`/`oid`/`x500` are RFC 9562
Section 6.6's well-known namespaces.

Not yet published to the Swift Package Registry under a registered `SkunkWerkx`
presence — for now this is proven by CI building and testing the native core plus
this package on real hardware for every platform leg. Consume via a direct
`.package(url: "https://github.com/SkunkWerkx/HyperUuid", branch: "...")` (scoped to
this `swift/` subdirectory) in the meantime.
