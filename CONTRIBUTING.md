# Contributing to IrisPhone

Thanks for helping! Bug reports, docs, templates and code are all welcome.

## Reporting a problem

Open an issue with the output of `irisphone doctor --json`, your Windows/Linux version, and what you ran.
Remove your Apple ID e-mail and device names if you'd rather keep them private.

## Building from source

```sh
git clone https://github.com/Iris-universe/IrisPhone
cd IrisPhone
cargo build            # target/debug/irisphone(.exe)
cargo test
cargo clippy --all-targets
cargo fmt
```

Rust stable, no other dependency. The Linux toolchain (`irisphone setup`) is only needed to test builds and installs.

## Layout

| Path | What |
|------|------|
| `src/main.rs` | CLI (clap) |
| `src/host.rs` | Runs the Linux side: WSL or native, path conversion, output capture |
| `src/windows.rs` | Apple Devices / usbmuxd service, `.wslconfig`, Xcode `.xip` lookup |
| `src/setup.rs`, `src/doctor.rs` | Installation and diagnostics |
| `src/project.rs` | Templates and project discovery |
| `src/ops.rs` | Build / run / logs shared by the CLI and MCP |
| `src/mcp.rs` | MCP server for Claude Code |
| `src/claude.rs` | Registering the MCP server in Claude Code |
| `scripts/` | Linux-side bash, embedded in the binary |
| `templates/` | Project templates (`__APP_NAME__`, `__BUNDLE_ID__`, `__DISPLAY_NAME__` placeholders) |
| `claude-plugin/`, `.claude-plugin/` | Claude Code plugin and marketplace |

## Guidelines

- Keep setup idempotent: every step checks before acting, so `irisphone setup` can always be re-run.
- Anything printed on stdout by `irisphone mcp` must be JSON-RPC; logs go to stderr.
- A rule in `templates/common/CLAUDE.md` or the plugin skill must be verified by a real build. Keep both in sync.
- New template: add the files under `templates/<name>/`, register them in `src/project.rs`, and make sure the
  generated project builds with `irisphone build`.
- Shell scripts must stay LF (`.gitattributes`) and pass `bash -n` / `shellcheck`.

## Releasing

Bump the version in `Cargo.toml` and `claude-plugin/.claude-plugin/plugin.json`, then push a `vX.Y.Z` tag:
the release workflow builds Windows (x64, arm64) and Linux (x64, arm64) binaries and attaches them to a
GitHub release, which the installers download.
