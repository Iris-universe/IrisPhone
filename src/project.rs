//! App projects: creating one from a template, finding the one we're in, reading its metadata.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

pub struct Template {
    pub name: &'static str,
    pub description: &'static str,
    files: &'static [(&'static str, &'static str)],
}

macro_rules! t {
    ($path:literal) => {
        include_str!(concat!("../templates/", $path))
    };
}

const COMMON: &[(&str, &str)] = &[
    ("xtool.yml", t!("common/xtool.yml")),
    ("Info.plist", t!("common/Info.plist")),
    (".gitignore", t!("common/gitignore")),
    (".mcp.json", t!("common/mcp.json")),
    ("CLAUDE.md", t!("common/CLAUDE.md")),
    ("README.md", t!("common/README.md")),
];

pub const TEMPLATES: &[Template] = &[
    Template {
        name: "swiftui",
        description: "Native SwiftUI app (default)",
        files: &[
            ("Package.swift", t!("swiftui/Package.swift")),
            (
                "Sources/__APP_NAME__/App.swift",
                t!("swiftui/Sources/App.swift"),
            ),
            (
                "Sources/__APP_NAME__/ContentView.swift",
                t!("swiftui/Sources/ContentView.swift"),
            ),
        ],
    },
    Template {
        name: "webview",
        description: "HTML/CSS/JS app in a native shell, with a JS <-> Swift bridge",
        files: &[
            ("Package.swift", t!("webview/Package.swift")),
            (
                "Sources/__APP_NAME__/App.swift",
                t!("webview/Sources/App.swift"),
            ),
            (
                "Sources/__APP_NAME__/WebAppView.swift",
                t!("webview/Sources/WebAppView.swift"),
            ),
            (
                "Sources/__APP_NAME__/Web/index.html",
                t!("webview/Sources/Web/index.html"),
            ),
            (
                "Sources/__APP_NAME__/Web/native.js",
                t!("webview/Sources/Web/native.js"),
            ),
            (
                "Sources/__APP_NAME__/Web/app.js",
                t!("webview/Sources/Web/app.js"),
            ),
            (
                "Sources/__APP_NAME__/Web/style.css",
                t!("webview/Sources/Web/style.css"),
            ),
        ],
    },
];

pub fn template(name: &str) -> Result<&'static Template> {
    TEMPLATES.iter().find(|t| t.name == name).with_context(|| {
        let names: Vec<_> = TEMPLATES.iter().map(|t| t.name).collect();
        format!(
            "unknown template '{name}' (available: {})",
            names.join(", ")
        )
    })
}

/// `MyApp` is fine; must be a valid Swift type name.
pub fn validate_name(name: &str) -> Result<()> {
    let ok = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && name.chars().all(|c| c.is_ascii_alphanumeric())
        && name.len() <= 50;
    if !ok {
        bail!("invalid app name '{name}': use letters and digits only, starting with a letter (e.g. MyApp)");
    }
    Ok(())
}

/// "MyCoolApp" -> "My Cool App"
pub fn display_name(name: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && c.is_ascii_uppercase() {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
            if prev.is_ascii_lowercase()
                || prev.is_ascii_digit()
                || (prev.is_ascii_uppercase() && next_lower)
            {
                out.push(' ');
            }
        }
        out.push(c);
    }
    out
}

/// Creates `<parent>/<name>` from a template. Returns the project directory.
pub fn create(parent: &Path, name: &str, template: &Template, bundle_id: &str) -> Result<PathBuf> {
    validate_name(name)?;
    let dir = parent.join(name);
    if dir.exists()
        && std::fs::read_dir(&dir)
            .map(|mut d| d.next().is_some())
            .unwrap_or(true)
    {
        bail!("{} already exists and is not empty", dir.display());
    }
    let display = display_name(name);
    let fill = |s: &str| {
        s.replace("__APP_NAME__", name)
            .replace("__BUNDLE_ID__", bundle_id)
            .replace("__DISPLAY_NAME__", &display)
    };
    for (path, content) in COMMON.iter().chain(template.files.iter()) {
        let target = dir.join(fill(path));
        if let Some(p) = target.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(&target, fill(content).replace("\r\n", "\n"))
            .with_context(|| format!("writing {}", target.display()))?;
    }
    Ok(dir)
}

/// Finds the project containing `start` (a directory with `xtool.yml`, searching upwards).
pub fn find(start: Option<&Path>) -> Result<PathBuf> {
    let start = match start {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()?,
    };
    let start = if start.is_absolute() {
        start
    } else {
        std::env::current_dir()?.join(start)
    };
    if !start.exists() {
        bail!("{} does not exist", start.display());
    }
    let mut dir = start.as_path();
    loop {
        if dir.join("xtool.yml").is_file() {
            return Ok(dir.to_path_buf());
        }
        match dir.parent() {
            Some(p) => dir = p,
            None => bail!(
                "no IrisPhone project here ({} has no xtool.yml in it or above it).\n\
                 Create one with: irisphone new MyApp",
                start.display()
            ),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Info {
    pub path: String,
    pub name: String,
    pub bundle_id: Option<String>,
    pub info_plist: Option<String>,
    pub icon: Option<String>,
}

/// Reads `xtool.yml` (simple `key: value` lines) and the product name from `Package.swift`.
pub fn info(dir: &Path) -> Result<Info> {
    let yml = std::fs::read_to_string(dir.join("xtool.yml"))
        .with_context(|| format!("reading {}", dir.join("xtool.yml").display()))?;
    let get = |key: &str| {
        yml.lines().find_map(|l| {
            let l = l.trim();
            let (k, v) = l.split_once(':')?;
            (k.trim() == key).then(|| v.trim().trim_matches(['"', '\'']).to_string())
        })
    };
    let name = std::fs::read_to_string(dir.join("Package.swift"))
        .ok()
        .and_then(|s| {
            let i = s.find("name:")?;
            let rest = &s[i + 5..];
            let a = rest.find('"')?;
            let b = rest[a + 1..].find('"')?;
            Some(rest[a + 1..a + 1 + b].to_string())
        })
        .unwrap_or_else(|| {
            dir.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
    Ok(Info {
        path: dir.display().to_string(),
        name,
        bundle_id: get("bundleID"),
        info_plist: get("infoPath"),
        icon: get("iconPath"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(validate_name("MyApp2").is_ok());
        assert!(validate_name("2App").is_err());
        assert!(validate_name("My App").is_err());
        assert_eq!(display_name("MyCoolApp"), "My Cool App");
        assert_eq!(display_name("HTMLViewer"), "HTML Viewer");
        assert_eq!(display_name("App2Go"), "App2 Go");
    }

    #[test]
    fn create_and_read() {
        let tmp = std::env::temp_dir().join(format!("irisphone-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        for t in TEMPLATES {
            let dir = create(&tmp.join(t.name), "DemoApp", t, "com.test.demoapp").unwrap();
            let i = info(&dir).unwrap();
            assert_eq!(i.name, "DemoApp");
            assert_eq!(i.bundle_id.as_deref(), Some("com.test.demoapp"));
            assert!(dir.join("Sources/DemoApp/App.swift").is_file());
            let all = std::fs::read_to_string(dir.join("Sources/DemoApp/App.swift")).unwrap();
            assert!(!all.contains("__"), "unfilled placeholder in {}", t.name);
            assert_eq!(find(Some(&dir.join("Sources"))).unwrap(), dir);
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
