# __DISPLAY_NAME__

iOS app built **without a Mac and without Xcode**, with [IrisPhone](https://github.com/Iris-universe/IrisPhone):
Swift + the iOS SDK run on Linux (WSL on Windows), [xtool](https://github.com/xtool-org/xtool) builds,
signs with the developer's Apple ID and installs over USB.

## Commands

Prefer the `irisphone` MCP tools when they are available (`.mcp.json`), otherwise the CLI:

| Goal | MCP tool | CLI |
|------|----------|-----|
| Is everything installed / iPhone plugged in? | `irisphone_doctor` | `irisphone doctor` |
| Compile and get the errors | `irisphone_build` | `irisphone build` |
| Build + install + launch on the iPhone | `irisphone_run` | `irisphone run` |
| Read the app's logs (`print`, `os.Logger`, crashes) | `irisphone_logs` | `irisphone logs` |

**After every code change, build** and fix every error before saying you are done.
There is no simulator: the only runtime test is the real iPhone (`irisphone_run`, then `irisphone_logs`).
If `irisphone_run` reports no device or no Apple ID, tell the user what to do (plug the iPhone in and unlock it,
or run `irisphone login` in their own terminal: it asks for a password and a 2FA code).

## Project layout

- `Package.swift`: a SwiftPM package with exactly **one library product = the app**. No `.xcodeproj`.
- `xtool.yml`: `bundleID` (unique at Apple), `infoPath`, optional `iconPath` (1024x1024 PNG).
- `Info.plist`: extra keys merged into the generated plist (display name, permissions).
- `Sources/__APP_NAME__/`: the code. Entry point = the `@main` `App` struct.

## Rules (no Xcode means no Xcode-only features)

- UI in **SwiftUI** (UIKit through `UIViewRepresentable` is fine). No storyboards, `.xib`, or `.xcassets` asset
  catalogs (there is no `actool` on Linux: the build fails). Colors in code, images as plain PNG/JPEG resources.
- Resources: plain files in a folder declared in `Package.swift` (`resources: [.process("Resources")]` or
  `.copy(...)`), loaded with `Bundle.module`. App icon: `iconPath` in `xtool.yml`.
- **No SwiftData**: its `@Model` macro has no Linux plugin and fails to build. Persist with `Codable` + JSON
  files in the Documents directory, or `UserDefaults` for small settings. `@Observable`, `@State`, `@Environment`
  work. `#Preview` compiles but there is no preview canvas: don't spend effort on previews.
- Any use of the camera, microphone, location, photos, contacts, Bluetooth, local network...
  requires its `NS...UsageDescription` key in `Info.plist`, or iOS kills the app on first use.
- Free Apple ID: the app expires after **7 days** (re-run `irisphone run`), max 3 sideloaded apps,
  10 new bundle IDs per week, and no push notifications, iCloud, Sign in with Apple, App Groups...
  Don't add entitlements unless the user has a paid developer account.
- Third-party code: Swift packages in `Package.swift` dependencies work if they support iOS and build with SwiftPM.
- Debug output: `print(...)` or `Logger` from `os`, read with `irisphone_logs` while the app runs.
