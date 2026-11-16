# IrisPhone (contributor notes for Claude)

Rust CLI that builds/installs iOS apps from Windows (via WSL) or Linux using xtool. See CONTRIBUTING.md for the layout.

- Check: `cargo fmt && cargo clippy --all-targets && cargo test`. Scripts: `bash -n scripts/*.sh`.
- `irisphone mcp` must only write JSON-RPC to stdout. Use `ui::*` (stderr) for messages.
- Linux-side logic belongs in `scripts/irisphone.sh` (embedded with `include_str!`); keep the Rust side thin.
- Rules given to users' Claude (templates/common/CLAUDE.md, claude-plugin/skills/ios-app-dev/SKILL.md) must be
  verified by a real `irisphone build` before being written down.
- End-to-end test: `irisphone new Demo --dir <tmp>` then `irisphone build <tmp>/Demo` for each template.
- Never commit an Xcode `.xip`, Apple IDs, or anything from a user's machine.
