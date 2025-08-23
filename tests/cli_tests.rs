use std::process::Command;
use std::path::PathBuf;

fn cargo_run(json: &str, extra: &[&str]) -> (String, String, i32) {
    let mut cmd = Command::new(env!("CARGO"));
    let mut args = vec!["run", "--quiet", "--" ];
    args.extend_from_slice(extra);
    if !json.is_empty() { args.push(json); }
    cmd.args(&args);
    let output = cmd.output().expect("failed to run binary");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.code().unwrap_or(-1))
}

fn fixture(name: &str) -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/grids");
    p.push(name);
    p.to_string_lossy().into_owned()
}

#[test]
fn run_easy_grid_default_color() {
    let (out, err, code) = cargo_run(&fixture("easy_grid.json"), &[]);
    assert_eq!(code, 0, "stderr: {err}");
    assert!(out.contains("\x1b["), "expected colored output");
}

#[test]
fn run_easy_grid_no_color_flag() {
    let (out, err, code) = cargo_run(&fixture("easy_grid.json"), &["--no-color"]);
    assert_eq!(code, 0, "stderr: {err}");
    assert!(!out.contains("\x1b["), "expected no ANSI escapes");
}

#[test]
fn run_unknown_flag_errors() {
    let (out, err, code) = cargo_run(&fixture("easy_grid.json"), &["--definitely-nope"]);
    assert_ne!(code, 0);
    assert!(err.contains("Unknown flag"));
    assert!(out.is_empty());
}

// Added tests
#[test]
fn run_no_arguments_shows_usage() {
    let (out, err, code) = cargo_run("", &[]);
    assert_ne!(code, 0);
    assert!(err.contains("Usage:"));
    assert!(out.is_empty());
}

#[test]
fn run_multiple_paths_errors() {
    let (out, err, code) = cargo_run(&fixture("easy_grid.json"), &[&fixture("medium_grid.json")]);
    assert_ne!(code, 0);
    assert!(err.contains("Expected exactly one JSON path"));
    assert!(out.is_empty());
}

#[test]
fn run_unsolvable_exits_2() {
    let (out, err, code) = cargo_run(&fixture("unsolvable_grid.json"), &[]);
    assert_eq!(code, 2);
    assert!(err.contains("No solution"));
    assert!(out.is_empty());
}

#[test]
fn run_alt_no_color_flags() {
    for flag in ["--no-colors", "-nc"] { // already tested --no-color
        let (out, err, code) = cargo_run(&fixture("easy_grid.json"), &[flag]);
        assert_eq!(code, 0, "stderr: {err}");
        assert!(!out.contains("\x1b["));
    }
}

#[test]
fn color_and_plain_layout_match() {
    let (colored, _, code1) = cargo_run(&fixture("easy_grid.json"), &[]);
    assert_eq!(code1, 0);
    let (plain, _, code2) = cargo_run(&fixture("easy_grid.json"), &["--no-color"]);
    assert_eq!(code2, 0);
    // strip ANSI from colored
    let stripped = ansi_strip(&colored);
    assert_eq!(normalize_ws(&stripped), normalize_ws(&plain));
}

fn ansi_strip(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' { // skip until 'm'
            while let Some(n) = chars.next() { if n == 'm' { break; } }
        } else { out.push(c); }
    }
    out
}

fn normalize_ws(s: &str) -> String { s.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n") }
