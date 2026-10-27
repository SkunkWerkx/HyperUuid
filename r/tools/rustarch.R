# The Rust target for a Windows build (src/Makevars.win): rustup's gnu toolchain, since
# Rtools ships no Rust; gnullvm on ARM64.
arm <- grepl("aarch64", R.version$platform)
cat(if (arm) "aarch64-pc-windows-gnullvm" else "x86_64-pc-windows-gnu")
