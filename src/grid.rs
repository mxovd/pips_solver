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
                // Build orientations vector to avoid borrowing temporary slice
                let orientations: &[(u8, u8)] = if domino.0 == domino.1 {
                    &[(domino.0, domino.1)]
                } else {
                    &[(domino.0, domino.1), (domino.1, domino.0)]
                };
                for &(a_val, b_val) in orientations {
                    // Assign
                    self.occupied.insert(next_coord, a_val);
                    self.occupied.insert(partner, b_val);
                    if self.affected_regions_feasible(&[next_coord, partner]) {
                        // mark domino used
                        let saved = domino;
                        self.domino_inventory[i] = (255, 255);
                        if self.backtrack() {
                            return true;
                        }
                        // restore domino
                        self.domino_inventory[i] = saved;
                    }
                    // undo assignment
                    self.occupied.remove(&next_coord);
                    self.occupied.remove(&partner);
                }
            }
        }
        false
    }
}
