import Benchmark
import Foundation
import HyperUuid

let benchmarks: @Sendable () -> Void = {
    Benchmark.defaultConfiguration = .init(
        metrics: [.wallClock, .throughput, .mallocCountTotal],
        maxDuration: .seconds(2)
    )

    Benchmark("Foundation.UUID()") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(UUID())
        }
    }

    Benchmark("HyperUuid.newV4") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try! UuidGenerator.newV4())
        }
    }

    Benchmark("HyperUuid.newV5") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try! UuidGenerator.newV5(namespace: Namespaces.dns, name: "example.com"))
        }
    }

    Benchmark("HyperUuid.newV6") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try! UuidGenerator.newV6())
        }
    }

    Benchmark("HyperUuid.newV7") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try! UuidGenerator.newV7())
        }
    }

    Benchmark("HyperUuid.newV6 x1000 (individual)") { benchmark in
        for _ in benchmark.scaledIterations {
            for _ in 0..<1000 {
                blackHole(try! UuidGenerator.newV6())
            }
        }
    }

    Benchmark("HyperUuid.newV6Batch(count: 1000)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try! UuidGenerator.newV6Batch(count: 1000))
        }
    }

    Benchmark("HyperUuid.newV7 x1000 (individual)") { benchmark in
        for _ in benchmark.scaledIterations {
            for _ in 0..<1000 {
                blackHole(try! UuidGenerator.newV7())
            }
        }
    }

    Benchmark("HyperUuid.newV7Batch(count: 1000)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try! UuidGenerator.newV7Batch(count: 1000))
        }
    }

    // Destination-buffer forms. The storage is allocated once, outside the measured loop,
    // because reusing a buffer the caller already owns is the whole point of these APIs —
    // allocating per iteration would measure exactly the cost they exist to avoid.
    Benchmark("HyperUuid.fillV7(into: [UUID]) 1000") { benchmark in
        var dst = [UUID](repeating: UUID(), count: 1000)
        for _ in benchmark.scaledIterations {
            try! UuidGenerator.fillV7(into: &dst, unixMillis: 1_645_557_742_000)
            blackHole(dst)
        }
    }

    Benchmark("HyperUuid.fillV7(into: raw bytes) 1000") { benchmark in
        var raw = [UInt8](repeating: 0, count: 1000 * 16)
        for _ in benchmark.scaledIterations {
            raw.withUnsafeMutableBytes { buf in
                try! UuidGenerator.fillV7(into: buf, unixMillis: 1_645_557_742_000)
            }
            blackHole(raw)
        }
    }

    Benchmark("HyperUuid.fillV6(into: [UUID]) 1000") { benchmark in
        var dst = [UUID](repeating: UUID(), count: 1000)
        for _ in benchmark.scaledIterations {
            try! UuidGenerator.fillV6(into: &dst, unixMillis: 1_645_557_742_000)
            blackHole(dst)
        }
    }
}
