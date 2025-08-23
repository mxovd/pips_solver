mod grid;
use grid::GameGrid;
use std::env;

#[derive(Debug, PartialEq)]
pub enum CliError {
    Usage,
    UnknownFlag(String),
    WrongArity(usize),
    Other(String),
    Unsolvable,
}

/// Core CLI logic extracted for unit testing. Accepts the already-split argument list (no program name).
pub fn run_cli(args: &[String]) -> Result<String, CliError> {
    if args.is_empty() { return Err(CliError::Usage); }
    let mut color = true;
    let mut positional: Vec<String> = Vec::new();
    for a in args.iter() {
        match a.as_str() {
            "--no-color" | "--no-colors" | "-nc" => color = false,
            _ if a.starts_with('-') => return Err(CliError::UnknownFlag(a.clone())),
            _ => positional.push(a.clone()),
        }
    }
    if positional.len() != 1 { return Err(CliError::WrongArity(positional.len())); }
    let path = &positional[0];
    let mut g = GameGrid::from_file(path).map_err(|e| CliError::Other(e.to_string()))?;
    if g.solve().is_some() {
        Ok(g.ascii_board(color))
    } else {
        Err(CliError::Unsolvable)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match run_cli(&args) {
        Ok(out) => { print!("{out}"); Ok(()) }
        Err(err) => {
            match &err {
                CliError::Usage => eprintln!("Usage: pips_solver [--no-color|-n|-nc|--no-colors] <path-to-grid.json>"),
                CliError::UnknownFlag(f) => eprintln!("Unknown flag: {f}"),
                CliError::WrongArity(n) => eprintln!("Expected exactly one JSON path. Got {n}."),
                CliError::Other(msg) => eprintln!("{msg}"),
                CliError::Unsolvable => { eprintln!("No solution found."); std::process::exit(2); }
            }
            // map all but Unsolvable to exit code 1
            if !matches!(err, CliError::Unsolvable) { std::process::exit(1); }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(name: &str) -> String {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/grids"); p.push(name); p.to_string_lossy().into_owned()
    }

    #[test]
    fn cli_usage_branch() { assert_eq!(run_cli(&[]), Err(CliError::Usage)); }

    #[test]
    fn cli_unknown_flag_branch() { assert_eq!(run_cli(&["--weird".into(), fixture("easy_grid.json")]), Err(CliError::UnknownFlag("--weird".into()))); }

    #[test]
    fn cli_wrong_arity_branch() { assert_eq!(run_cli(&[fixture("easy_grid.json"), fixture("medium_grid.json")]), Err(CliError::WrongArity(2))); }

    #[test]
    fn cli_unsolvable_branch() {
        let res = run_cli(&[fixture("unsolvable_grid.json")]);
        assert_eq!(res, Err(CliError::Unsolvable));
    }

    #[test]
    fn cli_success_color_and_no_color() {
        let out_color = run_cli(&[fixture("easy_grid.json")]).expect("should solve");
        assert!(out_color.contains("\x1b["));
        let out_plain = run_cli(&["--no-color".into(), fixture("easy_grid.json")]).expect("should solve");
        assert!(!out_plain.contains("\x1b["));
    }
}
