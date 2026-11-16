//! `irisphone mcp`: Model Context Protocol server over stdio, so Claude Code (or any MCP client)
//! can create, build, install and debug iPhone apps by itself.
//!
//! Newline-delimited JSON-RPC 2.0 on stdin/stdout. Nothing else may be written to stdout.

use crate::config::Config;
use crate::host::Host;
use crate::ops::{self, BuildOpts};
use crate::{doctor, project};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

const PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "IrisPhone builds and installs iOS apps from Windows/Linux without a Mac \
(Swift + iOS SDK + xtool in WSL/Linux, Apple ID signing, USB install). \
Workflow: irisphone_doctor if unsure the setup works; irisphone_new_app to start; after each code change \
irisphone_build and fix all errors; irisphone_run to install on the iPhone; irisphone_logs to read print() output. \
No simulator exists: runtime testing happens on the real device. Read the project's CLAUDE.md for the rules \
(no Xcode-only features: no #Preview, storyboards, asset catalogs, SwiftData macros). \
Logging in (irisphone login) needs a password and a 2FA code: ask the user to run it in their own terminal.";

fn tools() -> Value {
    let path = json!({"type": "string", "description": "App project directory (contains xtool.yml). Defaults to the current directory."});
    json!([
        {
            "name": "irisphone_doctor",
            "description": "Check the whole toolchain (WSL, Swift, iOS SDK, xtool, Apple USB service, Apple ID, connected iPhone). Returns each check with a fix. can_build / can_deploy summarise readiness.",
            "inputSchema": {"type": "object", "properties": {}},
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "irisphone_new_app",
            "description": "Create a new iOS app project from a template: 'swiftui' (native, default) or 'webview' (HTML/CSS/JS in a native shell with a JS<->Swift bridge). Creates <directory>/<name> with Package.swift, xtool.yml, Info.plist, CLAUDE.md and .mcp.json.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "App name: letters and digits, starting with a letter (e.g. MyApp)."},
                    "template": {"type": "string", "enum": ["swiftui", "webview"]},
                    "directory": {"type": "string", "description": "Parent directory. Defaults to the current directory."},
                    "bundle_id": {"type": "string", "description": "Bundle ID, unique at Apple. Defaults to <configured prefix>.<name lowercased>."}
                },
                "required": ["name"]
            }
        },
        {
            "name": "irisphone_project_info",
            "description": "Read a project's name (also the process name), bundle ID, Info.plist and icon paths.",
            "inputSchema": {"type": "object", "properties": {"path": path}},
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "irisphone_build",
            "description": "Compile the app for iOS (no iPhone needed). Returns success and the compiler output; on failure, the errors with their file:line in the project. Run it after every code change.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": path,
                    "release": {"type": "boolean", "description": "Release configuration (default: debug)."},
                    "full_log": {"type": "boolean", "description": "Return the whole log instead of a summary."}
                }
            }
        },
        {
            "name": "irisphone_run",
            "description": "Build, sign with the user's Apple ID, install and launch the app on the iPhone connected by USB. Takes 1-5 minutes. Needs a logged-in Apple ID and a trusted, unlocked iPhone.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": path,
                    "release": {"type": "boolean"},
                    "udid": {"type": "string", "description": "Target a specific device when several are connected."}
                }
            }
        },
        {
            "name": "irisphone_logs",
            "description": "Capture the iPhone's system log for the app's process for a few seconds (print(), os.Logger, crash messages). Launch the app first (irisphone_run or irisphone_launch), then call this while reproducing the issue.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": path,
                    "process": {"type": "string", "description": "Process name; defaults to the project's product name."},
                    "seconds": {"type": "integer", "minimum": 1, "maximum": 120, "description": "How long to listen (default 10)."},
                    "launch": {"type": "boolean", "description": "Launch the app first, then capture (default false)."}
                }
            },
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "irisphone_launch",
            "description": "Launch the installed app on the iPhone.",
            "inputSchema": {"type": "object", "properties": {"path": path, "bundle_id": {"type": "string"}}}
        },
        {
            "name": "irisphone_uninstall",
            "description": "Remove the app from the iPhone (frees one of the 3 slots of a free Apple ID).",
            "inputSchema": {"type": "object", "properties": {"path": path, "bundle_id": {"type": "string"}}},
            "annotations": {"destructiveHint": true}
        },
        {
            "name": "irisphone_devices",
            "description": "List iPhones/iPads connected by USB.",
            "inputSchema": {"type": "object", "properties": {}},
            "annotations": {"readOnlyHint": true}
        }
    ])
}

