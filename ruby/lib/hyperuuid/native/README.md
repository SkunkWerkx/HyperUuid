# native/

Populated per-RID with the platform's native `libhyperuuid` build (`native/{rid}/{lib}`) by CI
and by `release.yml` when it packs the universal gem — see `../../../.gitignore`. The RIDs are
the ones `../native_platform.rb` maps `RUBY_PLATFORM` to: `linux-x64`, `linux-arm64`,
`linux-musl-x64`, `linux-musl-arm64`, `osx-x64`, `osx-arm64`, `win-x64`, `win-arm64`.

These are the Fiddle backend's libraries, and only the universal `ruby`-platform gem ships
them — all eight. The precompiled platform gems carry none: `rake native:gem` leaves this
directory out, since each of them has a Magnus extension for every Ruby it installs on.

Nothing has to be staged here for local development: when a file is absent, the Fiddle
backend falls back to `rust/target/release/` (see `../runtime.rb`).

This file exists so the directory has at least one tracked file on a fresh checkout. It is
not part of any gem: the gemspec packages `native/*/*`, the staged binaries only.
