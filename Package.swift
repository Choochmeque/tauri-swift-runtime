// swift-tools-version:5.7
// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

import PackageDescription

let package = Package(
  name: "TauriSwiftRuntime",
  platforms: [
    .macOS(.v13),
    .iOS(.v15),
  ],
  products: [
    // Products define the executables and libraries a package produces, and make them visible to other packages.
    .library(
      name: "TauriSwiftRuntime",
      type: .static,
      targets: ["TauriSwiftRuntime"])
  ],
  dependencies: [
    // Dependencies declare other packages that this package depends on.
    .package(url: "https://github.com/Brendonovich/swift-rs", from: "1.0.0")
  ],
  targets: [
    // Targets are the basic building blocks of a package. A target can define a module or a test suite.
    // Targets can depend on other targets in this package, and on products in packages this package depends on.
    .target(
      name: "TauriSwiftRuntime",
      dependencies: [
        .product(name: "SwiftRs", package: "swift-rs")
      ],
      path: "src-swift/Sources"
    ),
    // .testTarget(
    //   name: "TauriTests",
    //   dependencies: ["TauriSwiftRuntime"]
    // ),
  ]
)
