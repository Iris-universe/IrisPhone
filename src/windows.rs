//! Windows side: Apple's USB driver (the "Apple Devices" Store app), WSL configuration,
//! and locating the Xcode archive. Functions are harmless no-ops on other platforms.

use crate::config;
use anyhow::{Context, Result};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Microsoft Store ID of Apple Devices (installs the iPhone USB driver + usbmuxd service).
pub const APPLE_DEVICES_STORE_ID: &str = "9NP83LWLPZ9K";
pub const MUX_PORT: u16 = 27015;
pub const XCODE_DOWNLOAD_URL: &str = "https://developer.apple.com/download/all/?q=Xcode";

pub struct AppleDevices {
    pub install_location: String,
    pub family_name: String,
}

fn powershell(script: &str) -> Command {
    let mut c = Command::new("powershell.exe");
    c.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        script,
    ]);
    c
}

pub fn apple_devices() -> Option<AppleDevices> {
    if !cfg!(windows) {
        return None;
    }
    let out = powershell(
        "$p = Get-AppxPackage -Name AppleInc.AppleDevices | Select-Object -First 1; \
         if ($p) { $p.InstallLocation + '|' + $p.PackageFamilyName }",
    )
    .output()
    .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let (loc, fam) = s.trim().split_once('|')?;
    Some(AppleDevices {
        install_location: loc.into(),
        family_name: fam.into(),
    })
}

pub fn install_apple_devices() -> Result<bool> {
    let status = Command::new("winget")
        .args([
            "install",
            "--id",
            APPLE_DEVICES_STORE_ID,
            "--source",
            "msstore",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--silent",
        ])
        .status()
        .context("winget not found (it ships with Windows 11 as \"App Installer\")")?;
    Ok(status.success())
}

/// True when Apple's usbmuxd service answers on 127.0.0.1:27015.
pub fn mux_listening() -> bool {
    let addr: SocketAddr = ([127, 0, 0, 1], MUX_PORT).into();
    TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
}

/// Starts Apple's USB service if needed. Returns false when it can't be started.
pub fn start_apple_service() -> bool {
    if !cfg!(windows) || mux_listening() {
        return true;
    }
    let Some(pkg) = apple_devices() else {
        return false;
    };
    // The launcher must run inside the package context, which only this cmdlet provides.
    let launcher = Path::new(&pkg.install_location).join("AppleMobileDeviceLauncher.exe");
    let script = format!(
        "Invoke-CommandInDesktopPackage -PackageFamilyName '{}' -AppId 'App' -Command '{}' -PreventBreakaway",
        pkg.family_name.replace('\'', "''"),
        launcher.display().to_string().replace('\'', "''"),
    );
    let _ = powershell(&script).status();
    for _ in 0..30 {
        if mux_listening() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    false
}

/// Installed WSL distributions (`wsl -l -q` prints UTF-16).
pub fn wsl_distros() -> Option<Vec<String>> {
    let out = Command::new("wsl.exe").args(["-l", "-q"]).output().ok()?;
    if !out.status.success() {
        return Some(Vec::new());
    }
    Some(
        decode_utf16_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
    )
}

fn decode_utf16_lossy(b: &[u8]) -> String {
    if b.len() >= 2 && b.iter().skip(1).step_by(2).take(8).all(|&x| x == 0) {
        let u: Vec<u16> = b
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&u)
    } else {
        String::from_utf8_lossy(b).into_owned()
    }
}

fn wslconfig_path() -> PathBuf {
    config::home().join(".wslconfig")
}

/// WSL must use mirrored networking so Linux reaches 127.0.0.1:27015 on Windows.
pub fn wsl_mirrored() -> bool {
    std::fs::read_to_string(wslconfig_path())
        .map(|s| {
            s.lines().any(|l| {
                let l = l.trim().replace(' ', "");
                l.eq_ignore_ascii_case("networkingMode=mirrored")
            })
        })
        .unwrap_or(false)
}

/// Sets `networkingMode=mirrored` in `%USERPROFILE%\.wslconfig`. Returns true if it changed.
pub fn set_wsl_mirrored() -> Result<bool> {
    if wsl_mirrored() {
        return Ok(false);
    }
    let path = wslconfig_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let new = set_networking_mode(&content);
    std::fs::write(&path, new).with_context(|| format!("writing {}", path.display()))?;
    Ok(true)
}

fn set_networking_mode(content: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    if let Some(i) = lines.iter().position(|l| {
        l.trim()
            .to_lowercase()
            .replace(' ', "")
            .starts_with("networkingmode=")
    }) {
        lines[i] = "networkingMode=mirrored".into();
    } else if let Some(i) = lines
        .iter()
        .position(|l| l.trim().eq_ignore_ascii_case("[wsl2]"))
    {
        lines.insert(i + 1, "networkingMode=mirrored".into());
    } else {
        if lines.last().is_some_and(|l| !l.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push("[wsl2]".into());
        lines.push("networkingMode=mirrored".into());
    }
    lines.join("\r\n") + "\r\n"
}

pub fn downloads_dir() -> PathBuf {
    config::home().join("Downloads")
}

/// Most recent `Xcode*.xip` in the current directory or the Downloads folder.
pub fn find_xip() -> Option<PathBuf> {
    let mut dirs = vec![downloads_dir()];
    if let Ok(cwd) = std::env::current_dir() {
        dirs.insert(0, cwd);
    }
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if name.starts_with("xcode") && name.ends_with(".xip") {
                if let Ok(m) = e.metadata() {
                    found.push((m.modified().unwrap_or(std::time::UNIX_EPOCH), e.path()));
                }
            }
        }
    }
    found.sort();
    found.pop().map(|(_, p)| p)
}

pub fn open_url(url: &str) {
    if cfg!(windows) {
        let _ = Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .status();
    } else {
        let _ = Command::new("xdg-open").arg(url).status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wslconfig_edit() {
        assert_eq!(
            set_networking_mode(""),
            "[wsl2]\r\nnetworkingMode=mirrored\r\n"
        );
        assert_eq!(
            set_networking_mode("[wsl2]\nmemory=8GB"),
            "[wsl2]\r\nnetworkingMode=mirrored\r\nmemory=8GB\r\n"
        );
        assert_eq!(
            set_networking_mode("[wsl2]\nnetworkingMode = NAT"),
            "[wsl2]\r\nnetworkingMode=mirrored\r\n"
        );
    }

    #[test]
    fn utf16() {
        let b: Vec<u8> = "Ubuntu\r\n"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        assert_eq!(decode_utf16_lossy(&b), "Ubuntu\r\n");
    }
}
