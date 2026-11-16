//! `irisphone setup`: installs and configures everything, step by step, and can be re-run safely.

use crate::config::{self, Config};
use crate::host::{self, Host};
use crate::{claude, doctor, ui, windows};
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct SetupOpts {
    pub xip: Option<PathBuf>,
    /// Answer yes to every question (non-interactive installs).
    pub yes: bool,
    pub skip_login: bool,
}

pub fn run(cfg: &Config, host: &Host, opts: &SetupOpts) -> Result<()> {
    let ask = |q: &str, default: bool| opts.yes || ui::confirm(q, default);

    if let Host::Wsl { distro } = host {
        ui::step(&format!("WSL ({distro})"));
        let present = windows::wsl_distros().is_some_and(|d| d.iter().any(|x| x == distro));
        if present {
            ui::ok("installed");
        } else {
            ui::info(&format!(
                "IrisPhone runs Swift and the iOS SDK inside WSL, in the {distro} distribution."
            ));
            if !ask(
                &format!("Install WSL with {distro} now? (Windows asks for admin rights)"),
                true,
            ) {
                bail!("WSL is required. Install it with: wsl --install -d {distro}");
            }
            let code = host::run_inherit({
                let mut c = std::process::Command::new("wsl.exe");
                c.args(["--install", "-d", distro]);
                c
            })?;
            let present = windows::wsl_distros().is_some_and(|d| d.iter().any(|x| x == distro));
            if code != 0 || !present {
                ui::warn("WSL is being installed. If Windows asks you to restart, do it.");
                ui::warn(&format!("Once {distro} has opened and you created your Linux user, run `irisphone setup` again."));
                return Ok(());
            }
            ui::ok("installed");
        }

        ui::step("Apple Devices (iPhone USB driver, from the Microsoft Store)");
        let installed = windows::apple_devices().is_some()
            || (windows::install_apple_devices()? && windows::apple_devices().is_some());
        if installed {
            ui::ok("installed");
        } else {
            ui::warn("Could not install it automatically: install \"Apple Devices\" from the Microsoft Store, then re-run setup.");
            windows::open_url(&format!(
                "ms-windows-store://pdp/?ProductId={}",
                windows::APPLE_DEVICES_STORE_ID
            ));
        }

        ui::step("WSL networking (lets Linux reach the iPhone service)");
        if windows::wsl_mirrored() {
            ui::ok("mirrored networking already on");
        } else {
            windows::set_wsl_mirrored()?;
            ui::ok("set networkingMode=mirrored in %USERPROFILE%\\.wslconfig");
            ui::info("Restarting WSL to apply it (this stops anything running in WSL)...");
            let _ = std::process::Command::new("wsl.exe")
                .arg("--shutdown")
                .status();
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    ui::step(&format!("Linux packages + xtool {}", config::XTOOL_VERSION));
    if !host.is_wsl() {
        ui::info("(asks for your sudo password)");
    }
    let code = host::run_inherit(host.root_script(&[config::XTOOL_VERSION.to_string()])?)?;
    if code != 0 {
        bail!("installing the Linux packages failed (exit code {code})");
    }

    ui::step(&format!("Swift {}", config::SWIFT_VERSION));
    if host.run(&["install-swift", config::SWIFT_VERSION])? != 0 {
        bail!("installing Swift failed");
    }

    ui::step("iOS SDK");
    let status = host.capture(&["sdk-status"], Some(Duration::from_secs(60)))?;
    if status.output.contains("is installed") {
        ui::ok("installed");
    } else {
        install_sdk(host, opts)?;
    }

    if host.is_wsl() {
        ui::step("Apple USB service");
        if windows::start_apple_service() {
            ui::ok("running");
        } else {
            ui::warn("not running yet: open the Apple Devices app once (it starts the service).");
        }
    }

    if !opts.skip_login {
        ui::step("Apple ID");
        let auth = host.capture(&["auth-status"], Some(Duration::from_secs(30)))?;
        let logged = auth.ok() && !auth.output.to_lowercase().contains("not logged");
        if logged {
            ui::ok(auth.output.lines().next().unwrap_or("logged in"));
        } else if !opts.yes
            && ui::confirm(
                "Log in with your Apple ID now? (a free account works)",
                true,
            )
        {
            login(cfg, host, None)?;
        } else {
            ui::info("Later: irisphone login");
        }
    }

    ui::step("Claude Code");
    claude::offer_registration(opts.yes)?;

    ui::step("Checking everything");
    let report = doctor::run(host);
    doctor::print(&report);
    if report.can_build {
        eprintln!();
        ui::ok("Next: irisphone new MyApp, then irisphone run inside it.");
    }
    Ok(())
}

fn install_sdk(host: &Host, opts: &SetupOpts) -> Result<()> {
    let xip = match opts.xip.clone().or_else(windows::find_xip) {
        Some(x) => x,
        None => {
            ui::info(
                "The iOS SDK is extracted from Xcode, which Apple distributes as a .xip archive.",
            );
            ui::info("Apple's license doesn't allow sharing it, so you download it yourself (free Apple ID):");
            ui::info(&format!("  {}", windows::XCODE_DOWNLOAD_URL));
            ui::info("Pick the latest stable \"Xcode\" release, download the .xip (about 3 GB) into your Downloads folder.");
            if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                bail!("no Xcode .xip found; re-run with --xip <path> once downloaded");
            }
            if ui::confirm("Open the download page now?", true) {
                windows::open_url(windows::XCODE_DOWNLOAD_URL);
            }
            wait_for_xip()?
        }
    };
    ui::info(&format!(
        "Building the SDK from {} (5-15 minutes)...",
        xip.display()
    ));
    let linux = host.linux_path(&xip)?;
    if host.run(&["sdk-install", &linux])? != 0 {
        bail!("building the iOS SDK failed");
    }
    ui::ok("iOS SDK installed. You can delete the .xip now.");
    Ok(())
}

/// Polls the Downloads folder until a finished Xcode .xip appears (size stable for 10 s).
fn wait_for_xip() -> Result<PathBuf> {
    ui::info(&format!(
        "Waiting for Xcode*.xip in {} ... (Ctrl+C to stop; or re-run later with --xip <path>)",
        windows::downloads_dir().display()
    ));
    let mut last: Option<(PathBuf, u64)> = None;
    loop {
        if let Some(p) = windows::find_xip() {
            let size = file_size(&p);
            if let Some((lp, ls)) = &last {
                if lp == &p && *ls == size && size > 1_000_000_000 {
                    ui::ok(&format!("found {}", p.display()));
                    return Ok(p);
                }
            }
            last = Some((p, size));
        }
        std::thread::sleep(Duration::from_secs(10));
    }
}

fn file_size(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

pub fn login(cfg: &Config, host: &Host, apple_id: Option<&str>) -> Result<()> {
    ui::info("Your password goes straight to Apple through xtool; IrisPhone never stores it.");
    ui::info("Tip: an extra free Apple ID just for development works well.");
    let id = apple_id
        .map(str::to_string)
        .or_else(|| cfg.apple_id.clone());
    let code = match &id {
        Some(i) => host.run(&["login", i])?,
        None => host.run(&["login"])?,
    };
    if code != 0 {
        bail!("login failed");
    }
    if let Some(i) = apple_id {
        let mut c = cfg.clone();
        c.apple_id = Some(i.to_string());
        c.save()?;
    }
    Ok(())
}
