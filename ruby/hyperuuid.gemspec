Gem::Specification.new do |spec|
  spec.name = "hyperuuid"
  # The real, committed version — same story as every other binding's manual bump: 0.0.1
  # proves the real RubyGems Trusted Publishing path for real, ahead of the coordinated
  # v0.1.0 release.
  spec.version = "0.0.1"
  spec.summary = "RFC 9562 UUID v4/v5/v6/v7 generation — direct native FFI into a Rust core, no runtime bridge."
  spec.description = <<~DESC
    High-performance, allocation-free RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7
    (time-sortable) generation, calling directly into the native libhyperuuid shared library
    via Fiddle (Ruby's standard-library FFI) — no runtime bridge, no extra gem dependency.
  DESC
  spec.authors = ["Brian Buvinghausen"]
  spec.license = "MIT"
  spec.homepage = "https://github.com/SkunkWerkx/HyperUuid"
  spec.required_ruby_version = ">= 3.2"

  spec.files = Dir["lib/**/*.rb"] + Dir["lib/**/native/**/*"] + ["README.md"]
  spec.require_paths = ["lib"]

  # fiddle was a Ruby default gem (effectively stdlib, no declaration needed) through Ruby
  # 3.x; Ruby 4.0 unbundled it into a regular gem, so it now needs an explicit dependency —
  # confirmed by hitting exactly the resulting LoadError under `bundle exec` on Ruby 4.0.6
  # before adding this line. Still zero *third-party* runtime dependencies: fiddle ships
  # with every Ruby install (rbenv/RubyGems installs it alongside the interpreter), just no
  # longer implicitly on the load path.
  spec.add_dependency "fiddle"
  spec.add_development_dependency "benchmark-ips", "~> 2.15"
  spec.add_development_dependency "rake", "~> 13.0"
  spec.add_development_dependency "yard", "~> 0.9"

  spec.metadata["source_code_uri"] = spec.homepage
end
