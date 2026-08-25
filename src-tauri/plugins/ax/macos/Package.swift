// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "AxPlugin",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "AxPlugin", type: .static, targets: ["AxPlugin"])
    ],
    dependencies: [
        .package(url: "https://github.com/Brendonovich/swift-rs", from: "1.0.7")
    ],
    targets: [
        .target(
            name: "AxPlugin",
            dependencies: [
                .product(name: "SwiftRs", package: "swift-rs")
            ],
            path: "Sources"
        )
    ]
)
