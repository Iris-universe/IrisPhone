---
name: ios-app-dev
description: Build iPhone/iPad apps on Windows or Linux with IrisPhone (no Mac, no Xcode). Use when the user wants to create, change, build, install or debug an iOS app, or when the project has an xtool.yml.
---

# iOS apps without a Mac (IrisPhone)

IrisPhone runs Swift and the iOS SDK on Linux (inside WSL on Windows). [xtool](https://github.com/xtool-org/xtool)
builds SwiftPM projects for iOS, signs them with the user's Apple ID and installs them on an iPhone over USB.
The `irisphone_*` MCP tools drive all of it; the `irisphone` CLI does the same from a terminal.

## Workflow

1. Not sure the machine is ready? `irisphone_doctor`. Each failing check comes with its fix.
   If the toolchain is missing, the user runs `irisphone setup` in their terminal (interactive, downloads ~3 GB).
2. New app: `irisphone_new_app` (`swiftui` template by default, `webview` for an HTML/JS app in a native shell).
   Then read the generated `CLAUDE.md`.
3. After **every** change: `irisphone_build`, fix all errors (paths in the output are the real project paths),
   repeat until `BUILD SUCCEEDED`.
4. `irisphone_run` installs and launches on the iPhone (1-5 min). There is no simulator.
5. `irisphone_logs` (optionally with `launch: true`) captures `print()` / `Logger` output and crash messages
   while the user tries the app. Ask them what to tap if needed.

Things only the user can do (ask, don't retry in a loop): `irisphone login` (password + 2FA code), plugging in
and unlocking the iPhone, tapping **Trust**, enabling Developer Mode, trusting the developer profile in
Settings > General > VPN & Device Management.

## Project layout

- `Package.swift`: SwiftPM, exactly one library product = the app. No `.xcodeproj`.
- `xtool.yml`: `bundleID`, `infoPath` (extra Info.plist keys), optional `iconPath` (1024x1024 PNG).
- `Sources/<App>/`: code; the `@main` `App` struct is the entry point.

## What doesn't exist without Xcode

- No storyboards, `.xib`, or `.xcassets` asset catalogs (no `actool` on Linux: the build fails).
  Colors in code; images as plain PNG/JPEG resources declared in `Package.swift`
  (`resources: [.process("Resources")]`) and loaded through `Bundle.module`.
- No SwiftData: `@Model` has no Linux macro plugin. Use `Codable` + JSON files, or `UserDefaults`.
  `@Observable` and the SwiftUI property wrappers work. `#Preview` compiles but there is no canvas.
- Every privacy-sensitive API needs its `NS...UsageDescription` in `Info.plist`, or iOS kills the app.
- Free Apple ID: apps expire after 7 days, 3 sideloaded apps max, 10 new bundle IDs per week,
  no push notifications / iCloud / App Groups / Sign in with Apple. Don't add entitlements unless the user
  says they have a paid account.
