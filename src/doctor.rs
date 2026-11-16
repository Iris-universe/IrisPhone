//! `irisphone doctor`: checks every link of the chain and says how to fix what's broken.

use crate::config;
use crate::host::Host;
use crate::{ui, windows};
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub id: &'static str,
    pub label: &'static str,
    /// "ok", "warn" or "fail"
    pub status: &'static str,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub version: &'static str,
    pub platform: &'static str,
    /// Everything needed to build is installed.
    pub can_build: bool,
    /// Building + an iPhone ready to receive apps.
    pub can_deploy: bool,
    pub checks: Vec<Check>,
}

fn check(
    id: &'static str,
    label: &'static str,
    ok: bool,
    detail: impl Into<String>,
    fix: &str,
) -> Check {
    Check {
        id,
        label,
        status: if ok { "ok" } else { "fail" },
        detail: detail.into(),
        fix: (!ok).then(|| fix.to_string()),
    }
}

pub fn run(host: &Host) -> Report {
    let mut checks = Vec::new();
    let mut linux_ok = true;

    if let Host::Wsl { distro } = host {
        let distros = windows::wsl_distros();
        let has_wsl = distros.is_some();
        let has_distro = distros
            .as_ref()
            .is_some_and(|d| d.iter().any(|x| x == distro));
        checks.push(check(
            "wsl",
            "WSL",
            has_wsl,
            if has_wsl {
                "installed"
            } else {
                "not installed"
            },
            "irisphone setup (or: wsl --install)",
        ));
        checks.push(check(
            "distro",
            "Linux distribution",
            has_distro,
            if has_distro {
                distro.clone()
            } else {
                format!("{distro} missing")
            },
            "irisphone setup",
        ));
        linux_ok = has_distro;
        let mirrored = windows::wsl_mirrored();
        checks.push(check(
            "wsl_network",
            "WSL mirrored networking",
            mirrored,
            if mirrored {
                "on"
            } else {
                "off: Linux can't reach the iPhone service"
            },
            "irisphone setup",
        ));
        let apple = windows::apple_devices().is_some();
        checks.push(check(
            "apple_devices",
            "Apple Devices (USB driver)",
            apple,
            if apple { "installed" } else { "missing" },
            "irisphone setup (or install \"Apple Devices\" from the Microsoft Store)",
        ));
        let svc = apple && windows::start_apple_service();
        checks.push(check(
            "apple_service",
            "Apple USB service",
            svc,
            if svc {
                "listening on 127.0.0.1:27015"
            } else {
                "not running"
            },
            "open the Apple Devices app once, then retry",
        ));
    }

    let kv: HashMap<String, String> = if linux_ok {
        host.capture(&["doctor"], Some(Duration::from_secs(60)))
            .map(|c| {
                c.output
                    .lines()
                    .filter_map(|l| l.split_once('='))
                    .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        HashMap::new()
    };
    let v = |k: &str| kv.get(k).cloned().unwrap_or_default();

    let swift = v("swift");
    checks.push(check(
        "swift",
        "Swift toolchain",
        !swift.is_empty(),
        if swift.is_empty() {
            "missing".into()
        } else {
            format!("Swift {swift}")
        },
        "irisphone setup",
    ));
    let xtool = v("xtool");
    let mut c = check(
        "xtool",
        "xtool",
        !xtool.is_empty(),
        if xtool.is_empty() {
            "missing".into()
        } else {
            xtool.clone()
        },
        "irisphone setup",
    );
    if !xtool.is_empty() && xtool != config::XTOOL_VERSION {
        c.status = "warn";
        c.detail = format!(
            "{xtool} (IrisPhone is tested with {})",
            config::XTOOL_VERSION
        );
        c.fix = Some("irisphone setup".into());
    }
    checks.push(c);
    checks.push(check(
        "sdk",
        "iOS SDK",
        v("sdk") == "ok",
        if v("sdk") == "ok" {
            "installed"
        } else {
            "missing (built from Xcode.xip)"
        },
        "irisphone setup (it will ask for Xcode.xip)",
    ));
    checks.push(check(
        "usbmux_tools",
        "libimobiledevice",
        v("usbmux_tools") == "ok",
        if v("usbmux_tools") == "ok" {
            "installed"
        } else {
            "missing"
        },
        "irisphone setup",
    ));

    let can_build = linux_ok
        && ["swift", "xtool", "sdk"]
            .iter()
            .all(|k| checks.iter().any(|c| c.id == *k && c.status != "fail"));

    let mux = v("usbmux") == "ok";
    checks.push(check(
        "usbmux",
        "iPhone service reachable from Linux",
        mux,
        if mux { "yes" } else { "no" },
        if host.is_wsl() {
            "irisphone setup, then: wsl --shutdown"
        } else {
            "sudo systemctl start usbmuxd"
        },
    ));
    let auth = v("auth");
    let logged = !auth.is_empty() && !auth.to_lowercase().contains("not logged");
    checks.push(check(
        "apple_id",
        "Apple ID",
        logged,
        if auth.is_empty() {
            "unknown".into()
        } else {
            auth
        },
        "irisphone login",
    ));
    let dev = v("device");
    let has_dev = !dev.is_empty() && dev != "none";
    let mut dc = check(
        "device",
        "iPhone connected",
        has_dev,
        if has_dev {
            let name = v("device_name");
            let ios = v("ios_version");
            format!(
                "{} {}{}",
                if name.is_empty() { &dev } else { &name },
                if ios.is_empty() { "" } else { "iOS " },
                ios
            )
        } else {
            "none".into()
        },
        "plug the iPhone in with a cable and unlock it",
    );
    if !has_dev {
        dc.status = "warn";
    }
    checks.push(dc);
    if has_dev {
        let paired = v("paired") == "yes";
        checks.push(check(
            "paired",
            "Trusted by the iPhone",
            paired,
            if paired { "yes" } else { "no" },
            "irisphone pair (then tap Trust on the iPhone)",
        ));
        let dm = v("developer_mode");
        if !dm.is_empty() {
            let on = dm == "true";
            let mut c = check(
                "developer_mode", "Developer Mode", on,
                if on { "on" } else { "off" },
                "iPhone: Settings > Privacy & Security > Developer Mode (appears after the first install)",
            );
            if !on {
                c.status = "warn";
            }
            checks.push(c);
        }
    }

    let can_deploy = can_build
        && ["usbmux", "apple_id", "paired"]
            .iter()
            .all(|k| checks.iter().any(|c| c.id == *k && c.status == "ok"));
    Report {
        version: env!("CARGO_PKG_VERSION"),
        platform: if host.is_wsl() {
            "windows-wsl"
        } else {
            "linux"
        },
        can_build,
        can_deploy,
        checks,
    }
}

pub fn print(report: &Report) {
    for c in &report.checks {
        eprintln!(
            "  [{}] {:<38} {}",
            ui::status_tag(c.status),
            c.label,
            c.detail
        );
    }
    let fixes: Vec<&String> = {
        let mut seen = Vec::new();
        for f in report
            .checks
            .iter()
            .filter(|c| c.status != "ok")
            .filter_map(|c| c.fix.as_ref())
        {
            if !seen.contains(&f) {
                seen.push(f);
            }
        }
        seen
    };
    eprintln!();
    if report.can_deploy {
        ui::ok("Ready: `irisphone run` builds and installs on your iPhone.");
    } else if report.can_build {
        ui::ok("Ready to build. To install on the iPhone:");
        for f in fixes {
            ui::info(&format!("- {f}"));
        }
    } else {
        ui::warn("Not ready yet. To fix:");
        for f in fixes {
            ui::info(&format!("- {f}"));
        }
    }
}
