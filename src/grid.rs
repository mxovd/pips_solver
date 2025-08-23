use std::collections::HashMap;
use std::fs;

use serde::Deserialize;

pub type Coord = (u32, u32);
pub type Domino = (u8, u8);

/// Top-level JSON structure describing a puzzle: rule regions and the available domino set.
#[derive(Deserialize)]
pub struct GridFile {
    pub grid: Vec<GridEntry>,
    pub dominoes: Vec<Domino>,
}

/// One rule region with its textual rule and the list of coordinates it constrains.
#[derive(Deserialize)]
pub struct GridEntry {
    pub rule: String, // parsed later into Rule
    pub coords: Vec<Coord>,
}

/// Normalised internal representation of rule semantics extracted from the JSON string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// All pips in the region must be identical.
    Equal,
    /// Sum of all pips equals the target value.
    Sum(u32),
    /// Sum of all pips strictly greater than value.
    GreaterThan(u32),
    /// Sum of all pips strictly less than value.
    LessThan(u32),
    /// Unconstrained region ("x").
    Any,
    /// Unrecognised / unsupported rule token (treated as unconstrained for now).
    Unknown,
}

impl Rule {
    /// Parse a raw rule string (e.g. "=", "6", ">2", "<6", "x") into a `Rule` value.
    fn parse(s: &str) -> Self {
        if s == "=" {
            return Rule::Equal;
        }
        if s == "x" {
            return Rule::Any;
        }
        if let Some(num) = s.strip_prefix('>') {
            return num.parse().map(Rule::GreaterThan).unwrap_or(Rule::Unknown);
        }
        if let Some(num) = s.strip_prefix('<') {
            return num.parse().map(Rule::LessThan).unwrap_or(Rule::Unknown);
        }
        if let Ok(v) = s.parse() {
            return Rule::Sum(v);
        }
        Rule::Unknown
    }
}

/// Internal evaluation state for a region while solving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegionState {
    Incomplete,
    Satisfied,
    Violated,
}

/// In-memory puzzle grid plus solver state (current assignments & remaining dominoes).
pub struct GameGrid {
    pub entries: Vec<GridEntry>,
    pub rule_index: HashMap<Coord, String>, // original string rules by coord
    pub occupied: HashMap<Coord, u8>,       // now stores pip value per cell
    // Parsed & derived data:
    parsed_rules: Vec<Rule>,                   // parallel to entries
    coord_regions: HashMap<Coord, Vec<usize>>, // coord -> indices of entries
    domino_inventory: Vec<Domino>,             // remaining dominoes
    domino_ids: HashMap<Coord, usize>, // new: track which domino each coord belongs to
}