pub fn serve() -> Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                write(
                    &mut stdout,
                    &json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": e.to_string()}}),
                )?;
                continue;
            }
        };
        let batch = match msg {
            Value::Array(a) => a,
            m => vec![m],
        };
        for m in batch {
            if let Some(resp) = handle(&m) {
                write(&mut stdout, &resp)?;
            }
        }
    }
    Ok(())
}

fn write(out: &mut impl Write, v: &Value) -> Result<()> {
    writeln!(out, "{}", serde_json::to_string(v)?)?;
    out.flush()?;
    Ok(())
}

fn handle(msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(json!({}));
    // Notifications (no id) never get a response.
    let id = id?;
    let result = match method {
        "initialize" => {
            let asked = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("");
            let version = if PROTOCOL_VERSIONS.contains(&asked) {
                asked
            } else {
                PROTOCOL_VERSIONS[0]
            };
            Ok(json!({
                "protocolVersion": version,
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "irisphone", "title": "IrisPhone", "version": env!("CARGO_PKG_VERSION")},
                "instructions": INSTRUCTIONS
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools()})),
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            Ok(match call(&name, &args) {
                Ok((text, ok)) => {
                    json!({"content": [{"type": "text", "text": text}], "isError": !ok})
                }
                Err(e) => {
                    json!({"content": [{"type": "text", "text": format!("Error: {e:#}")}], "isError": true})
                }
            })
        }
        "resources/list" => Ok(json!({"resources": []})),
        "prompts/list" => Ok(json!({"prompts": []})),
        _ => Err((-32601, format!("method not found: {method}"))),
    };
    Some(match result {
        Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
        Err((code, message)) => {
            json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
        }
    })
}

fn arg_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
}

fn arg_bool(args: &Value, key: &str) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn project_dir(args: &Value) -> Result<PathBuf> {
    project::find(arg_str(args, "path").map(std::path::Path::new))
}

