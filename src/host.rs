//! Where the Linux toolchain runs: inside WSL on Windows, directly on Linux.
//!
//! Everything iOS-related (Swift, the iOS SDK, xtool, libimobiledevice) lives on the Linux side
//! and is driven through `scripts/irisphone.sh`, which is embedded in this binary.

use crate::config::{self, Config};
use anyhow::{bail, Context, Result};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAIN_SCRIPT: &str = include_str!("../scripts/irisphone.sh");
const ROOT_SCRIPT: &str = include_str!("../scripts/root-setup.sh");

#[derive(Debug, Clone)]
pub enum Host {
    Wsl { distro: String },
    Native,
}

/// Output of a captured run: exit code + stdout and stderr interleaved, ANSI codes removed.
pub struct Captured {
    pub code: i32,
    pub output: String,
    pub timed_out: bool,
}

impl Captured {
    pub fn ok(&self) -> bool {
        self.code == 0 && !self.timed_out
    }
}

impl Host {
    pub fn detect(cfg: &Config) -> Host {
        if cfg!(windows) {
            Host::Wsl {
                distro: cfg.distro(),
            }
        } else {
            Host::Native
        }
    }

    pub fn is_wsl(&self) -> bool {
        matches!(self, Host::Wsl { .. })
    }

    /// Converts a local path to the path the Linux side sees (`C:\x` -> `/mnt/c/x` under WSL).
    pub fn linux_path(&self, p: &Path) -> Result<String> {
        let abs = absolute(p)?;
        match self {
            Host::Native => Ok(abs.to_string_lossy().into_owned()),
            Host::Wsl { .. } => windows_to_wsl(&abs.to_string_lossy()),
        }
    }

    /// Command running the main script with `args`, as the default Linux user.
    pub fn script(&self, args: &[String]) -> Result<Command> {
        let script = self.linux_path(&materialize("irisphone.sh", MAIN_SCRIPT)?)?;
        Ok(self.bash(&script, args, false))
    }

    /// Command running the root setup script (apt packages + xtool).
    pub fn root_script(&self, args: &[String]) -> Result<Command> {
        let script = self.linux_path(&materialize("root-setup.sh", ROOT_SCRIPT)?)?;
        Ok(self.bash(&script, args, true))
    }

    fn bash(&self, script: &str, args: &[String], root: bool) -> Command {
        match self {
            Host::Wsl { distro } => {
                let mut c = Command::new("wsl.exe");
                c.args(["-d", distro]);
                if root {
                    c.args(["-u", "root"]);
                }
                c.args(["-e", "bash", script]).args(args);
                c
            }
            Host::Native => {
                let mut c = if root && !is_root() {
                    let mut s = Command::new("sudo");
                    s.arg("bash");
                    s
                } else {
                    Command::new("bash")
                };
                c.arg(script).args(args);
                c
            }
        }
    }

    /// Runs the main script attached to the console (interactive: passwords, progress bars).
    pub fn run(&self, args: &[&str]) -> Result<i32> {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        run_inherit(self.script(&args)?)
    }

    /// Runs the main script and captures its output (for --json and the MCP server).
    pub fn capture(&self, args: &[&str], timeout: Option<Duration>) -> Result<Captured> {
        self.capture_with(
            args,
            CaptureOpts {
                timeout,
                ..Default::default()
            },
        )
    }

    pub fn capture_with(&self, args: &[&str], opts: CaptureOpts) -> Result<Captured> {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        capture_with(self.script(&args)?, opts)
    }
}

pub fn run_inherit(mut cmd: Command) -> Result<i32> {
    let status = cmd
        .status()
        .with_context(|| format!("could not start {:?}", cmd.get_program()))?;
    Ok(status.code().unwrap_or(-1))
}

#[derive(Default, Clone)]
pub struct CaptureOpts {
    pub timeout: Option<Duration>,
    /// Also print each line to stderr as it arrives.
    pub echo: bool,
    /// Local project path: Linux build paths in the output are rewritten to it.
    pub project: Option<String>,
}

const BUILD_DIR_MARKER: &str = "@@irisphone-build-dir ";

