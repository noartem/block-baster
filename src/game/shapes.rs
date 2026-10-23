use std::sync::LazyLock;

/// A cell offset inside a shape: (row, col).
pub type Cell = (i32, i32);

#[derive(Debug)]
pub struct Shape {
    /// Unique id of this exact orientation, e.g. "L4-2".
    pub id: String,
    /// Id of the shape family, shared by all rotations.
    pub family: &'static str,
    pub cells: Vec<Cell>,
    pub rows: i32,
    pub cols: i32,
    /// One bitmask per shape row (bit c set = cell at column c).
    pub row_masks: Vec<u32>,
    /// Relative probability of this orientation being dealt.
    pub weight: f64,
    /// 0 = small/easy, 1 = medium, 2 = large/awkward.
    pub tier: usize,
}

impl PartialEq for Shape {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

struct Family {
    family: &'static str,
    pattern: &'static [&'static str],
    /// Total weight of the family, split evenly over its distinct rotations.
    weight: f64,
    tier: usize,
    /// Whether to generate all distinct rotations (true) or keep the pattern as-is.
    rotate: bool,
}

const FAMILIES: &[Family] = &[
    Family { family: "dot", pattern: &["#"], weight: 3.0, tier: 0, rotate: false },
    Family { family: "I2", pattern: &["##"], weight: 6.0, tier: 0, rotate: true },
    Family { family: "I3", pattern: &["###"], weight: 8.0, tier: 0, rotate: true },
    Family { family: "I4", pattern: &["####"], weight: 8.0, tier: 1, rotate: true },
    Family { family: "I5", pattern: &["#####"], weight: 4.0, tier: 2, rotate: true },
    Family { family: "O2", pattern: &["##", "##"], weight: 8.0, tier: 0, rotate: false },
    Family { family: "O3", pattern: &["###", "###", "###"], weight: 3.0, tier: 2, rotate: false },
    Family { family: "R23", pattern: &["###", "###"], weight: 5.0, tier: 1, rotate: true },
    Family { family: "V3", pattern: &["#.", "##"], weight: 8.0, tier: 0, rotate: true },
    Family { family: "L4", pattern: &["#.", "#.", "##"], weight: 7.0, tier: 1, rotate: true },
    Family { family: "J4", pattern: &[".#", ".#", "##"], weight: 7.0, tier: 1, rotate: true },
    Family { family: "T4", pattern: &["###", ".#."], weight: 7.0, tier: 1, rotate: true },
    Family { family: "S4", pattern: &[".##", "##."], weight: 5.0, tier: 1, rotate: true },
    Family { family: "Z4", pattern: &["##.", ".##"], weight: 5.0, tier: 1, rotate: true },
    Family { family: "V5", pattern: &["#..", "#..", "###"], weight: 4.0, tier: 2, rotate: true },
];

fn parse(pattern: &[&str]) -> Vec<Cell> {
    let mut cells = Vec::new();
    for (r, line) in pattern.iter().enumerate() {
        for (c, ch) in line.chars().enumerate() {
            if ch == '#' {
                cells.push((r as i32, c as i32));
            }
        }
    }
    cells
}

fn normalize(cells: &[Cell]) -> Vec<Cell> {
    let min_r = cells.iter().map(|c| c.0).min().unwrap_or(0);
    let min_c = cells.iter().map(|c| c.1).min().unwrap_or(0);
    let mut out: Vec<Cell> = cells.iter().map(|&(r, c)| (r - min_r, c - min_c)).collect();
    out.sort();
    out
}

/// 90° clockwise: (r, c) -> (c, -r).
fn rotate(cells: &[Cell]) -> Vec<Cell> {
    normalize(&cells.iter().map(|&(r, c)| (c, -r)).collect::<Vec<_>>())
}

fn make_shape(f: &Family, cells: Vec<Cell>, index: usize, weight: f64) -> Shape {
    let rows = cells.iter().map(|c| c.0).max().unwrap_or(0) + 1;
    let cols = cells.iter().map(|c| c.1).max().unwrap_or(0) + 1;
    let mut row_masks = vec![0u32; rows as usize];
    for &(r, c) in &cells {
        row_masks[r as usize] |= 1 << c;
    }
    Shape { id: format!("{}-{}", f.family, index), family: f.family, cells, rows, cols, row_masks, weight, tier: f.tier }
}

fn build_shapes() -> Vec<Shape> {
    let mut shapes = Vec::new();
    for f in FAMILIES {
        let mut variants: Vec<Vec<Cell>> = Vec::new();
        let mut cells = normalize(&parse(f.pattern));
        for _ in 0..(if f.rotate { 4 } else { 1 }) {
            if !variants.contains(&cells) {
                variants.push(cells.clone());
            }
            cells = rotate(&cells);
        }
        let n = variants.len() as f64;
        for (i, v) in variants.into_iter().enumerate() {
            shapes.push(make_shape(f, v, i, f.weight / n));
        }
    }
    shapes
}

pub static SHAPES: LazyLock<Vec<Shape>> = LazyLock::new(build_shapes);

/// Every shape orientation, in a fixed order.
pub fn shapes() -> &'static [Shape] {
    &SHAPES
}

pub fn shape_by_id(id: &str) -> Option<&'static Shape> {
    SHAPES.iter().find(|s| s.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique() {
        let ids: HashSet<_> = shapes().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids.len(), shapes().len());
    }

    #[test]
    fn generates_distinct_rotations() {
        let count = |f: &str| shapes().iter().filter(|s| s.family == f).count();
        assert_eq!(count("dot"), 1);
        assert_eq!(count("I2"), 2);
        assert_eq!(count("O2"), 1);
        assert_eq!(count("L4"), 4);
        assert_eq!(count("S4"), 2);
        assert_eq!(count("T4"), 4);
        assert_eq!(count("V5"), 4);
    }

    #[test]
    fn row_masks_match_cells() {
        for s in shapes() {
            let mut n = 0;
            for (r, m) in s.row_masks.iter().enumerate() {
                for c in 0..s.cols {
                    if m & (1 << c) != 0 {
                        assert!(s.cells.contains(&(r as i32, c)));
                        n += 1;
                    }
                }
            }
            assert_eq!(n, s.cells.len());
        }
    }

    #[test]
    fn splits_family_weight_over_rotations() {
        let total: f64 = shapes().iter().filter(|s| s.family == "L4").map(|s| s.weight).sum();
        assert!((total - 7.0).abs() < 1e-9);
    }

    #[test]
    fn finds_by_id() {
        assert_eq!(shape_by_id("O3-0").unwrap().cells.len(), 9);
        assert!(shape_by_id("nope").is_none());
    }
}
