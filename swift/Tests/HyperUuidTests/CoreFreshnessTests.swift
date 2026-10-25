#if !HYPERUUID_LOCAL_CORE && (os(macOS) || os(Linux) || os(Windows))
    import Foundation
    import XCTest

    /// The core freshness check. Outside CI this suite links the committed artifact bundle,
    /// which stage-native-binaries.yml refreshes only at release, so after a change to the core
    /// a plain `swift test` would test the core as it was at the last staging. The check finds
    /// the commit that staging was built from and fails when rust/ differs from it. CI writes
    /// a freshly built core into the bundle before the suite runs, which the check sees as a
    /// changed tree, and sets CI besides; `HYPERUUID_LOCAL_CORE=1`, which links the checkout's
    /// own core, compiles this file out. XCTest cannot stop the run before the other tests, so
    /// a stale core is this one named failure beside whatever the rest report.
    final class CoreFreshnessTests: XCTestCase {
        func testTheLinkedCoreWasBuiltFromThisCheckout() {
            if let message = Self.staleCoreMessage() {
                XCTFail(message)
            }
        }

        /// Why the committed core is older than this checkout's rust/, or `nil` when it is
        /// not, or when the question cannot be answered here.
        private static func staleCoreMessage() -> String? {
            let tree = "swift/HyperUuidCore.artifactbundle"
            if let ci = ProcessInfo.processInfo.environment["CI"], !ci.isEmpty {
                return nil
            }
            // git runs in the repository's top level once that is known (this file's
            // directory until then), and reports its exit code, or -1 when it could not run.
            var top = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            func git(_ args: String...) -> (String, Int32) {
                guard let executable = gitExecutable() else { return ("", -1) }
                let process = Process()
                process.executableURL = executable
                process.arguments = args
                process.currentDirectoryURL = top
                let output = Pipe()
                process.standardOutput = output
                process.standardError = FileHandle.nullDevice
                do {
                    try process.run()
                } catch {
                    return ("", -1)
                }
                let data = output.fileHandleForReading.readDataToEndOfFile()
                process.waitUntilExit()
                let text = String(decoding: data, as: UTF8.self)
                return (text.trimmingCharacters(in: .whitespacesAndNewlines), process.terminationStatus)
            }
            let (dir, found) = git("rev-parse", "--show-toplevel")
            guard found == 0, !dir.isEmpty else { return nil }
            top = URL(fileURLWithPath: dir)
            let (staging, _) = git("log", "-1", "--format=%H", "--grep=^chore: stage native binaries", "HEAD")
            guard !staging.isEmpty else { return nil }
            var (source, _) = git("log", "-1", "--format=%(trailers:key=Release-Source-SHA,valueonly)", staging)
            if source.isEmpty {
                let (subject, _) = git("log", "-1", "--format=%s", staging)
                if let match = subject.firstMatch(of: /\(([0-9a-f]{40})\)$/) {
                    source = String(match.1)
                }
            }
            guard source.count >= 12 else { return nil }
            guard git("cat-file", "-e", "\(source)^{commit}").1 == 0 else { return nil }
            // Fresh binaries written into the tree count; its READMEs do not.
            let readmes = ":(exclude,glob)\(tree)/**/README.md"
            guard git("diff", "--quiet", "HEAD", "--", tree, readmes).1 == 0 else { return nil }
            let (_, differs) = git("diff", "--quiet", source, "--", "rust/src", "rust/Cargo.toml", "rust/Cargo.lock")
            let (untracked, _) = git("ls-files", "--others", "--exclude-standard", "--", "rust/src")
            guard differs == 1 || !untracked.isEmpty else { return nil }
            return "\(tree) was built from rust/ at \(source.prefix(12)), and this checkout's rust/ differs from it, "
                + "so these tests would run against a stale core. Build the checkout's own with "
                + ".github/scripts/local-core.sh (FORGE=<path> if the SkunkWerkx/.github checkout is not "
                + "../.github or ../HyperForge), then run HYPERUUID_LOCAL_CORE=1 swift test instead; "
                + "CI builds its own core and skips this check."
        }

        /// git as PATH finds it, or `nil` when it is not there.
        private static func gitExecutable() -> URL? {
            #if os(Windows)
                let (separator, name): (Character, String) = (";", "git.exe")
            #else
                let (separator, name): (Character, String) = (":", "git")
            #endif
            // Windows spells it Path.
            let path = ProcessInfo.processInfo.environment.first { $0.key.uppercased() == "PATH" }?.value ?? ""
            for entry in path.split(separator: separator) where !entry.isEmpty {
                let candidate = URL(fileURLWithPath: String(entry)).appendingPathComponent(name)
                if FileManager.default.isExecutableFile(atPath: candidate.path) {
                    return candidate
                }
            }
            return nil
        }
    }
#endif
