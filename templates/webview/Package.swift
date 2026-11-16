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
            name: "__APP_NAME__",
            // The web app (HTML/CSS/JS) is shipped as-is inside the app.
            resources: [.copy("Web")],
            swiftSettings: [.swiftLanguageMode(.v5)]
        ),
    ]
)
