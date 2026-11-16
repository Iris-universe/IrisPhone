# How IrisPhone works

Building an iOS app needs three things Apple normally only offers on a Mac: a Swift compiler that targets iOS,
the iOS SDK (headers and libraries of UIKit, SwiftUI...), and a way to sign and install the app on a device.
IrisPhone assembles open-source pieces that do each of these on Linux, and runs Linux inside WSL on Windows.

## The pieces

| Piece | Role | Installed by |
|-------|------|--------------|
| WSL 2 + Ubuntu 24.04 | Linux on Windows | `wsl --install` (setup offers it) |
| [Swift](https://swift.org) (via [swiftly](https://github.com/swiftlang/swiftly)) | Compiler, cross-compiles to `arm64-apple-ios` | `irisphone setup` |
| iOS SDK | Apple's frameworks, extracted from `Xcode.xip` by `xtool sdk install` | `irisphone setup` + your download |
| [xtool](https://github.com/xtool-org/xtool) | Builds SwiftPM apps into `.app`, signs with an Apple ID, installs | `irisphone setup` |
| libimobiledevice / usbmuxd tools | Talk to the iPhone (pairing, logs, device info) | `irisphone setup` (apt) |
| Apple Devices (Microsoft Store) | Apple's USB driver + `usbmuxd` service on Windows | `irisphone setup` (winget) |

## The USB link on Windows

USB devices aren't visible inside WSL by default. Instead of forwarding USB, IrisPhone uses Apple's own Windows
service: the **Apple Devices** app ships `usbmuxd`, which listens on `127.0.0.1:27015`. With WSL's
**mirrored networking** (`networkingMode=mirrored` in `%USERPROFILE%\.wslconfig`), Linux shares Windows'
localhost, so Linux tools reach it by setting `USBMUXD_SOCKET_ADDRESS=127.0.0.1:27015`. No admin rights,
no `usbipd`.

`irisphone` starts that service on demand (it normally only runs while the Apple Devices app is open).

## Builds

`irisphone build` / `run`:

1. finds the project (`xtool.yml`, searching upwards from the current directory);
2. copies it with `rsync` to `~/.cache/irisphone/build/<name>-<hash>` on the Linux disk (much faster than
   building on `/mnt/c`), keeping the previous build cache;
3. runs `xtool dev build` or `xtool dev run --usb` there;
4. rewrites paths in the compiler output back to your project folder, so errors point at your files;
5. after `build`, copies `xtool/<App>.app` (or the `.ipa`) back next to your sources.

## Signing

xtool logs into Apple Developer Services with your Apple ID (`irisphone login`, password + 2FA; the session is
stored by xtool inside WSL, not by IrisPhone). For a free account it creates a development certificate and a
provisioning profile for each bundle ID: that's where the 7-day expiry and the 3-app / 10-ID-per-week limits
come from. A paid account can log in with an App Store Connect API key (`xtool auth login --mode key`).

## The binary

`irisphone` is a single Rust executable. The Linux-side logic is `scripts/irisphone.sh` and
`scripts/root-setup.sh`, embedded in the binary and written to `%LOCALAPPDATA%\IrisPhone\scripts`
(`~/.local/share/irisphone/scripts` on Linux) when needed. Settings live in `%APPDATA%\IrisPhone\config.toml`.

On Linux, everything runs natively (`sudo` for the apt step); the iPhone is reached through the system `usbmuxd`.

## Claude Code integration

`irisphone mcp` is a [Model Context Protocol](https://modelcontextprotocol.io) server on stdio exposing the same
operations as tools. Build output is summarised for the model: diagnostics with their context and the end of
the log, compiler command lines truncated. Each new project ships a `CLAUDE.md` with the rules of iOS development
without Xcode, checked against real builds (e.g. SwiftData's `@Model` and `.xcassets` fail, `#Preview` compiles).
