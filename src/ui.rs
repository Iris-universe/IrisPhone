//! Terminal output helpers.

use std::io::{IsTerminal, Write};

fn color() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stderr().is_terminal()
}

fn paint(code: &str, s: &str) -> String {
    if color() {
        format!("\u{1b}[{code}m{s}\u{1b}[0m")
    } else {
        s.to_string()
    }
}

pub fn step(msg: &str) {
    eprintln!("{} {}", paint("1;36", "==>"), paint("1", msg));
}

pub fn ok(msg: &str) {
    eprintln!("    {} {msg}", paint("32", "ok"));
}

pub fn warn(msg: &str) {
    eprintln!("    {} {msg}", paint("33", "!!"));
}

pub fn fail(msg: &str) {
    eprintln!("    {} {msg}", paint("31", "xx"));
}

pub fn info(msg: &str) {
    eprintln!("    {msg}");
}

pub fn status_tag(status: &str) -> String {
    match status {
        "ok" => paint("32", "  ok  "),
        "warn" => paint("33", " warn "),
        _ => paint("31", " FAIL "),
    }
}

/// Yes/no prompt; `default` is returned when stdin is not interactive or the answer is empty.
pub fn confirm(question: &str, default: bool) -> bool {
    if !std::io::stdin().is_terminal() {
        return default;
    }
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    eprint!("    {question} {hint} ");
    let _ = std::io::stderr().flush();
    let mut s = String::new();
    if std::io::stdin().read_line(&mut s).is_err() {
        return default;
    }
    match s.trim().to_lowercase().as_str() {
        "" => default,
        a => a.starts_with('y') || a.starts_with('o'),
    }
}

pub fn prompt(question: &str) -> Option<String> {
    if !std::io::stdin().is_terminal() {
        return None;
    }
    eprint!("    {question} ");
    let _ = std::io::stderr().flush();
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).ok()?;
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}
