# pips_solver

NYT pips puzzle solver in Rust. Loads a JSON description of rule regions and a multiset of dominoes, then performs a backtracking search with forward checking, outputting an ASCII board.

This project is just meant as a way for me to learn rust. Don't put too much trust in the code.

## Puzzle Concept
Each coordinate cell must be covered by exactly one half of a domino. A domino contributes its two pip values to the two cells it spans. Regions (arbitrary sets of coordinates) have textual rules restricting the pip values:

Rule syntax:
- `=` : all pips in the region must be identical.
- `<N` : sum of pips strictly less than `N`.
- `>N` : sum of pips strictly greater than `N`.
- `N`  : sum of pips exactly `N`.
- `x`  : unconstrained region.
- Unrecognized tokens are treated as unconstrained (reported internally as `Unknown`).

**Coordinates are relative to the lower-left corner of the puzzle's bounding box, even counting cells that are not actually included of the puzzle grid. This ensures the printed solution correctly matches the puzzle's shape. The bottom left cell should be (1,1)**

## JSON Format
```json
{
  "grid": [
    { "rule": "=", "coords": [[x,y], [x2,y2], ...] },
    { "rule": "6", "coords": [[x3,y3]] },
    { "rule": ">2", "coords": [[x4,y4], [x5,y5], ...] },
  ],
  "dominoes": [ [a,b], [c,d], ... ]
}
```

## Features
- Rule normalization and incremental region feasibility checking.
- Backtracking solver with pruning (forward checking of affected regions after each placement).
- Tracks domino identity per cell for color grouping.
- ASCII board rendering with optional ANSI color (distinct color per domino, cycling palette).
- CLI with flag to disable color.

## Build
Requires stable Rust (2024 edition set in `Cargo.toml`).
```bash
cargo build --release
```
Binary will be at `target/release/pips_solver`.

## Run
```bash
cargo run -- <puzzle.json>
```
Disable color:
```bash
cargo run -- --no-color <puzzle.json>
# or
cargo run -- -nc <puzzle.json>
```
Exit codes:
- 0 success (solution printed)
- 1 usage / argument / I/O / parse error
- 2 unsolvable puzzle

## Example
```
$ cargo run -- tests/grids/easy_grid.json
0 0 1
3 3 3
0 4
```

## Testing
```bash
cargo test
```