/// Returns (text, success).
fn call(name: &str, args: &Value) -> Result<(String, bool)> {
    let cfg = Config::load();
    let host = Host::detect(&cfg);
    match name {
        "irisphone_doctor" => {
            let report = doctor::run(&host);
            Ok((serde_json::to_string_pretty(&report)?, true))
        }
        "irisphone_new_app" => {
            let name = arg_str(args, "name").context("'name' is required")?;
            let template = project::template(arg_str(args, "template").unwrap_or("swiftui"))?;
            let parent = match arg_str(args, "directory") {
                Some(d) => PathBuf::from(d),
                None => std::env::current_dir()?,
            };
            let bundle = match arg_str(args, "bundle_id") {
                Some(b) => b.to_string(),
                None => format!("{}.{}", cfg.bundle_prefix_or_default(), name.to_lowercase()),
            };
            if !crate::config::is_valid_bundle_id(&bundle) {
                bail!("invalid bundle ID '{bundle}'");
            }
            let dir = project::create(&parent, name, template, &bundle)?;
            Ok((
                format!(
                    "Created {} ({} template, bundle ID {bundle}).\nNext: edit Sources/{name}/, then irisphone_build, then irisphone_run.\nRead {} for the rules.",
                    dir.display(),
                    template.name,
                    dir.join("CLAUDE.md").display()
                ),
                true,
            ))
        }
        "irisphone_project_info" => {
            let dir = project_dir(args)?;
            Ok((serde_json::to_string_pretty(&project::info(&dir)?)?, true))
        }
        "irisphone_build" => {
            let dir = project_dir(args)?;
            let opts = BuildOpts {
                release: arg_bool(args, "release"),
                ipa: false,
            };
            let c = ops::build(&host, &dir, opts, false, Some(Duration::from_secs(20 * 60)))?;
            let ok = c.ok();
            let log = if arg_bool(args, "full_log") {
                c.output.clone()
            } else {
                ops::summarize(&c.output, ok)
            };
            let head = if c.timed_out {
                "BUILD TIMED OUT".to_string()
            } else if ok {
                "BUILD SUCCEEDED".to_string()
            } else {
                format!("BUILD FAILED (exit code {})", c.code)
            };
            Ok((format!("{head}\n\n{log}"), ok))
        }
        "irisphone_run" => {
            let dir = project_dir(args)?;
            let opts = BuildOpts {
                release: arg_bool(args, "release"),
                ipa: false,
            };
            let c = ops::run_captured(
                &host,
                &dir,
                opts,
                arg_str(args, "udid"),
                Duration::from_secs(20 * 60),
            )?;
            let ok = c.ok();
            let mut text = format!(
                "{}\n\n{}",
                if ok {
                    "INSTALLED AND LAUNCHED"
                } else if c.timed_out {
                    "TIMED OUT"
                } else {
                    "FAILED"
                },
                ops::summarize(&c.output, ok)
            );
            if !ok {
                text.push_str(&hints(&c.output));
            }
            Ok((text, ok))
        }
        "irisphone_logs" => {
            let process = match arg_str(args, "process") {
                Some(p) => p.to_string(),
                None => ops::process_name(&project_dir(args)?)?,
            };
            let seconds = args
                .get("seconds")
                .and_then(Value::as_u64)
                .unwrap_or(10)
                .clamp(1, 120);
            ops::ensure_usb(&host)?;
            let mut pre = String::new();
            if arg_bool(args, "launch") {
                let bundle = ops::bundle_id(&project_dir(args)?)?;
                let c = host.capture(&["launch", &bundle], Some(Duration::from_secs(60)))?;
                pre = format!("launch: {}\n", if c.ok() { "ok".into() } else { c.output });
            }
            let secs = seconds.to_string();
            let c = host.capture(
                &["logs", &process, &secs],
                Some(Duration::from_secs(seconds + 30)),
            )?;
            let lines: Vec<&str> = c.output.lines().collect();
            let shown = if lines.len() > 400 {
                lines[lines.len() - 400..].join("\n")
            } else {
                c.output.clone()
            };
            let body = if shown.trim().is_empty() {
                format!("No log lines from process '{process}' in {seconds}s. Is the app running in the foreground?")
            } else {
                shown
            };
            Ok((format!("{pre}{body}"), true))
        }
        "irisphone_launch" | "irisphone_uninstall" => {
            let bundle = match arg_str(args, "bundle_id") {
                Some(b) => b.to_string(),
                None => ops::bundle_id(&project_dir(args)?)?,
            };
            ops::ensure_usb(&host)?;
            let verb = if name == "irisphone_launch" {
                "launch"
            } else {
                "uninstall"
            };
            let c = host.capture(&[verb, &bundle], Some(Duration::from_secs(120)))?;
            let ok = c.ok();
            Ok((
                if c.output.trim().is_empty() {
                    format!("{verb} {bundle}: {}", if ok { "done" } else { "failed" })
                } else {
                    c.output
                },
                ok,
            ))
        }
        "irisphone_devices" => {
            ops::ensure_usb(&host)?;
            let c = host.capture(&["devices"], Some(Duration::from_secs(60)))?;
            let ok = c.ok();
            Ok((
                if c.output.trim().is_empty() {
                    "No device connected.".into()
                } else {
                    c.output
                },
                ok,
            ))
        }
        _ => bail!("unknown tool '{name}'"),
    }
}

/// Plain-language next steps for the usual install failures.
fn hints(output: &str) -> String {
    let o = output.to_lowercase();
    let mut h = Vec::new();
    if o.contains("not logged in") || o.contains("auth login") || o.contains("unauthorized") {
        h.push("The Apple ID is not logged in: ask the user to run `irisphone login` in their terminal (password + 2FA).");
    }
    if o.contains("no devices") || o.contains("no device") || o.contains("waiting for") {
        h.push("No iPhone found: ask the user to plug it in by cable, unlock it, and tap Trust.");
    }
    if o.contains("developer mode") {
        h.push("Developer Mode is off: iPhone Settings > Privacy & Security > Developer Mode, then restart the iPhone.");
    }
    if o.contains("maximum number")
        || o.contains("app id limit")
        || o.contains("limit") && o.contains("app")
    {
        h.push("Free Apple ID limit reached (3 installed apps / 10 new bundle IDs per week): uninstall another sideloaded app or reuse a bundle ID.");
    }
    if o.contains("untrusted") || o.contains("not trusted") || o.contains("0xe8000067") {
        h.push("The developer profile is not trusted yet: iPhone Settings > General > VPN & Device Management > the Apple ID > Trust.");
    }
    if h.is_empty() {
        String::new()
    } else {
        format!("\n\nLikely fix:\n- {}", h.join("\n- "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_basics() {
        let init = handle(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}})).unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert!(handle(&json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
        let list = handle(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
        assert!(list["result"]["tools"].as_array().unwrap().len() >= 8);
        let bad = handle(&json!({"jsonrpc":"2.0","id":3,"method":"nope"})).unwrap();
        assert_eq!(bad["error"]["code"], -32601);
    }
}
