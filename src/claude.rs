//! Claude Code integration: registers `irisphone mcp` as a user-level MCP server.

use crate::{config, ui};
use anyhow::{bail, Result};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// How to start Claude Code: from the PATH, the native installer location, or the binary
/// bundled with the VS Code / Cursor extension (users of the extension often have no `claude` on PATH).
fn claude() -> Command {
    static FOUND: OnceLock<Option<PathBuf>> = OnceLock::new();
    match FOUND.get_or_init(find_binary) {
        Some(p) => Command::new(p),
        // `claude` may be a .cmd shim on Windows (npm), which Command can't start directly.
        None if cfg!(windows) => {
            let mut c = Command::new("cmd.exe");
            c.args(["/d", "/c", "claude"]);
            c
        }
        None => Command::new("claude"),
    }
}

fn works(mut c: Command) -> bool {
    c.arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn find_binary() -> Option<PathBuf> {
    let on_path = if cfg!(windows) {
        let mut c = Command::new("cmd.exe");
        c.args(["/d", "/c", "claude"]);
        c
    } else {
        Command::new("claude")
    };
    if works(on_path) {
        return None;
    }
    let exe = if cfg!(windows) {
        "claude.exe"
    } else {
        "claude"
    };
    let home = config::home();
    let mut candidates = vec![home.join(".local").join("bin").join(exe)];
    for editor in [".vscode", ".vscode-insiders", ".cursor", ".windsurf"] {
        let Ok(rd) = std::fs::read_dir(home.join(editor).join("extensions")) else {
            continue;
        };
        let mut ext: Vec<PathBuf> = rd
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("anthropic.claude-code-")
            })
            .map(|e| e.path().join("resources").join("native-binary").join(exe))
            .collect();
        // Newest extension version first.
        ext.sort_by_key(|p| {
            std::cmp::Reverse(std::fs::metadata(p).and_then(|m| m.modified()).ok())
        });
        candidates.extend(ext);
    }
    candidates
        .into_iter()
        .find(|p| p.is_file() && works(Command::new(p)))
}

pub fn installed() -> bool {
    works(claude())
}

pub fn registered() -> bool {
    claude()
        .args(["mcp", "get", "irisphone"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

pub fn register() -> Result<()> {
    if !installed() {
        bail!("Claude Code is not installed (https://claude.com/claude-code)");
    }
    if registered() {
        let _ = claude()
            .args(["mcp", "remove", "--scope", "user", "irisphone"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let exe = std::env::current_exe()?;
    let status = claude()
        .args(["mcp", "add", "--scope", "user", "irisphone", "--"])
        .arg(&exe)
        .arg("mcp")
        .status()?;
    if !status.success() {
        bail!("`claude mcp add` failed");
    }
    Ok(())
}

pub fn unregister() -> Result<()> {
    let status = claude()
        .args(["mcp", "remove", "--scope", "user", "irisphone"])
        .status()?;
    if !status.success() {
        bail!("`claude mcp remove` failed");
    }
    Ok(())
}

/// Called by setup: register if Claude Code is there and IrisPhone isn't registered yet.
pub fn offer_registration(yes: bool) -> Result<()> {
    if !installed() {
        ui::info("Claude Code not found. Once installed, run `irisphone claude` to connect it.");
        return Ok(());
    }
    if registered() {
        ui::ok("IrisPhone tools already available in Claude Code");
        return Ok(());
    }
    if yes
        || ui::confirm(
            "Make the IrisPhone tools available in Claude Code (all projects)?",
            true,
        )
    {
        register()?;
        ui::ok("registered: Claude can now build, install and debug your iPhone apps");
    }
    Ok(())
}
