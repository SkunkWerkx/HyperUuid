Gem::Specification.new do |spec|
  spec.name = "hyperuuid"
  # Kept in lockstep with HyperUuid::VERSION (lib/hyperuuid.rb) and rust/Cargo.toml by the
  # prepare-release workflow, which rewrites all three together.
  spec.version = "0.6.0"
  spec.summary = "RFC 9562 UUID v4/v5/v6/v7 generation over a native Rust core, shipped prebuilt"
  spec.description = <<~DESC
    RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, with
    batch and raw-bytes forms, from one native Rust core. Two backends behind one surface,
    selected automatically and both shipped prebuilt: a Magnus extension where a precompiled
    platform gem matches, and stdlib Fiddle everywhere else. No runtime bridge, no
    dependencies beyond Fiddle.
  DESC
  spec.authors = ["Brian Buvinghausen"]
  spec.license = "MIT"
  spec.homepage = "https://github.com/SkunkWerkx/HyperUuid"
  # 3.3 floor: the oldest Ruby still supported upstream (3.2 reached end of life on
  # 2026-03-31). The precompiled platform gems narrow this further, to the ABIs they carry —
  # see the Rakefile's native:gem task.
  spec.required_ruby_version = ">= 3.3"

  # LICENSE is a local copy of the repo root's, not a reference to it: RubyGems stores a
  # "../LICENSE" entry with the `..` intact (a path-traversal entry no installer accepts),
  # and a symlink is stored *as* a symlink — `gem build` warns, and it dangles once the gem
  # is unpacked somewhere else entirely. Same reason rust/ and python/ carry their own.
  #
  # native/*/* is exactly the staged binaries, one directory per RID — not native/README.md, the placeholder that only keeps the directory in a fresh checkout.
  spec.files = Dir["lib/**/*.rb"] + Dir["lib/hyperuuid/native/*/*"] + ["README.md", "LICENSE"]
  spec.require_paths = ["lib"]

  # fiddle was a Ruby default gem (effectively stdlib, no declaration needed) through Ruby
  # 3.x; Ruby 4.0 unbundled it into a regular gem, so it now needs an explicit dependency —
  # confirmed by hitting exactly the resulting LoadError under `bundle exec` on Ruby 4.0.6
  # before adding this line. Still zero *third-party* runtime dependencies: fiddle ships
  # with every Ruby install (rbenv/RubyGems installs it alongside the interpreter), just no
  # longer implicitly on the load path.
  spec.add_dependency "fiddle"
  # The test and benchmark gems live in the Gemfile.
  spec.add_development_dependency "rake", "~> 13.0"
  spec.add_development_dependency "yard", "~> 0.9"

  spec.metadata["source_code_uri"] = spec.homepage
  spec.metadata["changelog_uri"] = "#{spec.homepage}/blob/master/CHANGELOG.md"
  spec.metadata["bug_tracker_uri"] = "#{spec.homepage}/issues"
  spec.metadata["documentation_uri"] = "#{spec.homepage}/tree/master/ruby#readme"
  # Pushing or yanking a version takes an account with multi-factor authentication on.
  spec.metadata["rubygems_mfa_required"] = "true"
end
