use super::shapes::Shape;

pub const SIZE: usize = 8;
const FULL_ROW: u32 = (1 << SIZE) - 1;

/// The board is a flat array of SIZE*SIZE cells. 0 means empty, any other value
/// is the colour id (1-based) of the block occupying that cell.
pub type Board = [u8; SIZE * SIZE];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lines {
    pub rows: Vec<usize>,
    pub cols: Vec<usize>,
}

impl Lines {
    pub fn count(&self) -> usize {
        self.rows.len() + self.cols.len()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClearedCell {
    pub r: usize,
    pub c: usize,
    pub color: u8,
}

pub fn create_board() -> Board {
    [0; SIZE * SIZE]
}

pub const fn idx(r: usize, c: usize) -> usize {
    r * SIZE + c
}

pub fn can_place(board: &Board, shape: &Shape, r: i32, c: i32) -> bool {
    if r < 0 || c < 0 || r + shape.rows > SIZE as i32 || c + shape.cols > SIZE as i32 {
        return false;
    }
    shape.cells.iter().all(|&(dr, dc)| board[idx((r + dr) as usize, (c + dc) as usize)] == 0)
}

/// Writes the shape into the board. Caller must check can_place first.
pub fn place_shape(board: &mut Board, shape: &Shape, r: i32, c: i32, color: u8) {
    for &(dr, dc) in &shape.cells {
        board[idx((r + dr) as usize, (c + dc) as usize)] = color;
    }
}

pub fn find_full_lines(board: &Board) -> Lines {
    let rows = (0..SIZE).filter(|&r| (0..SIZE).all(|c| board[idx(r, c)] != 0)).collect();
    let cols = (0..SIZE).filter(|&c| (0..SIZE).all(|r| board[idx(r, c)] != 0)).collect();
    Lines { rows, cols }
}

/// Empties the given rows/cols and returns the removed cells (each cell once).
pub fn clear_lines(board: &mut Board, lines: &Lines) -> Vec<ClearedCell> {
    let mut cleared = Vec::new();
    let mut mark = |board: &mut Board, r: usize, c: usize| {
        let i = idx(r, c);
        if board[i] != 0 {
            cleared.push(ClearedCell { r, c, color: board[i] });
            board[i] = 0;
        }
    };
    for &r in &lines.rows {
        for c in 0..SIZE {
            mark(board, r, c);
        }
    }
    for &c in &lines.cols {
        for r in 0..SIZE {
            mark(board, r, c);
        }
    }
    cleared
}

/// Lines that would be completed if the shape were placed at (r, c).
pub fn preview_lines(board: &Board, shape: &Shape, r: i32, c: i32) -> Lines {
    let mut copy = *board;
    place_shape(&mut copy, shape, r, c, 1);
    find_full_lines(&copy)
}

pub fn can_place_anywhere(board: &Board, shape: &Shape) -> bool {
    any_fit(&to_masks(board), shape)
}

pub fn is_empty(board: &Board) -> bool {
    board.iter().all(|&v| v == 0)
}

pub fn filled_count(board: &Board) -> usize {
    board.iter().filter(|&&v| v != 0).count()
}

// ---------------------------------------------------------------------------
// Bitmask representation used by the (hot) solver: one 8-bit mask per row.
// ---------------------------------------------------------------------------

pub type Masks = [u32; SIZE];

pub fn to_masks(board: &Board) -> Masks {
    let mut masks = [0u32; SIZE];
    for (r, mask) in masks.iter_mut().enumerate() {
        for c in 0..SIZE {
            if board[idx(r, c)] != 0 {
                *mask |= 1 << c;
            }
        }
    }
    masks
}

pub fn fits_mask(masks: &Masks, shape: &Shape, r: usize, c: usize) -> bool {
    shape.row_masks.iter().enumerate().all(|(i, &m)| masks[r + i] & (m << c) == 0)
}

pub fn any_fit(masks: &Masks, shape: &Shape) -> bool {
    let (rows, cols) = (shape.rows as usize, shape.cols as usize);
    (0..=SIZE - rows).any(|r| (0..=SIZE - cols).any(|c| fits_mask(masks, shape, r, c)))
}

/// Places a shape on a copy of the masks, clears full lines and returns the result and the number of lines.
pub fn place_mask(masks: &Masks, shape: &Shape, r: usize, c: usize) -> (Masks, u32) {
    let mut next = *masks;
    for (i, &m) in shape.row_masks.iter().enumerate() {
        next[r + i] |= m << c;
    }
    let col_full = next.iter().fold(FULL_ROW, |acc, &m| acc & m);
    let mut lines = 0;
    for m in next.iter_mut() {
        if *m == FULL_ROW {
            *m = 0;
            lines += 1;
        }
    }
    if col_full != 0 {
        for m in next.iter_mut() {
            *m &= !col_full;
        }
        lines += col_full.count_ones();
    }
    (next, lines)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::game::shapes::shape_by_id;

    /// Builds a board from 8 strings where '#' is a filled cell.
    pub fn board_from(rows: &[&str]) -> Board {
        let mut b = create_board();
        for (r, line) in rows.iter().enumerate() {
            for (c, ch) in line.chars().enumerate().take(SIZE) {
                if ch == '#' {
                    b[idx(r, c)] = 1;
                }
            }
        }
        b
    }

    fn shape(id: &str) -> &'static Shape {
        shape_by_id(id).unwrap()
    }

    #[test]
    fn checks_bounds_and_overlap() {
        let b = board_from(&["#......."]);
        assert!(!can_place(&b, shape("dot-0"), 0, 0));
        assert!(can_place(&b, shape("dot-0"), 0, 1));
        assert!(!can_place(&b, shape("I3-0"), 0, 6));
        assert!(!can_place(&b, shape("dot-0"), -1, 0));
        assert!(can_place(&b, shape("O2-0"), 6, 6));
        assert!(!can_place(&b, shape("O2-0"), 7, 6));
    }

    #[test]
    fn finds_and_clears_lines_counting_each_cell_once() {
        let mut b = board_from(&["########", "#", "#", "#", "#", "#", "#", "#"]);
        let lines = find_full_lines(&b);
        assert_eq!(lines, Lines { rows: vec![0], cols: vec![0] });
        let cleared = clear_lines(&mut b, &lines);
        assert_eq!(cleared.len(), 15);
        assert!(is_empty(&b));
    }

    #[test]
    fn previews_lines_without_changing_the_board() {
        let b = board_from(&["#######."]);
        let lines = preview_lines(&b, shape("dot-0"), 0, 7);
        assert_eq!(lines.rows, vec![0]);
        assert_eq!(filled_count(&b), 7);
    }

    #[test]
    fn masks_agree_with_the_board() {
        let b = board_from(&["##......", "", "..#....."]);
        let m = to_masks(&b);
        assert_eq!(m[0], 0b11);
        assert_eq!(m[2], 0b100);
        assert!(!fits_mask(&m, shape("O2-0"), 0, 0));
        assert!(fits_mask(&m, shape("O2-0"), 0, 2));
    }

    #[test]
    fn place_mask_clears_rows_and_columns() {
        let b = board_from(&["#######.", ".......#", ".......#", ".......#", ".......#", ".......#", ".......#", ".......#"]);
        let (next, lines) = place_mask(&to_masks(&b), shape("dot-0"), 0, 7);
        assert_eq!(lines, 2);
        assert!(next.iter().all(|&m| m == 0));
    }

    #[test]
    fn detects_when_nothing_fits() {
        let b = board_from(&["#.#.#.#.", ".#.#.#.#", "#.#.#.#.", ".#.#.#.#", "#.#.#.#.", ".#.#.#.#", "#.#.#.#.", ".#.#.#.#"]);
        assert!(!can_place_anywhere(&b, shape("I2-0")));
        assert!(can_place_anywhere(&b, shape("dot-0")));
    }
}
