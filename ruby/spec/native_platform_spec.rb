require "spec_helper"
require "hyperuuid"

# The RUBY_PLATFORM -> native/{rid}/{lib} table the Fiddle backend loads through. Only one
# row of it is the host's, so the rest are walked by passing the platform string in — the
# strings below are what each platform's Ruby actually reports.
RSpec.describe HyperUuid::NativePlatform do
  {
    "x86_64-linux" => ["linux-x64", "libhyperuuid.so"],
    "aarch64-linux" => ["linux-arm64", "libhyperuuid.so"],
    "x86_64-linux-musl" => ["linux-musl-x64", "libhyperuuid.so"],
    "aarch64-linux-musl" => ["linux-musl-arm64", "libhyperuuid.so"],
    "x86_64-darwin24" => ["osx-x64", "libhyperuuid.dylib"],
    "arm64-darwin24" => ["osx-arm64", "libhyperuuid.dylib"],
    "x64-mingw-ucrt" => ["win-x64", "hyperuuid.dll"],
    "aarch64-mingw-ucrt" => ["win-arm64", "hyperuuid.dll"]
  }.each do |platform, expected|
    it "maps #{platform} to #{expected.join('/')}" do
      expect(described_class.rid_and_library_name(platform)).to eq(expected)
    end
  end

  it "keeps musl apart from glibc — one cannot be relied on to load the other's library" do
    expect(described_class.rid_and_library_name("x86_64-linux-musl").first)
      .not_to eq(described_class.rid_and_library_name("x86_64-linux").first)
  end

  it "resolves the running platform by default" do
    expect(described_class.rid_and_library_name).to eq(described_class.rid_and_library_name(RUBY_PLATFORM))
  end

  it "raises its own error, naming the platform, for one with no native build" do
    expect { described_class.rid_and_library_name("x86_64-freebsd14") }
      .to raise_error(HyperUuid::NativePlatform::UnsupportedPlatformError, /RUBY_PLATFORM=x86_64-freebsd14/)
  end
end
