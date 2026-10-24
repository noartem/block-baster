use serde::{Deserialize, Serialize};

use super::board::{
    Board, ClearedCell, Lines, can_place, can_place_anywhere, clear_lines, create_board, filled_count, find_full_lines, is_empty, place_shape,
};
use super::generator::{TRAY_SIZE, generate_tray, is_lucky_deal};
use super::rng::Rng;
use super::shapes::{Shape, shape_by_id};

pub const COLOR_COUNT: usize = 7;
/// Placements without a clear after which the combo streak ends.
pub const COMBO_WINDOW: u32 = 3;
pub const BOARD_CLEAR_BONUS: u32 = 300;

#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub shape: &'static Shape,
    /// 1-based colour id.
    pub color: u8,
}

#[derive(Clone, Debug)]
pub struct PlaceResult {
    pub slot: usize,
    pub piece: Piece,
    pub row: i32,
    pub col: i32,
    pub lines: Lines,
    pub cleared: Vec<ClearedCell>,
    /// Points gained by this move (placement + lines + bonuses).
    pub points: u32,
    pub line_points: u32,
    /// Current combo streak after this move (0 = no streak).
    pub combo: u32,
    pub board_cleared: bool,
    /// True when a fresh tray was dealt after this move.
    pub refilled: bool,
    pub game_over: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedPiece {
    pub id: String,
    pub color: u8,
}

/// The saved game, in the same JSON shape the web version always stored.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedGame {
    pub v: u32,
    pub board: Vec<u8>,
    pub tray: Vec<Option<SavedPiece>>,
    pub score: u32,
    pub combo: u32,
    pub since_clear: u32,
    pub moves: u32,
    /// Times the whole board was cleared; picks the style variant. Missing in older saves.
    #[serde(default)]
    pub clears: u32,
    /// Plain trays dealt since the last lucky one. Missing in older saves.
    #[serde(default)]
    pub since_lucky: u32,
    /// Whether the current tray is a lucky one. Missing in older saves.
    #[serde(default)]
    pub lucky: bool,
    pub rng: u32,
    pub over: bool,
}

/// Points for clearing `lines` lines at once with the given combo streak.
pub fn line_points(lines: u32, combo: u32) -> u32 {
    if lines == 0 {
        return 0;
    }
    10 * lines * (lines + 1) * combo.max(1)
}

#[derive(Clone, Debug)]
pub struct Game {
    pub board: Board,
    pub tray: [Option<Piece>; TRAY_SIZE],
    pub score: u32,
    pub combo: u32,
    pub since_clear: u32,
    pub moves: u32,
    pub board_clears: u32,
    /// Plain trays dealt since the last lucky one.
    pub since_lucky: u32,
    /// Whether the current tray was dealt as a lucky one.
    pub lucky: bool,
    pub over: bool,
    pub rng: Rng,
}

impl Game {
    pub fn new(seed: Option<u32>) -> Self {
        let mut game = Game {
            board: create_board(),
            tray: [None; TRAY_SIZE],
            score: 0,
            combo: 0,
            since_clear: 0,
            moves: 0,
            board_clears: 0,
            since_lucky: 0,
            lucky: false,
            over: false,
            rng: seed.map_or_else(Rng::from_time, Rng::new),
        };
        game.deal();
        game
    }

    /// Deals a new set of pieces into the tray.
    fn deal(&mut self) {
        let fill = filled_count(&self.board) as f64 / self.board.len() as f64;
        self.lucky = is_lucky_deal(self.score as f64, fill, self.since_lucky);
        self.since_lucky = if self.lucky { 0 } else { self.since_lucky + 1 };
        let shapes = generate_tray(&self.board, &mut self.rng, self.score as f64, self.lucky);
        let mut used: Vec<u8> = Vec::new();
        for (slot, shape) in shapes.into_iter().enumerate() {
            // Prefer distinct colours inside one tray.
            let mut color = 1 + self.rng.int(COLOR_COUNT) as u8;
            let mut i = 0;
            while i < 4 && used.contains(&color) {
                color = 1 + self.rng.int(COLOR_COUNT) as u8;
                i += 1;
            }
            used.push(color);
            self.tray[slot] = Some(Piece { shape, color });
        }
    }

    pub fn can_place(&self, slot: usize, row: i32, col: i32) -> bool {
        match self.tray.get(slot).copied().flatten() {
            Some(piece) => !self.over && can_place(&self.board, piece.shape, row, col),
            None => false,
        }
    }

    /// Whether the piece in the slot fits anywhere on the board right now.
    pub fn is_playable(&self, slot: usize) -> bool {
        self.tray.get(slot).copied().flatten().is_some_and(|p| can_place_anywhere(&self.board, p.shape))
    }

    pub fn has_moves(&self) -> bool {
        (0..TRAY_SIZE).any(|i| self.is_playable(i))
    }

