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
}
