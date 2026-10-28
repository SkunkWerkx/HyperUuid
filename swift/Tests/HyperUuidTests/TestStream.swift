import Foundation

@testable import HyperUuid

// The v7 stream is process-global. Prime it at the present once, before any test: a fixed
// old timestamp (the RFC vector) then mints out of stream and carries its own millisecond
// exactly, instead of seeding the stream in 2022 and leaving which tests are ordered up to
// their order of execution. The rule for every test in this target: no test supplies more than
// now + 500 ms, and current-time bounds allow + 1000 ms.
enum TestStream {
    /// Touch this from `setUp()`; the closure runs once per process.
    static let primed: Void = {
        do {
            _ = try UuidGenerator.newV7(unixMillis: now())
        } catch {
            fatalError("priming the v7 stream failed: \(error)")
        }
    }()

    /// The present, in Unix milliseconds.
    static func now() -> UInt64 {
        UInt64(Date().timeIntervalSince1970 * 1000)
    }
}
