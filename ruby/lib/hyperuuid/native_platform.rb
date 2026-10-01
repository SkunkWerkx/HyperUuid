module HyperUuid
  # Maps the running RUBY_PLATFORM to the RID-style directory (matching the other bindings'
  # runtimes/{rid}/native/ / native/{rid}/ convention) and native library filename to load.
  module NativePlatform
    class UnsupportedPlatformError < StandardError; end

    # +platform+ is a parameter only so the specs can walk the whole table from one host;
    # every real caller takes the default.
    def self.rid_and_library_name(platform = RUBY_PLATFORM)
      arch = platform.match?(/arm64|aarch64/) ? "arm64" : "x64"

      case platform
      when /mingw|mswin|windows/
        ["win-#{arch}", "hyperuuid.dll"]
      when /darwin/
        ["osx-#{arch}", "libhyperuuid.dylib"]
      when /linux/
        # Two C libraries, two builds: a glibc-linked library cannot be relied on to dlopen
        # into a musl process (Alpine), so musl gets RIDs of its own. Ruby names the libc
        # in its platform string there ("x86_64-linux-musl") and leaves it off on glibc.
        os = platform.include?("musl") ? "linux-musl" : "linux"
        ["#{os}-#{arch}", "libhyperuuid.so"]
      else
        raise UnsupportedPlatformError, "hyperuuid: unsupported platform RUBY_PLATFORM=#{platform}"
      end
    end
  end
end