impl GameGrid {
    /// Load a `GameGrid` from a JSON file on disk.
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let json: String = fs::read_to_string(path)?;
        let parsed: GridFile = serde_json::from_str(&json)?;
        Ok(Self::from_parsed(parsed))
    }

    /// Construct from an already deserialized `GridFile`, building indices used by the solver.
    pub fn from_parsed(parsed: GridFile) -> Self {
        let mut rule_index = HashMap::new();
        let mut parsed_rules = Vec::with_capacity(parsed.grid.len());
        let mut coord_regions: HashMap<Coord, Vec<usize>> = HashMap::new();
        for (i, entry) in parsed.grid.iter().enumerate() {
            let r = Rule::parse(&entry.rule);
            parsed_rules.push(r);
            for &c in &entry.coords {
                rule_index.insert(c, entry.rule.clone());
                coord_regions.entry(c).or_default().push(i);
            }
        }
        GameGrid {
            entries: parsed.grid,
            rule_index,
            occupied: HashMap::new(),
            parsed_rules,
            coord_regions,
            domino_inventory: parsed.dominoes,
            domino_ids: HashMap::new(),
        }
    }

    /// Return orthogonally adjacent coordinates (wrapping subtraction safe for x/y=0).
    pub fn neighbors(coord: Coord) -> impl Iterator<Item = Coord> {
        let (x, y) = coord;
        [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ]
        .into_iter()
    }

    /// Determine current state (Incomplete / Satisfied / Violated) of region `idx`.
    fn region_state(&self, idx: usize) -> RegionState {
        let entry = &self.entries[idx];
        let rule = self.parsed_rules[idx];
        let mut sum = 0u32;
        let mut values: Vec<u8> = Vec::new();
        let mut empty = 0usize;
        for &c in &entry.coords {
            if let Some(&v) = self.occupied.get(&c) {
                sum += v as u32;
                values.push(v);
            } else {
                empty += 1;
            }
        }
        match rule {
            Rule::Any | Rule::Unknown => {
                if empty == 0 {
                    RegionState::Satisfied
                } else {
                    RegionState::Incomplete
                }
            }
            Rule::Equal => {
                if values.len() <= 1 {
                    return if empty == 0 {
                        RegionState::Satisfied
                    } else {
                        RegionState::Incomplete
                    };
                }
                let first = values[0];
                if values.iter().any(|&v| v != first) {
                    RegionState::Violated
                } else if empty == 0 {
                    RegionState::Satisfied
                } else {
                    RegionState::Incomplete
                }
            }
            Rule::Sum(target) => {
                if sum > target {
                    return RegionState::Violated;
                }
                let max_possible = sum + (empty as u32) * 6;
                if max_possible < target {
                    return RegionState::Violated;
                }
                if empty == 0 {
                    if sum == target {
                        RegionState::Satisfied
                    } else {
                        RegionState::Violated
                    }
                } else {
                    RegionState::Incomplete
                }
            }
            Rule::GreaterThan(k) => {
                let max_possible = sum + (empty as u32) * 6;
                if max_possible <= k {
                    return RegionState::Violated;
                }
                if empty == 0 {
                    if sum > k {
                        RegionState::Satisfied
                    } else {
                        RegionState::Violated
                    }
                } else {
                    RegionState::Incomplete
                }
            }
            Rule::LessThan(k) => {
                if sum >= k {
                    return RegionState::Violated;
                }
                if empty == 0 {
                    if sum < k {
                        RegionState::Satisfied
                    } else {
                        RegionState::Violated
                    }
                } else {
                    RegionState::Incomplete
                }
            }
        }
    }

    /// Check that every region touching any of the provided coordinates is still feasible.
    fn affected_regions_feasible(&self, coords: &[Coord]) -> bool {
        let mut seen = std::collections::HashSet::new();
        for &c in coords {
            if let Some(indices) = self.coord_regions.get(&c) {
                for &idx in indices {
                    if seen.insert(idx) {
                        if matches!(self.region_state(idx), RegionState::Violated) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    /// Attempt to solve the puzzle, returning a map of coordinate -> pip value on success.
    pub fn solve(&mut self) -> Option<HashMap<Coord, u8>> {
        if self.backtrack() {
            Some(self.occupied.clone())
        } else {
            None
        }
    }

    /// Recursive backtracking search with forward-checking (region feasibility pruning).
    fn backtrack(&mut self) -> bool {
        // If all cells filled, verify all regions satisfied
        if self.occupied.len() == self.rule_index.len() {
            return self
                .parsed_rules
                .iter()
                .enumerate()
                .all(|(i, _)| matches!(self.region_state(i), RegionState::Satisfied));
        }
        // Choose an empty coordinate (simple heuristic: first)
        let next_coord = self
            .rule_index
            .keys()
            .find(|c| !self.occupied.contains_key(*c))
            .copied()
            .unwrap();
        // Try to pair with an adjacent empty coord
        let partner_candidates: Vec<Coord> = Self::neighbors(next_coord)
            .filter(|c| self.rule_index.contains_key(c) && !self.occupied.contains_key(c))
            .collect();
        if partner_candidates.is_empty() {
            return false;
        }
        // Domino inventory iteration
        for i in 0..self.domino_inventory.len() {
            let domino = self.domino_inventory[i];
            if domino == (255, 255) {
                continue;
            } // sentinel used when consumed
            for &partner in &partner_candidates {
                let orientations: &[(u8,u8)] = if domino.0 == domino.1 { &[(domino.0, domino.1)] } else { &[(domino.0, domino.1), (domino.1, domino.0)] };
                for &(a_val,b_val) in orientations {
                    self.occupied.insert(next_coord, a_val);
                    self.occupied.insert(partner, b_val);
                    self.domino_ids.insert(next_coord, i);
                    self.domino_ids.insert(partner, i);
                    if self.affected_regions_feasible(&[next_coord, partner]) {
                        let saved = domino; self.domino_inventory[i] = (255,255);
                        if self.backtrack() { return true; }
                        self.domino_inventory[i] = saved;
                    }
                    self.occupied.remove(&next_coord);
                    self.occupied.remove(&partner);
                    self.domino_ids.remove(&next_coord);
                    self.domino_ids.remove(&partner);
                }
            }
        }
        false
    }

    /// Render the current grid as ASCII with origin at bottom-left (y increases upward).
    /// Each occupied cell shows its pip value; undefined coordinates are blank.
    pub fn ascii_board_bottom_origin(&self) -> String {
        if self.rule_index.is_empty() { return String::new(); }
        let mut min_x = u32::MAX; let mut min_y = u32::MAX; let mut max_x = 0u32; let mut max_y = 0u32;
        for &(x,y) in self.rule_index.keys() { min_x = min_x.min(x); min_y = min_y.min(y); max_x = max_x.max(x); max_y = max_y.max(y); }
        use std::fmt::Write;
        let mut out = String::new();
        for y in (min_y..=max_y).rev() { // top to bottom so origin visually bottom-left
            for x in min_x..=max_x {
                let c = (x,y);
                if self.rule_index.contains_key(&c) {
                    if let Some(v) = self.occupied.get(&c) { write!(out, "{v} ").ok(); } else { out.push_str(". "); }
                } else {
                    out.push_str("  ");
                }
            }
            out.push('\n');
        }
        out
    }

    pub fn ascii_board_colored_pairs(&self, color: bool) -> String {
        if self.rule_index.is_empty() { return String::new(); }
        if !color { return self.ascii_board_bottom_origin(); }
        let mut min_x = u32::MAX; let mut min_y = u32::MAX; let mut max_x = 0u32; let mut max_y = 0u32;
        for &(x,y) in self.rule_index.keys() { min_x = min_x.min(x); min_y = min_y.min(y); max_x = max_x.max(x); max_y = max_y.max(y); }
        use std::fmt::Write; let mut out = String::new();
        for y in (min_y..=max_y).rev() {
            for x in min_x..=max_x {
                let c=(x,y);
                if self.rule_index.contains_key(&c) {
                    if let Some(&v)=self.occupied.get(&c) {
                        let id = self.domino_ids.get(&c).copied();
                        if let Some(idx) = id { let (start,end)=color_for_domino(idx); write!(out, "{start}{v}{end} ").ok(); } else { write!(out, "{v} ").ok(); }
                    } else { out.push_str(". "); }
                } else { out.push_str("  "); }
            }
            out.push('\n');
        }
        out
    }
    /// New default ascii_board name referencing colored pairs output
    pub fn ascii_board(&self, color: bool) -> String { self.ascii_board_colored_pairs(color) }
}

fn color_for_domino(idx: usize) -> (&'static str, &'static str) {
    const RESET: &str = "\x1b[0m";
    // Foreground (text) colors, bold for visibility. Cycles if more dominoes than colors.
    const PAL: [&str; 12] = [
        "\x1b[1;38;5;196m", // red
        "\x1b[1;38;5;202m", // orange
        "\x1b[1;38;5;226m", // yellow
        "\x1b[1;38;5;46m",  // green
        "\x1b[1;38;5;51m",  // cyan
        "\x1b[1;38;5;27m",  // blue
        "\x1b[1;38;5;129m", // purple
        "\x1b[1;38;5;201m", // pink
        "\x1b[1;38;5;208m", // dark orange
        "\x1b[1;38;5;118m", // light green
        "\x1b[1;38;5;99m",  // violet
        "\x1b[1;38;5;244m", // grey
    ];
    (PAL[idx % PAL.len()], RESET)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_parse_basic() {
        assert!(matches!(Rule::parse("="), Rule::Equal));
        assert!(matches!(Rule::parse("x"), Rule::Any));
        assert!(matches!(Rule::parse(">3"), Rule::GreaterThan(3)));
        assert!(matches!(Rule::parse("<7"), Rule::LessThan(7)));
        assert!(matches!(Rule::parse("10"), Rule::Sum(10)));
    }

    #[test]
    fn rule_parse_unknown() {
        assert!(matches!(Rule::parse("??"), Rule::Unknown));
    }

    #[test]
    fn solve_trivial_two_cells() {
        // Two adjacent cells with a single domino (2,5) and no constraints other than presence.
        let parsed = GridFile {
            grid: vec![GridEntry { rule: "x".to_string(), coords: vec![(0,0),(1,0)] }],
            dominoes: vec![(2,5)],
        };
        let mut g = GameGrid::from_parsed(parsed);
        let sol = g.solve().expect("should solve");
        assert_eq!(sol.len(), 2);
        let vals: Vec<u8> = sol.values().copied().collect();
        assert!(vals.contains(&2) && vals.contains(&5));
        // Color flag off should yield no ANSI escapes
        let plain = g.ascii_board(false);
        assert!(!plain.contains("\x1b["));
    }

    #[test]
    fn ascii_color_flag_changes_output() {
        let parsed = GridFile {
            grid: vec![GridEntry { rule: "x".into(), coords: vec![(0,0),(1,0)] }],
            dominoes: vec![(1,1)],
        };
        let mut g = GameGrid::from_parsed(parsed);
        g.solve().unwrap();
        let colored = g.ascii_board(true);
        let plain = g.ascii_board(false);
        assert!(colored.contains("\x1b["));
        assert!(!plain.contains("\x1b["));
    }

    #[test]
    fn ascii_empty_grid() {
        let parsed = GridFile { grid: vec![], dominoes: vec![] };
        let g = GameGrid::from_parsed(parsed);
        assert_eq!(g.ascii_board(false), "");
        assert_eq!(g.ascii_board(true), "");
    }

    #[test]
    fn unsolvable_two_cells_equal_rule() {
        // Rule requires equality but only domino (1,2) available.
        let parsed = GridFile { grid: vec![GridEntry{ rule: "=".into(), coords: vec![(0,0),(1,0)] }], dominoes: vec![(1,2)] };
        let mut g = GameGrid::from_parsed(parsed);
        assert!(g.solve().is_none());
    }

    // Region state branch coverage tests
    #[test]
    fn region_equal_violated() {
        let parsed = GridFile { grid: vec![GridEntry{ rule: "=".into(), coords: vec![(0,0),(1,0)] }], dominoes: vec![(1,1)] };
        let mut g = GameGrid::from_parsed(parsed);
        g.occupied.insert((0,0), 1);
        g.occupied.insert((1,0), 2);
        assert!(matches!(g.region_state(0), RegionState::Violated));
    }

    #[test]
    fn region_sum_variants() {
        // sum > target
        let parsed = GridFile { grid: vec![GridEntry{ rule: "3".into(), coords: vec![(0,0),(1,0)] }], dominoes: vec![] };
        let mut g = GameGrid::from_parsed(parsed);
        g.occupied.insert((0,0),2); g.occupied.insert((1,0),2);
        assert!(matches!(g.region_state(0), RegionState::Violated));
        // max_possible < target
        let parsed2 = GridFile { grid: vec![GridEntry{ rule: "8".into(), coords: vec![(0,0),(1,0)] }], dominoes: vec![] };
        let mut g2 = GameGrid::from_parsed(parsed2);
        g2.occupied.insert((0,0),1); // one empty cell left => max_possible 7 <8
        assert!(matches!(g2.region_state(0), RegionState::Violated));
        // satisfied final
        let parsed3 = GridFile { grid: vec![GridEntry{ rule: "5".into(), coords: vec![(0,0),(1,0)] }], dominoes: vec![] };
        let mut g3 = GameGrid::from_parsed(parsed3);
        g3.occupied.insert((0,0),2); g3.occupied.insert((1,0),3);
        assert!(matches!(g3.region_state(0), RegionState::Satisfied));
    }

    #[test]
    fn region_greater_than_variants() {
        // satisfied
        let parsed = GridFile { grid: vec![GridEntry{ rule: ">3".into(), coords: vec![(0,0),(1,0)] }], dominoes: vec![] };
        let mut g = GameGrid::from_parsed(parsed);
        g.occupied.insert((0,0),2); g.occupied.insert((1,0),2);
        assert!(matches!(g.region_state(0), RegionState::Satisfied));
        // boundary violated final (sum == k)
        let parsed2 = GridFile { grid: vec![GridEntry{ rule: ">3".into(), coords: vec![(0,0),(1,0)] }], dominoes: vec![] };
        let mut g2 = GameGrid::from_parsed(parsed2);
        g2.occupied.insert((0,0),1); g2.occupied.insert((1,0),2);
        assert!(matches!(g2.region_state(0), RegionState::Violated));
        // max_possible <= k early violation
        let parsed3 = GridFile { grid: vec![GridEntry{ rule: ">8".into(), coords: vec![(0,0),(1,0)] }], dominoes: vec![] };
        let mut g3 = GameGrid::from_parsed(parsed3);
        g3.occupied.insert((0,0),2); // max possible 8
        assert!(matches!(g3.region_state(0), RegionState::Violated));
    }

    #[test]
    fn region_less_than_variants() {
        // satisfied final (sum < k)
        let parsed = GridFile { grid: vec![GridEntry{ rule: "<5".into(), coords: vec![(0,0),(1,0)] }], dominoes: vec![] };
        let mut g = GameGrid::from_parsed(parsed);
        g.occupied.insert((0,0),2); g.occupied.insert((1,0),2);
        assert!(matches!(g.region_state(0), RegionState::Satisfied));
        // violated sum >= k
        let parsed2 = GridFile { grid: vec![GridEntry{ rule: "<4".into(), coords: vec![(0,0),(1,0)] }], dominoes: vec![] };
        let mut g2 = GameGrid::from_parsed(parsed2);
        g2.occupied.insert((0,0),2); g2.occupied.insert((1,0),2);
        assert!(matches!(g2.region_state(0), RegionState::Violated));
    }
}
