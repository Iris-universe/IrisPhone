//! User configuration (`irisphone config`) and well-known locations.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// xtool release installed by `irisphone setup`. Bump after testing a new release.
pub const XTOOL_VERSION: &str = "1.24.0";
/// Swift toolchain installed by `irisphone setup` (through swiftly).
pub const SWIFT_VERSION: &str = "6.4.1";
/// WSL distribution used when none is configured.
pub const DEFAULT_DISTRO: &str = "Ubuntu-24.04";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Prefix for the bundle ID of new apps, e.g. `com.alice` gives `com.alice.myapp`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_prefix: Option<String>,
    /// WSL distribution (Windows only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distro: Option<String>,
    /// Apple ID used by `irisphone login` (never the password).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apple_id: Option<String>,
}

pub const KEYS: &[(&str, &str)] = &[
    (
        "bundle_prefix",
        "prefix for new bundle IDs, e.g. com.yourname",
    ),
    ("distro", "WSL distribution to use (Windows)"),
    ("apple_id", "Apple ID suggested by `irisphone login`"),
];

impl Config {
    pub fn load() -> Config {
        std::fs::read_to_string(config_file())
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = config_file();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, toml::to_string_pretty(self)?)
            .with_context(|| format!("writing {}", path.display()))
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(match key {
            "bundle_prefix" => self.bundle_prefix.clone(),
            "distro" => self.distro.clone(),
            "apple_id" => self.apple_id.clone(),
            _ => bail!("unknown key '{key}'"),
        })
    }

    pub fn set(&mut self, key: &str, value: Option<String>) -> Result<()> {
        let value = value.filter(|v| !v.trim().is_empty());
        if key == "bundle_prefix" {
            if let Some(v) = &value {
                if !is_valid_bundle_id(v) {
                    bail!("'{v}' is not a valid bundle ID prefix (expected something like com.yourname)");
                }
            }
        }
        match key {
            "bundle_prefix" => self.bundle_prefix = value,
            "distro" => self.distro = value,
            "apple_id" => self.apple_id = value,
            _ => bail!("unknown key '{key}'"),
        }
        Ok(())
    }

    pub fn distro(&self) -> String {
        std::env::var("IRISPHONE_DISTRO")
            .ok()
            .or_else(|| self.distro.clone())
            .unwrap_or_else(|| DEFAULT_DISTRO.to_string())
    }

    /// Bundle prefix, or a guess from the OS user name (`com.<user>`).
    pub fn bundle_prefix_or_default(&self) -> String {
        if let Some(p) = &self.bundle_prefix {
            return p.clone();
        }
        let user = std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .unwrap_or_else(|_| "me".into());
        let clean: String = user
            .to_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        let clean = if clean.is_empty() || clean.starts_with(|c: char| c.is_ascii_digit()) {
            format!("dev{clean}")
        } else {
            clean
        };
        format!("com.{clean}")
    }
}

pub fn is_valid_bundle_id(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() >= 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                && !p.starts_with('-')
        })
}

/// `%APPDATA%\IrisPhone` on Windows, `~/.config/irisphone` elsewhere.
pub fn config_dir() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".into())).join("IrisPhone")
    } else if let Ok(x) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(x).join("irisphone")
    } else {
        home().join(".config").join("irisphone")
    }
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

/// `%LOCALAPPDATA%\IrisPhone` on Windows, `~/.local/share/irisphone` elsewhere.
pub fn data_dir() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into()))
            .join("IrisPhone")
    } else if let Ok(x) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(x).join("irisphone")
    } else {
        home().join(".local").join("share").join("irisphone")
    }
}

pub fn home() -> PathBuf {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_ids() {
        assert!(is_valid_bundle_id("com.alice"));
        assert!(is_valid_bundle_id("io.github.alice-b.app"));
        assert!(!is_valid_bundle_id("alice"));
        assert!(!is_valid_bundle_id("com..alice"));
        assert!(!is_valid_bundle_id("com.al ice"));
    }
}
