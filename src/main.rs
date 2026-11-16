//! IrisPhone: build and install iPhone apps from Windows (or Linux), no Mac required.

mod claude;
mod config;
mod doctor;
mod host;
mod mcp;
mod ops;
mod project;
mod setup;
mod ui;
mod windows;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use config::Config;
use host::Host;
use ops::BuildOpts;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "irisphone",
    version,
    about = "Build and install iPhone apps from Windows or Linux. No Mac required.",
    after_help = "Getting started:\n  irisphone setup        install everything (once)\n  irisphone new MyApp    create an app\n  cd MyApp\n  irisphone run          build + install on the iPhone plugged in by USB\n\nWith Claude Code: run `claude` inside the app folder and describe what you want."
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Install and configure everything (WSL, Apple USB driver, Swift, iOS SDK, xtool). Safe to re-run.
    Setup {
        /// Path to the Xcode .xip (otherwise looked up in the current and Downloads folders)
        #[arg(long)]
        xip: Option<PathBuf>,
        /// Don't ask questions: answer yes to everything
        #[arg(long, short)]
        yes: bool,
        /// Skip the Apple ID login step
        #[arg(long)]
        skip_login: bool,
    },
    /// Check every part of the toolchain and the connected iPhone
    Doctor {
        /// Machine-readable output
        #[arg(long)]
        json: bool,
    },
    /// Create a new app project
    New {
        /// App name: letters and digits, starting with a letter (e.g. MyApp)
        name: Option<String>,
        /// Template: swiftui (native) or webview (HTML/CSS/JS + native shell)
        #[arg(long, short, default_value = "swiftui")]
        template: String,
        /// Parent directory (default: current directory)
        #[arg(long, short)]
        dir: Option<PathBuf>,
        /// Bundle ID (default: <bundle_prefix>.<name>)
        #[arg(long)]
        bundle_id: Option<String>,
    },
    /// List project templates
    Templates,
    /// Compile the app (no iPhone needed)
    Build {
        /// Project directory (default: current directory)
        path: Option<PathBuf>,
        #[arg(long)]
        release: bool,
        /// Produce a .ipa (signed with your Apple ID) in xtool/
        #[arg(long)]
        ipa: bool,
    },
    /// Build, install and launch the app on the iPhone connected by USB
    #[command(visible_alias = "deploy")]
    Run {
        path: Option<PathBuf>,
        #[arg(long)]
        release: bool,
        /// Device to target when several are connected
        #[arg(long)]
        udid: Option<String>,
    },
    /// Stream the app's logs from the iPhone (print, os.Logger, crashes)
    Logs {
        path: Option<PathBuf>,
        /// Process name (default: the project's product name)
        #[arg(long, short)]
        process: Option<String>,
        /// Stop after N seconds (default: follow until Ctrl+C)
        #[arg(long, short, default_value_t = 0)]
        seconds: u64,
    },
    /// Launch the installed app on the iPhone
    Launch {
        path: Option<PathBuf>,
        #[arg(long)]
        bundle_id: Option<String>,
    },
    /// Remove the app from the iPhone
    Uninstall {
        path: Option<PathBuf>,
        #[arg(long)]
        bundle_id: Option<String>,
    },
    /// List iPhones connected by USB
    Devices,
    /// Pair the iPhone with this computer ("Trust This Computer")
    Pair,
    /// Log in with your Apple ID (free or paid) to sign apps
    Login {
        /// Apple ID e-mail (remembered for next time)
        #[arg(long)]
        apple_id: Option<String>,
    },
    /// Log out of the Apple ID
    Logout,
    /// Show or change settings (bundle_prefix, distro, apple_id)
    Config {
        /// Key to show or set; no key lists everything
        key: Option<String>,
        /// New value ("" to reset)
        value: Option<String>,
    },
    /// Connect IrisPhone to Claude Code (registers the MCP server for all projects)
    Claude {
        /// Remove it from Claude Code instead
        #[arg(long)]
        remove: bool,
    },
    /// Run the MCP server on stdio (used by Claude Code and other MCP clients)
    Mcp,
}

fn main() {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            ui::fail(&format!("{e:#}"));
            std::process::exit(1);
        }
    }
}

