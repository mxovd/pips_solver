mod grid;
use grid::GameGrid;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = match env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("Usage: pips_solver <path-to-grid.json>");
            std::process::exit(1);
        }
    };
    let mut g = GameGrid::from_file(&path)?;
    if let Some(solution) = g.solve() {
        println!("Solved. Filled {} cells.", solution.len());
        // Pretty print sorted by (y,x)
        let mut cells: Vec<_> = solution.into_iter().collect();
        cells.sort_by_key(|((x, y), _)| (*y, *x));
        for ((x, y), v) in cells {
            println!("({x},{y}) = {v}");
        }
    } else {
        println!("No solution found.");
    }
    Ok(())
}
