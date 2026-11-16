# Troubleshooting

Start with `irisphone doctor`: it checks each link and prints the fix for what's broken.

## The iPhone isn't found

- Use a data cable (some charging cables carry no data), unlock the iPhone, tap **Trust** when asked.
- `irisphone pair` re-runs the pairing and tells you when the iPhone refused it.
- If you tapped **Don't Trust** once, the refusal is remembered: on the iPhone,
  Settings > General > Transfer or Reset iPhone > Reset > **Reset Location & Privacy**, then plug it in again.
- Windows: `Apple USB service: not running` → open the **Apple Devices** app once.
- Windows: `iPhone service reachable from Linux: no` while the Apple service runs → mirrored networking
  isn't active. Check `%USERPROFILE%\.wslconfig` has `networkingMode=mirrored` under `[wsl2]`, then
  `wsl --shutdown`. Mirrored networking needs Windows 11 22H2 or later.

## Installing fails

| Message | Fix |
|---------|-----|
| not logged in / unauthorized | `irisphone login` |
| maximum number of apps / App ID limit | Free account: 3 sideloaded apps, 10 new bundle IDs per 7 days. Uninstall one (`irisphone uninstall --bundle-id ...`) or reuse a bundle ID |
| Developer Mode | iPhone: Settings > Privacy & Security > Developer Mode > On, restart |
| app opens then closes / "Untrusted Developer" | iPhone: Settings > General > VPN & Device Management > your Apple ID > Trust |
| app stopped opening after a week | Free-account apps expire after 7 days: `irisphone run` again |

## Build errors that only happen here

- `external macro implementation type 'SwiftDataMacros...' could not be found` → SwiftData isn't supported;
  use Codable + files or UserDefaults.
- `actool` / `.xcassets` errors → asset catalogs need Xcode; use plain image files as resources.
- Errors mentioning `Package.swift` products → an xtool project must have exactly one `.library` product.

## Setup

- **WSL install asks for a restart**: restart, open Ubuntu once to create your Linux user, run `irisphone setup` again.
- **Xcode download**: Apple requires signing in with an Apple ID (free) at
  <https://developer.apple.com/download/all/?q=Xcode>. Any recent stable Xcode works. Pass the file
  explicitly with `irisphone setup --xip <path>` if it isn't in Downloads.
- **Not on Ubuntu-24.04**: `irisphone config distro <name>` to use another WSL distribution (Ubuntu/Debian only).

## Logs and reports

- `irisphone doctor --json` gives a machine-readable report: paste it in issues.
- Builds live in `~/.cache/irisphone/build` inside WSL; deleting that folder forces a clean build.
