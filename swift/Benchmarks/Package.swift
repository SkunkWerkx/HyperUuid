// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "HyperUuidBenchmarksPackage",
    platforms: [.macOS(.v13)],
    dependencies: [
        .package(path: "../"),
        .package(url: "https://github.com/ordo-one/benchmark", from: "1.4.0"),
    ],
    targets: [
        .executableTarget(
            name: "HyperUuidBenchmarks",
            dependencies: [
                .product(name: "HyperUuid", package: "swift"),
                .product(name: "Benchmark", package: "benchmark"),
            ],
            path: "Benchmarks/HyperUuidBenchmarks",
            plugins: [
                .plugin(name: "BenchmarkPlugin", package: "benchmark")
            ]
        )
    ]
)
