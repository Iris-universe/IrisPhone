// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "__APP_NAME__",
    platforms: [
        .iOS(.v17),
        .macOS(.v14),
    ],
    products: [
        // An xtool project has exactly one library product: the app itself.
        .library(
            name: "__APP_NAME__",
            targets: ["__APP_NAME__"]
        ),
    ],
    targets: [
        .target(
            name: "__APP_NAME__"
            // Bundled files (images, JSON...): put them in Sources/__APP_NAME__/Resources and add
            // `resources: [.process("Resources")]`, then load them with Bundle.module.
        ),
    ]
)
