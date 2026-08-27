module HyperUuid
  # Maps the running RUBY_PLATFORM to the RID-style directory (matching the other bindings'
  # runtimes/{rid}/native/ / native/{rid}/ convention) and native library filename to load.
  module NativePlatform
    class UnsupportedPlatformError < StandardError; end

    def self.rid_and_library_name
      is_arm = RUBY_PLATFORM.match?(/arm64|aarch64/)

      case RUBY_PLATFORM
      when /mingw|mswin|windows/
        is_arm ? ["win-arm64", "hyperuuid.dll"] : ["win-x64", "hyperuuid.dll"]
      when /darwin/
        is_arm ? ["osx-arm64", "libhyperuuid.dylib"] : ["osx-x64", "libhyperuuid.dylib"]
      when /linux/
        is_arm ? ["linux-arm64", "libhyperuuid.so"] : ["linux-x64", "libhyperuuid.so"]
      else
        raise UnsupportedPlatformError, "hyperuuid: unsupported platform RUBY_PLATFORM=#{RUBY_PLATFORM}"
      end
    end
  end
end