    pub fn place(&mut self, slot: usize, row: i32, col: i32) -> Option<PlaceResult> {
        if !self.can_place(slot, row, col) {
            return None;
        }
        let piece = self.tray[slot].take()?;
        place_shape(&mut self.board, piece.shape, row, col, piece.color);
        self.moves += 1;

        let lines = find_full_lines(&self.board);
        let count = lines.count() as u32;
        let cleared = clear_lines(&mut self.board, &lines);
        if count > 0 {
            self.combo += 1;
            self.since_clear = 0;
        } else {
            self.since_clear += 1;
            if self.since_clear >= COMBO_WINDOW {
                self.combo = 0;
            }
        }

        let board_cleared = count > 0 && is_empty(&self.board);
        if board_cleared {
            self.board_clears += 1;
        }
        let lp = line_points(count, self.combo);
        let points = piece.shape.cells.len() as u32 + lp + if board_cleared { BOARD_CLEAR_BONUS } else { 0 };
        self.score += points;

        let mut refilled = false;
        if self.tray.iter().all(Option::is_none) {
            self.deal();
            refilled = true;
        }
        self.over = !self.has_moves();

        Some(PlaceResult {
            slot,
            piece,
            row,
            col,
            lines,
            cleared,
            points,
            line_points: lp,
            combo: if count > 0 { self.combo } else { 0 },
            board_cleared,
            refilled,
            game_over: self.over,
        })
    }

    pub fn serialize(&self) -> SavedGame {
        SavedGame {
            v: 1,
            board: self.board.to_vec(),
            tray: self.tray.iter().map(|p| p.map(|p| SavedPiece { id: p.shape.id.clone(), color: p.color })).collect(),
            score: self.score,
            combo: self.combo,
            since_clear: self.since_clear,
            moves: self.moves,
            clears: self.board_clears,
            since_lucky: self.since_lucky,
            lucky: self.lucky,
            rng: self.rng.state,
            over: self.over,
        }
    }

    pub fn deserialize(data: &SavedGame) -> Result<Game, String> {
        if data.v != 1 || data.board.len() != 64 || data.tray.len() != TRAY_SIZE {
            return Err("Unsupported save".into());
        }
        let mut tray = [None; TRAY_SIZE];
        for (slot, p) in data.tray.iter().enumerate() {
            if let Some(p) = p {
                let shape = shape_by_id(&p.id).ok_or_else(|| format!("Unknown shape: {}", p.id))?;
                tray[slot] = Some(Piece { shape, color: p.color });
            }
        }
        let mut board = create_board();
        board.copy_from_slice(&data.board);
        Ok(Game {
            board,
            tray,
            score: data.score,
            combo: data.combo,
            since_clear: data.since_clear,
            moves: data.moves,
            board_clears: data.clears,
            since_lucky: data.since_lucky,
            lucky: data.lucky,
            over: data.over,
            rng: Rng::new(data.rng),
        })
    }

