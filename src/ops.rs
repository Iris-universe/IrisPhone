//! Build / run / logs: the operations shared by the CLI and the MCP server.

use crate::host::{CaptureOpts, Captured, Host};
use crate::project;
use crate::windows;
use anyhow::{bail, Result};
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Copy, Default)]
pub struct BuildOpts {
    pub release: bool,
    pub ipa: bool,
}

/// Starts Apple's USB service on Windows (no-op elsewhere).
pub fn ensure_usb(host: &Host) -> Result<()> {
    if host.is_wsl() && !windows::start_apple_service() {
        if windows::apple_devices().is_none() {
            bail!("Apple Devices is not installed (it provides the iPhone USB driver). Run: irisphone setup");
        }
        bail!(
            "Apple's USB service did not start. Open the \"Apple Devices\" app once, then retry."
        );
    }
    Ok(())
}

fn build_args(dir_linux: &str, opts: BuildOpts, run: bool, udid: Option<&str>) -> Vec<String> {
    let mut a = vec![
        if run { "deploy" } else { "build" }.to_string(),
        dir_linux.to_string(),
    ];
    if opts.release {
        a.extend(["--configuration".into(), "release".into()]);
    }
    if opts.ipa && !run {
        a.push("--ipa".into());
    }
    if let Some(u) = udid {
        a.extend(["--udid".into(), u.into()]);
    }
    a
}

/// Builds the project. `echo` streams the output to the terminal as it comes.
pub fn build(
    host: &Host,
    dir: &Path,
    opts: BuildOpts,
    echo: bool,
    timeout: Option<Duration>,
) -> Result<Captured> {
    let linux = host.linux_path(dir)?;
    let args = build_args(&linux, opts, false, None);
    let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    host.capture_with(
        &args,
        CaptureOpts {
            timeout,
            echo,
            project: Some(dir.display().to_string()),
        },
    )
}

/// Builds, installs and launches on the iPhone, capturing the output (MCP).
pub fn run_captured(
    host: &Host,
    dir: &Path,
    opts: BuildOpts,
    udid: Option<&str>,
    timeout: Duration,
) -> Result<Captured> {
    ensure_usb(host)?;
    let linux = host.linux_path(dir)?;
    let args = build_args(&linux, opts, true, udid);
    let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    host.capture_with(
        &args,
        CaptureOpts {
            timeout: Some(timeout),
            echo: false,
            project: Some(dir.display().to_string()),
        },
    )
}

/// Builds, installs and launches on the iPhone, attached to the terminal (CLI).
pub fn run_interactive(
    host: &Host,
    dir: &Path,
    opts: BuildOpts,
    udid: Option<&str>,
) -> Result<i32> {
    ensure_usb(host)?;
    let linux = host.linux_path(dir)?;
    let args = build_args(&linux, opts, true, udid);
    let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    host.run(&args)
}

/// Process name of the app on the device (= the SwiftPM product name).
pub fn process_name(dir: &Path) -> Result<String> {
    Ok(project::info(dir)?.name)
}

pub fn bundle_id(dir: &Path) -> Result<String> {
    match project::info(dir)?.bundle_id {
        Some(b) => Ok(b),
        None => bail!("no bundleID in {}", dir.join("xtool.yml").display()),
    }
}

/// Shortens a long build log for an AI: every diagnostic with its context, plus the tail.
pub fn summarize(output: &str, success: bool) -> String {
    // Compiler command lines can be thousands of characters long: useless for diagnosis.
    let shortened: Vec<String> = output
        .lines()
        .map(|l| {
            if l.chars().count() > 400 {
                let head: String = l.chars().take(300).collect();
                format!("{head}... [{} more characters]", l.chars().count() - 300)
            } else {
                l.to_string()
            }
        })
        .collect();
    let lines: Vec<&str> = shortened.iter().map(String::as_str).collect();
    if lines.len() <= 60 {
        return lines.join("\n");
    }
    if success {
        let warnings = lines.iter().filter(|l| l.contains("warning:")).count();
        let tail = lines[lines.len() - 15..].join("\n");
        return format!(
            "({} lines of output, {warnings} warnings; last lines:)\n{tail}",
            lines.len()
        );
    }
    let mut keep = vec![false; lines.len()];
    for (i, l) in lines.iter().enumerate() {
        if l.contains("error:") || l.contains("fatal error") || l.contains("Error:") {
            for k in keep
                .iter_mut()
                .take((i + 5).min(lines.len()))
                .skip(i.saturating_sub(1))
            {
                *k = true;
            }
        }
    }
    for k in keep.iter_mut().skip(lines.len().saturating_sub(20)) {
        *k = true;
    }
    let mut out = format!(
        "({} lines of output, showing errors and the end)\n",
        lines.len()
    );
    let mut skipped = false;
    for (i, l) in lines.iter().enumerate() {
        if keep[i] {
            if skipped {
                out.push_str("...\n");
                skipped = false;
            }
            out.push_str(l);
            out.push('\n');
        } else {
            skipped = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_keeps_errors() {
        let mut log: Vec<String> = (0..200).map(|i| format!("Compiling step {i}")).collect();
        log[50] = "/p/Sources/A/App.swift:3:5: error: cannot find 'x' in scope".into();
        let s = summarize(&log.join("\n"), false);
        assert!(s.contains("error: cannot find 'x'"));
        assert!(s.contains("Compiling step 199"));
        assert!(!s.contains("Compiling step 100\n"));
    }
}
