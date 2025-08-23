mod grid;
use grid::GameGrid;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Usage: pips_solver [--no-color|-n|-nc|--no-colors] <path-to-grid.json>");
        std::process::exit(1);
    }
    let mut color = true;
    // Collect non-flag args
    let mut positional: Vec<String> = Vec::new();
    for a in args.into_iter() {
        match a.as_str() {
            "--no-color" | "--no-colors" | "-nc" => color = false,
            _ if a.starts_with('-') => {
                eprintln!("Unknown flag: {a}");
                std::process::exit(1);
            }
            _ => positional.push(a),
        }
    }
    if positional.len() != 1 {
        eprintln!("Expected exactly one JSON path. Got {}.", positional.len());
        std::process::exit(1);
    }
    let path = &positional[0];
    let mut g = GameGrid::from_file(path)?;
    if g.solve().is_some() {
        print!("{}", g.ascii_board(color));
    } else {
        eprintln!("No solution found.");
        std::process::exit(2);
    }
    Ok(())
}