fn run(cli: Cli) -> Result<i32> {
    let mut cfg = Config::load();
    let host = Host::detect(&cfg);
    match cli.command {
        Cmd::Setup {
            xip,
            yes,
            skip_login,
        } => {
            setup::run(
                &cfg,
                &host,
                &setup::SetupOpts {
                    xip,
                    yes,
                    skip_login,
                },
            )?;
        }
        Cmd::Doctor { json } => {
            let report = doctor::run(&host);
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                doctor::print(&report);
            }
            return Ok(if report.can_build { 0 } else { 1 });
        }
        Cmd::New {
            name,
            template,
            dir,
            bundle_id,
        } => {
            let name = match name.or_else(|| ui::prompt("App name (e.g. MyApp):")) {
                Some(n) => n,
                None => bail!("give the app a name: irisphone new MyApp"),
            };
            project::validate_name(&name)?;
            let template = project::template(&template)?;
            if bundle_id.is_none() && cfg.bundle_prefix.is_none() {
                let guess = cfg.bundle_prefix_or_default();
                ui::info("Apps need a unique bundle ID, usually your reversed domain or name: com.yourname.myapp.");
                let prefix = ui::prompt(&format!("Bundle ID prefix [{guess}]:")).unwrap_or(guess);
                cfg.set("bundle_prefix", Some(prefix))?;
                cfg.save()?;
            }
            let bundle = bundle_id.unwrap_or_else(|| {
                format!("{}.{}", cfg.bundle_prefix_or_default(), name.to_lowercase())
            });
            if !config::is_valid_bundle_id(&bundle) {
                bail!("invalid bundle ID '{bundle}'");
            }
            let parent = match dir {
                Some(d) => d,
                None => std::env::current_dir()?,
            };
            let path = project::create(&parent, &name, template, &bundle)?;
            ui::ok(&format!(
                "created {} ({} template, bundle ID {bundle})",
                path.display(),
                template.name
            ));
            ui::info("Next:");
            ui::info(&format!("  cd {}", path.display()));
            ui::info("  irisphone run     # build + install on the iPhone");
            ui::info("  claude            # or let Claude Code write the app with you");
        }
        Cmd::Templates => {
            for t in project::TEMPLATES {
                println!("{:<10} {}", t.name, t.description);
            }
        }
        Cmd::Build { path, release, ipa } => {
            let dir = project::find(path.as_deref())?;
            ui::step(&format!("Building {}", dir.display()));
            let c = ops::build(&host, &dir, BuildOpts { release, ipa }, true, None)?;
            if !c.ok() {
                ui::fail("build failed");
                return Ok(1);
            }
            ui::ok("build succeeded");
        }
        Cmd::Run {
            path,
            release,
            udid,
        } => {
            let dir = project::find(path.as_deref())?;
            ui::step(&format!("Building and installing {}", dir.display()));
            ui::info("(iPhone plugged in, unlocked, and this computer trusted)");
            let code = ops::run_interactive(
                &host,
                &dir,
                BuildOpts {
                    release,
                    ipa: false,
                },
                udid.as_deref(),
            )?;
            if code != 0 {
                ui::fail(&format!(
                    "failed (exit code {code}). `irisphone doctor` shows what's missing."
                ));
                return Ok(code);
            }
            ui::ok("installed and launched.");
            ui::info("First install only, on the iPhone:");
            ui::info("  Settings > General > VPN & Device Management > your Apple ID > Trust");
            ui::info("  Settings > Privacy & Security > Developer Mode > On (restarts the phone)");
        }
        Cmd::Logs {
            path,
            process,
            seconds,
        } => {
            let process = match process {
                Some(p) => p,
                None => ops::process_name(&project::find(path.as_deref())?)?,
            };
            ops::ensure_usb(&host)?;
            ui::step(&format!("Logs of '{process}' (Ctrl+C to stop)"));
            return host.run(&["logs", &process, &seconds.to_string()]);
        }
        Cmd::Launch { path, bundle_id } => {
            let b = match bundle_id {
                Some(b) => b,
                None => ops::bundle_id(&project::find(path.as_deref())?)?,
            };
            ops::ensure_usb(&host)?;
            return host.run(&["launch", &b]);
        }
        Cmd::Uninstall { path, bundle_id } => {
            let b = match bundle_id {
                Some(b) => b,
                None => ops::bundle_id(&project::find(path.as_deref())?)?,
            };
            ops::ensure_usb(&host)?;
            return host.run(&["uninstall", &b]);
        }
        Cmd::Devices => {
            ops::ensure_usb(&host)?;
            return host.run(&["devices"]);
        }
        Cmd::Pair => {
            ops::ensure_usb(&host)?;
            return host.run(&["pair"]);
        }
        Cmd::Login { apple_id } => setup::login(&cfg, &host, apple_id.as_deref())?,
        Cmd::Logout => return host.run(&["logout"]),
        Cmd::Config { key, value } => match (key, value) {
            (None, _) => {
                eprintln!("# {}", config::config_file().display());
                for (k, help) in config::KEYS {
                    let v = cfg.get(k)?.unwrap_or_default();
                    println!("{k:<14} = {v:<28} # {help}");
                }
            }
            (Some(k), None) => println!("{}", cfg.get(&k)?.unwrap_or_default()),
            (Some(k), Some(v)) => {
                cfg.set(&k, Some(v))?;
                cfg.save()?;
                ui::ok(&format!("{k} = {}", cfg.get(&k)?.unwrap_or_default()));
            }
        },
        Cmd::Claude { remove } => {
            if remove {
                claude::unregister()?;
                ui::ok("removed from Claude Code");
            } else {
                claude::register()?;
                ui::ok("Claude Code can now use IrisPhone in every project (check with `claude mcp list`).");
            }
        }
        Cmd::Mcp => mcp::serve().context("MCP server")?,
    }
    Ok(0)
}