    /// Replaces the tray with the given shape ids, coloured 1, 2, 3 (for tests and tools).
    pub fn set_tray(&mut self, ids: &[Option<&str>]) {
        self.tray = [None; TRAY_SIZE];
        for (i, id) in ids.iter().enumerate().take(TRAY_SIZE) {
            self.tray[i] = id.and_then(shape_by_id).map(|shape| Piece { shape, color: i as u8 + 1 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::board::idx;
    use crate::game::board::tests::board_from;

    fn with_tray(g: &mut Game, ids: &[Option<&str>]) {
        g.set_tray(ids);
        g.over = false;
    }

    #[test]
    fn starts_with_a_full_tray_and_an_empty_board() {
        let g = Game::new(Some(1));
        assert!(g.tray.iter().all(Option::is_some));
        assert!(is_empty(&g.board));
        assert_eq!(g.score, 0);
    }

    #[test]
    fn scores_one_point_per_placed_block() {
        let mut g = Game::new(Some(1));
        with_tray(&mut g, &[Some("O2-0"), Some("dot-0"), Some("dot-0")]);
        let res = g.place(0, 3, 3).unwrap();
        assert_eq!(res.points, 4);
        assert_eq!(g.score, 4);
        assert!(g.tray[0].is_none());
        assert_eq!(g.board[idx(4, 4)], 1);
    }

    #[test]
    fn rejects_invalid_placements() {
        let mut g = Game::new(Some(1));
        with_tray(&mut g, &[Some("O2-0"), None, Some("dot-0")]);
        assert!(g.place(0, 7, 7).is_none());
        assert!(g.place(1, 0, 0).is_none());
        assert_eq!(g.score, 0);
    }

    #[test]
    fn clears_lines_scores_them_and_tracks_combo_streaks() {
        let mut g = Game::new(Some(1));
        g.board = board_from(&["####....", "####....", "", "", "", "", "", "#"]);
        with_tray(&mut g, &[Some("I4-0"), Some("I4-0"), Some("dot-0")]);
        let first = g.place(0, 0, 4).unwrap();
        assert_eq!(first.lines.rows, vec![0]);
        assert_eq!(first.combo, 1);
        assert_eq!(first.points, 4 + line_points(1, 1));
        let second = g.place(1, 1, 4).unwrap();
        assert_eq!(second.combo, 2);
        assert_eq!(second.line_points, line_points(1, 2));
        assert!(!second.board_cleared);
    }

    #[test]
    fn awards_a_bonus_when_the_whole_board_is_cleared() {
        let mut g = Game::new(Some(1));
        g.board = board_from(&["#######."]);
        with_tray(&mut g, &[Some("dot-0"), Some("dot-0"), Some("dot-0")]);
        let res = g.place(0, 0, 7).unwrap();
        assert!(res.board_cleared);
        assert_eq!(res.points, 1 + line_points(1, 1) + BOARD_CLEAR_BONUS);
    }

    #[test]
    fn ends_the_combo_after_too_many_placements_without_a_clear() {
        let mut g = Game::new(Some(1));
        g.combo = 4;
        for i in 0..COMBO_WINDOW {
            with_tray(&mut g, &[Some("dot-0"), Some("dot-0"), Some("dot-0")]);
            g.place(0, 4, i as i32);
            assert_eq!(g.combo, if i < COMBO_WINDOW - 1 { 4 } else { 0 });
        }
    }

    #[test]
    fn deals_a_new_tray_once_all_pieces_are_used() {
        let mut g = Game::new(Some(1));
        with_tray(&mut g, &[Some("dot-0"), Some("dot-0"), Some("dot-0")]);
        assert!(!g.place(0, 0, 0).unwrap().refilled);
        assert!(!g.place(1, 0, 1).unwrap().refilled);
        assert!(g.place(2, 0, 2).unwrap().refilled);
        assert!(g.tray.iter().all(Option::is_some));
    }

    #[test]
    fn deals_lucky_trays_on_a_steady_beat_that_slows_with_the_score() {
        let mut g = Game::new(Some(1));
        assert!(g.lucky);
        g.score = 100_000;
        let mut beat = Vec::new();
        for _ in 0..6 {
            g.board = create_board();
            with_tray(&mut g, &[Some("dot-0"), Some("dot-0"), Some("dot-0")]);
            g.place(0, 0, 0);
            g.place(1, 0, 1);
            g.place(2, 0, 2);
            beat.push(g.lucky);
        }
        assert_eq!(beat, [false, false, true, false, false, true]);
    }

    #[test]
    fn detects_game_over_when_no_remaining_piece_fits() {
        let mut g = Game::new(Some(1));
        g.board = board_from(&[".#.#.#.#", "#.#.#.#.", ".#.#.#.#", "#.#.#.#.", ".#.#.#.#", "#.#.#.#.", ".#.#.#.#", "#.#.#.#."]);
        with_tray(&mut g, &[Some("dot-0"), Some("O2-0"), Some("I3-0")]);
        let res = g.place(0, 0, 0).unwrap();
        assert!(res.game_over);
        assert!(g.over);
        assert!(g.place(1, 0, 0).is_none());
    }

    #[test]
    fn round_trips_through_serialization() {
        let mut g = Game::new(Some(99));
        g.board = create_board();
        g.place(0, 0, 0);
        let json = serde_json::to_string(&g.serialize()).unwrap();
        let mut copy = Game::deserialize(&serde_json::from_str(&json).unwrap()).unwrap();
        assert_eq!(copy.serialize(), g.serialize());
        assert_eq!(copy.since_lucky, g.since_lucky);
        assert_eq!(copy.rng.next(), g.rng.next());
        assert!(serde_json::from_str::<SavedGame>(r#"{"v":2}"#).is_err());
    }

    #[test]
    fn reads_saves_of_the_web_version() {
        let json = r#"{"v":1,"board":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,3],
          "tray":[{"id":"L4-2","color":5},null,{"id":"dot-0","color":1}],"score":120,"combo":1,"sinceClear":0,"moves":12,"rng":123456,"over":false}"#;
        let g = Game::deserialize(&serde_json::from_str(json).unwrap()).unwrap();
        assert_eq!(g.score, 120);
        assert_eq!(g.board[63], 3);
        assert_eq!(g.tray[0].unwrap().shape.id, "L4-2");
        assert_eq!(g.board_clears, 0);
        assert!(!g.lucky);
    }

    #[test]
    fn plays_full_random_games_to_completion_without_errors() {
        for seed in 1..=5 {
            let mut g = Game::new(Some(seed));
            let mut guard = 0;
            while !g.over && guard < 2000 {
                guard += 1;
                let mut moved = false;
                'outer: for s in 0..3 {
                    for r in 0..8 {
                        for c in 0..8 {
                            if g.place(s, r, c).is_some() {
                                moved = true;
                                break 'outer;
                            }
                        }
                    }
                }
                assert!(moved);
            }
            assert!(g.over);
            assert!(g.score > 0);
        }
    }
}