/// Runs `cmd`, interleaving stdout and stderr line by line, with an optional timeout.
pub fn capture_with(mut cmd: Command, opts: CaptureOpts) -> Result<Captured> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .with_context(|| format!("could not start {:?}", cmd.get_program()))?;
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    // (linux build dir, local project dir), learnt from the marker line printed by the script.
    let mapping: Arc<Mutex<Option<(String, String)>>> = Arc::new(Mutex::new(None));
    let mut readers = Vec::new();
    let out: Box<dyn Read + Send> = Box::new(child.stdout.take().unwrap());
    let err: Box<dyn Read + Send> = Box::new(child.stderr.take().unwrap());
    for stream in [out, err] {
        let lines = Arc::clone(&lines);
        let mapping = Arc::clone(&mapping);
        let opts = opts.clone();
        readers.push(std::thread::spawn(move || {
            let mut r = BufReader::new(stream);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match r.read_until(b'\n', &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let raw = String::from_utf8_lossy(&buf);
                        let mut line = strip_ansi(raw.trim_end_matches(['\n', '\r']))
                            .trim_end()
                            .to_string();
                        if let Some(dir) = line.strip_prefix(BUILD_DIR_MARKER) {
                            if let Some(local) = &opts.project {
                                let local = local.replace('\\', "/");
                                *mapping.lock().unwrap() = Some((dir.trim().to_string(), local));
                            }
                            continue;
                        }
                        if let Some((from, to)) = mapping.lock().unwrap().as_ref() {
                            line = line.replace(from.as_str(), to);
                        }
                        if opts.echo {
                            eprintln!("{line}");
                        }
                        lines.lock().unwrap().push(line);
                    }
                }
            }
        }));
    }
    let start = Instant::now();
    let mut timed_out = false;
    let code = loop {
        if let Some(status) = child.try_wait()? {
            break status.code().unwrap_or(-1);
        }
        if opts.timeout.is_some_and(|t| start.elapsed() > t) {
            let _ = child.kill();
            timed_out = true;
            break child.wait().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    for r in readers {
        let _ = r.join();
    }
    let output = lines.lock().unwrap().join("\n");
    Ok(Captured {
        code,
        output,
        timed_out,
    })
}

/// Writes an embedded script to the data directory (only when its content changed).
fn materialize(name: &str, content: &str) -> Result<PathBuf> {
    let dir = config::data_dir().join("scripts");
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join(name);
    let content = content.replace("\r\n", "\n");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(content.as_str()) {
        std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(path)
}

fn absolute(p: &Path) -> Result<PathBuf> {
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()?.join(p)
    };
    // Normalise `.` and `..` without touching the disk (canonicalize adds \\?\ on Windows).
    let mut out = PathBuf::new();
    for comp in abs.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c.as_os_str()),
        }
    }
    Ok(out)
}

/// `C:\Users\me\My App` -> `/mnt/c/Users/me/My App`.
pub fn windows_to_wsl(p: &str) -> Result<String> {
    let p = p.strip_prefix(r"\\?\").unwrap_or(p);
    let bytes = p.as_bytes();
    if bytes.len() < 2 || bytes[1] != b':' || !bytes[0].is_ascii_alphabetic() {
        bail!("'{p}' is not on a local drive; WSL can only reach paths like C:\\...");
    }
    let drive = (bytes[0] as char).to_ascii_lowercase();
    let rest = p[2..].replace('\\', "/");
    Ok(format!("/mnt/{drive}{rest}"))
}

fn is_root() -> bool {
    #[cfg(unix)]
    {
        std::env::var("USER").map(|u| u == "root").unwrap_or(false)
            || Command::new("id")
                .arg("-u")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
                .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        false
    }
}

pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                // CSI: parameters until a final byte in @..~
                for n in chars.by_ref() {
                    if ('@'..='~').contains(&n) {
                        break;
                    }
                }
            } else {
                chars.next();
            }
        } else if c == '\r' {
            // progress bars redraw with \r: keep only what follows the last one
            out.clear();
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wsl_paths() {
        assert_eq!(
            windows_to_wsl(r"C:\Users\me\My App").unwrap(),
            "/mnt/c/Users/me/My App"
        );
        assert_eq!(windows_to_wsl(r"\\?\D:\x").unwrap(), "/mnt/d/x");
        assert!(windows_to_wsl(r"\\server\share").is_err());
    }

    #[test]
    fn ansi() {
        assert_eq!(strip_ansi("\u{1b}[1;31merror\u{1b}[0m: x"), "error: x");
        assert_eq!(strip_ansi("10%\r50%\r100% done"), "100% done");
    }
}
