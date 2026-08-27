# hyperuuid

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, calling
directly into the native `libhyperuuid` shared library via
[`Fiddle`](https://docs.ruby-lang.org/en/master/Fiddle.html) — dlopen/dlsym plus a raw C-ABI
call, no runtime bridge. Bundles a native build for every supported platform (linux/darwin/
windows × x64/arm64) and picks the right one at runtime, the same trick the Go/Java
bindings use since RubyGems has no per-platform native selection wired up here yet.

```ruby
require "hyperuuid"

id = HyperUuid.new_v4
id2 = HyperUuid.new_v5(HyperUuid::Namespaces::DNS, "example.com")
id3 = HyperUuid.new_v6
id4 = HyperUuid.new_v7
```

Returns `HyperUuid::Uuid`, a minimal value object (`#bytes`, `#to_s`, `#version`, `#variant`,
comparable/hashable) — this gem has no runtime dependency on the `uuid` gem.
`HyperUuid::Namespaces::DNS`/`URL`/`OID`/`X500` are RFC 9562 Section 6.6's well-known
namespaces. `#timestamp` recovers the embedded UTC `Time` from a version 6 or 7 UUID.
`HyperUuid::Uuid::NIL`/`MAX` are the RFC 9562 §5.9/§5.10 special-value UUIDs.
`HyperUuid.new_v6_batch(count)`/`new_v7_batch(count)` generate `count` UUIDs sharing one
timestamp capture and one native call, instead of `count` of each.

Not yet published to RubyGems.org under a registered `SkunkWerkx`/`buvinghausen` presence —
for now this is proven by CI building and testing the native core plus this gem on real
hardware for every platform leg. Consume via a direct
`gem "hyperuuid", git: "https://github.com/SkunkWerkx/HyperUuid", glob: "ruby/*.gemspec"` in
the meantime.
