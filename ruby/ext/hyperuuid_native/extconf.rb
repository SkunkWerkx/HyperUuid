# The hyperuuid-wasm gem's extension: the Magnus extension, linked statically into a ruby.wasm
# interpreter by `rbwasm build`. ruby.wasm cannot load an extension at runtime, so a consumer
# puts this gem in the Gemfile they hand to rbwasm and gets one interpreter with HyperUuid
# compiled in. Only this companion gem ships this file; the hyperuuid gems do not, because an
# `extensions` entry there would run extconf on every host `gem install`.
#
# Bundler installs the gem on the host first, where there is nothing to build. Then rbwasm runs
# this again with the cross-compiled Ruby's RbConfig, `make clean`, `make static`, and links
# every *.a left in the build directory. Both paths below end in the same place: the archive
# saved as hyperuuid_native.prebuilt (not *.a, which `make clean` removes), copied into place
# by `make static`.
#
#  * Prebuilt, what a consumer gets: the gem carries one gzipped archive per Ruby minor (Rakefile,
#    wasm:gem), built and attested in CI. No Rust toolchain on the consumer's machine.
#  * From source, how CI makes those archives: HYPERUUID_RUST_DIR names the crate, and
#    HYPERUUID_WASM_OUT, if set, receives a copy of the finished archive.
require "mkmf"
require "rbconfig"
require "zlib"
require "fileutils"

unless RbConfig::CONFIG["host_os"].include?("wasi")
  File.write("Makefile", dummy_makefile($srcdir).join)
  exit
end

# rbwasm reads $target back out to name the extension's Init function in its extinit table.
$target = "hyperuuid_native"
minor = "#{RbConfig::CONFIG["MAJOR"]}.#{RbConfig::CONFIG["MINOR"]}"
prebuilt = "hyperuuid_native.prebuilt"

if (rust_dir = ENV["HYPERUUID_RUST_DIR"])
  # rb-sys binds against the target Ruby through RBCONFIG_* variables. Ruby is not installed
  # yet when rbwasm builds extensions, so the two header directories point at the source and
  # build trees instead (rbwasm sets top_srcdir and extout for exactly this).
  RbConfig::CONFIG.merge(
    "rubyhdrdir" => File.join(ENV.fetch("top_srcdir"), "include"),
    "rubyarchhdrdir" => File.join(ENV.fetch("extout"), "include", RbConfig::CONFIG["arch"])
  ).each { |key, value| ENV["RBCONFIG_#{key}"] = value.to_s }
  wasi_sdk = File.dirname(File.dirname(RbConfig::CONFIG["CC"].split.first))
  ENV["BINDGEN_EXTRA_CLANG_ARGS"] = "--sysroot=#{wasi_sdk}/share/wasi-sysroot " \
                                    "-D_WASI_EMULATED_SIGNAL -D_WASI_EMULATED_PROCESS_CLOCKS -D_WASI_EMULATED_MMAN"
  ENV["CC_wasm32_wasip1"] = "#{wasi_sdk}/bin/clang"
  target_dir = File.expand_path("target")
  system("cargo", "rustc", "--manifest-path", File.join(rust_dir, "Cargo.toml"), "--release",
         "--target", "wasm32-wasip1", "--crate-type", "staticlib", "--features", "ruby",
         "--target-dir", target_dir, exception: true)
  archive = File.binread(File.join(target_dir, "wasm32-wasip1/release/libhyperuuid.a"))
  # The crate links into one object (release LTO), and besides Init_hyperuuid_native and the
  # C ABI it exports three symbols that every Rust extension carrying std defines too:
  # rust_eh_personality, which wasi-vfs (linked into every ruby.wasm) also defines;
  # ruby_abi_version, rb-sys's ABI stamp for a dynamically loaded extension, which a static
  # one never needs; and std's EMPTY_PANIC. Two definitions of any of them stop the link, so
  # each is renamed to one this crate owns, and the HyperCast extension, renamed the same way,
  # can share the interpreter. wasm llvm-objcopy can neither localize nor rename a symbol, so
  # the names are rewritten in place at the same length, which leaves every offset in the
  # archive and its objects valid (and keeps EMPTY_PANIC's v0 mangling well-formed). Nothing
  # calls the first two: a static extension's ABI stamp is never looked up, and wasm32-wasip1
  # builds with panic=abort.
  {
    "rust_eh_personality" => "hyperuuid_eh_person",
    "ruby_abi_version" => "hyperuuid_abiver",
    "9panicking11EMPTY_PANIC" => "9panicking11HUUID_PANIC"
  }.each { |from, to| archive.gsub!(from, to) }
  File.binwrite(prebuilt, archive)
  system("#{wasi_sdk}/bin/llvm-ranlib", prebuilt, exception: true)
  # And a newer Rust that exports another of std's symbols fails here, by name, rather than in
  # the link of an interpreter that also carries the HyperCast extension.
  member = nil
  exported = IO.popen(["#{wasi_sdk}/bin/llvm-nm", "--defined-only", "--extern-only", prebuilt], err: File::NULL, &:read)
               .each_line(chomp: true).filter_map do |line|
    if line.end_with?(".o:")
      member = line
      next
    end
    line.split.last if member&.start_with?("hyperuuid-") && line.match?(/\A\h+ [A-Z] /)
  end
  shared = exported.grep(/\A(_R|_ZN|rust_|__rust|ruby_abi_version\z)/).grep_v(/HUUID_PANIC\z/)
  shared.empty? or abort "hyperuuid-wasm: the extension exports #{shared.join(", ")}, which " \
                         "another Rust extension in the same interpreter would define too"
  if (out = ENV["HYPERUUID_WASM_OUT"])
    FileUtils.mkdir_p(File.join(out, minor))
    FileUtils.cp(prebuilt, File.join(out, minor, "hyperuuid_native.a"))
  end
else
  gz = File.join(__dir__, minor, "hyperuuid_native.a.gz")
  unless File.exist?(gz)
    shipped = Dir[File.join(__dir__, "*", "hyperuuid_native.a.gz")].map { |f| File.basename(File.dirname(f)) }
    abort "hyperuuid-wasm: no prebuilt extension for Ruby #{minor} (this gem carries #{shipped.sort.join(", ")})"
  end
  File.binwrite(prebuilt, Zlib::GzipReader.open(gz, &:read))
end

# install-so is rbwasm's dynamic-linking path (a pic target, for the component model), which
# a static archive cannot serve.
File.write("Makefile", <<~MAKE)
  all: static
  static:
  \tcp #{prebuilt} hyperuuid_native.a
  install-so:
  \t@echo "hyperuuid-wasm links statically: build a wasm32-unknown-wasip1 interpreter" >&2; exit 1
  install-rb:
  \t@true
  clean:
  \trm -f hyperuuid_native.a
  .PHONY: all static install-so install-rb clean
MAKE
