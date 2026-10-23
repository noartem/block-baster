use super::board::{Board, Masks, SIZE, any_fit, filled_count, fits_mask, place_mask, to_masks};
use super::rng::Rng;
use super::shapes::{Shape, shapes};

pub const TRAY_SIZE: usize = 3;

/// Score at which the piece mix reaches its hardest distribution.
const HARDEST_AT: f64 = 150_000.0;
/// How far towards the hard piece mix the game ever gets (0..1). Capped below
/// 1 so even very long runs never see big pieces outnumber medium ones.
const MAX_DIFFICULTY: f64 = 0.85;
const CANDIDATE_ATTEMPTS: usize = 60;
/// Solvable trays a lucky deal chooses the best one from.
const LUCKY_CANDIDATES: usize = 16;
/// Solvable trays a plain deal looks at to find one that leaves enough room.
const PLAIN_CANDIDATES: usize = 4;
/// A plain tray should leave at most this many cells filled when played out well.
const ROOMY_FILLED: u32 = 40;
const SOLVER_BUDGET: u64 = 25_000;

pub struct LuckLevel {
    pub until: f64,
    pub every: u32,
}

/// How often the dealer gives a hand. Below `until` points every `every`-th
/// tray is a lucky one: picked out of many random trays for the most lines it
/// clears and the most room it leaves. The rest are plain random trays.
/// Lucky trays never get rarer than every 3rd.
pub const LUCK_LEVELS: [LuckLevel; 3] = [
    LuckLevel { until: 3_000.0, every: 1 },
    LuckLevel { until: 40_000.0, every: 2 },
    LuckLevel { until: f64::INFINITY, every: 3 },
];

/// When the board is at least this full, the next tray is always a lucky one.
pub const RESCUE_FILL: f64 = 0.45;

pub type Tray = Vec<&'static Shape>;

/// Every how many trays a lucky one comes at this score.
pub fn luck_every(score: f64) -> u32 {
    LUCK_LEVELS.iter().find(|l| score < l.until).map_or(3, |l| l.every)
}

/// Whether the next tray should be a lucky one, given the score, how full the
/// board is (0..1) and how many plain trays were dealt since the last lucky one.
pub fn is_lucky_deal(score: f64, fill: f64, since_lucky: u32) -> bool {
    since_lucky + 1 >= luck_every(score) || fill >= RESCUE_FILL
}

/// How hard the piece mix is at this score, from 0 up to MAX_DIFFICULTY.
pub fn difficulty(score: f64) -> f64 {
    MAX_DIFFICULTY * (score.max(0.0) / HARDEST_AT).min(1.0)
}

/// Weight of a shape given the current game situation.
pub fn shape_weight(shape: &Shape, score: f64, fill: f64) -> f64 {
    let d = difficulty(score);
    // Crowded boards get smaller pieces more often, so a run doesn't end too abruptly.
    let crowded = (fill - 0.35).max(0.0) * 2.5;
    let tier_factor = [1.3 - 0.4 * d + crowded, 1.0, 0.5 + 0.6 * d - crowded * 0.5][shape.tier];
    shape.weight * tier_factor.max(0.05)
}

/// Positions where a shape may go, top-left corners.
fn spots(shape: &Shape) -> impl Iterator<Item = (usize, usize)> + use<> {
    let (rows, cols) = (SIZE - shape.rows as usize, SIZE - shape.cols as usize);
    (0..=rows).flat_map(move |r| (0..=cols).map(move |c| (r, c)))
}

/// Returns true if every shape can be placed on the board in some order
/// (accounting for lines cleared between placements). Returns false when the
/// search budget runs out, so callers treat "unknown" as "not solvable".
pub fn is_solvable(masks: &Masks, shapes: &[&Shape], budget: u64) -> bool {
    fn solve(m: &Masks, rest: &[&Shape], nodes: &mut u64, budget: u64) -> bool {
        if rest.is_empty() {
            return true;
        }
        let mut tried: Vec<&str> = Vec::new();
        for (i, shape) in rest.iter().enumerate() {
            if tried.contains(&shape.id.as_str()) {
                continue;
            }
            tried.push(&shape.id);
            let others: Vec<&Shape> = rest.iter().enumerate().filter(|&(j, _)| j != i).map(|(_, s)| *s).collect();
            for (r, c) in spots(shape) {
                if !fits_mask(m, shape, r, c) {
                    continue;
                }
                *nodes += 1;
                if *nodes > budget {
                    return false;
                }
                if others.is_empty() {
                    return true;
                }
                if solve(&place_mask(m, shape, r, c).0, &others, nodes, budget) {
                    return true;
                }
            }
        }
        false
    }
    let mut nodes = 0;
    solve(masks, shapes, &mut nodes, budget)
}

pub fn is_solvable_default(masks: &Masks, shapes: &[&Shape]) -> bool {
    is_solvable(masks, shapes, SOLVER_BUDGET)
}

const WALLS: u32 = 0b10_0000_0001;

/// How many filled cells or walls touch the shape's cells when placed at (r, c).
fn contact(masks: &Masks, shape: &Shape, r: usize, c: usize) -> u32 {
    // Rows padded with a wall bit on each side; rows outside the board are all wall.
    let row = |i: i32| if i < 0 || i >= SIZE as i32 { 0x3ff } else { (masks[i as usize] << 1) | WALLS };
    let mut n = 0;
    for (i, &m) in shape.row_masks.iter().enumerate() {
        let y = (r + i) as i32;
        let cells = m << (c + 1);
        let here = row(y);
        n += (cells & row(y - 1)).count_ones() + (cells & row(y + 1)).count_ones() + ((cells >> 1) & here).count_ones() + ((cells << 1) & here).count_ones();
    }
    n
}

const FULL: u32 = (1 << SIZE) - 1;

/// Empty cells boxed in on all four sides (by blocks or walls). Only a single
/// dot can ever fill them, so they make a board much harder to play.
pub fn holes(masks: &Masks) -> u32 {
    let mut n = 0;
    for r in 0..SIZE {
        let above = if r == 0 { FULL } else { masks[r - 1] };
        let below = if r == SIZE - 1 { FULL } else { masks[r + 1] };
        let row = masks[r];
        let left = ((row << 1) | 1) & FULL;
        let right = (row >> 1) | (1 << (SIZE - 1));
        n += (!row & above & below & left & right & FULL).count_ones();
    }
    n
}

#[derive(Clone, Debug)]
pub struct PlayOut {
    /// Lines cleared over the whole tray.
    pub lines: u32,
    /// Cells left filled afterwards.
    pub filled: u32,
    /// The board afterwards.
    pub masks: Masks,
}

/// Plays the tray out: its pieces placed one after another, in the best order,
/// each at the spot that clears most (or fits snuggest). Returns the lines
/// cleared and the board left behind. This catches pieces that only clear
/// together, like two I4s on an empty row.
pub fn play_out(masks: &Masks, tray: &[&Shape]) -> PlayOut {
    fn run(m: Masks, rest: &[&Shape], lines: u32, best: &mut PlayOut) {
        if rest.is_empty() {
            let filled = m.iter().map(|r| r.count_ones()).sum();
            if lines > best.lines || (lines == best.lines && filled < best.filled) {
                *best = PlayOut { lines, filled, masks: m };
            }
            return;
        }
        for (i, shape) in rest.iter().enumerate() {
            if rest.iter().position(|s| s.id == shape.id) != Some(i) {
                continue;
            }
            let mut pick: Option<(Masks, u32, u32)> = None;
            for (r, c) in spots(shape) {
                if !fits_mask(&m, shape, r, c) {
                    continue;
                }
                let (next, l) = place_mask(&m, shape, r, c);
                let touch = contact(&m, shape, r, c);
                if pick.is_none_or(|(_, pl, pc)| l > pl || (l == pl && touch > pc)) {
                    pick = Some((next, l, touch));
                }
            }
            if let Some((next, l, _)) = pick {
                let others: Vec<&Shape> = rest.iter().enumerate().filter(|&(j, _)| j != i).map(|(_, s)| *s).collect();
                run(next, &others, lines + l, best);
            }
        }
    }
    let mut best = PlayOut { lines: 0, filled: u32::MAX, masks: *masks };
    run(*masks, tray, 0, &mut best);
    best
}

/// Lines the tray clears when played out (see play_out).
pub fn tray_clears(masks: &Masks, tray: &[&Shape]) -> u32 {
    play_out(masks, tray).lines
}

/// How good a tray is for the player: lines cleared count most, then how much
/// room is left, with boxed-in holes counting as extra filled cells.
fn value_of(out: &PlayOut) -> f64 {
    (out.lines as f64) * (SIZE * SIZE) as f64 - out.filled as f64 - 3.0 * holes(&out.masks) as f64
}

pub fn tray_value(masks: &Masks, tray: &[&Shape]) -> f64 {
    value_of(&play_out(masks, tray))
}

/// Whether some fitting placement of the shape completes a line.
fn clears_line(masks: &Masks, shape: &Shape) -> bool {
    spots(shape).any(|(r, c)| fits_mask(masks, shape, r, c) && place_mask(masks, shape, r, c).1 > 0)
}

fn random_tray(rng: &mut Rng, weight: &dyn Fn(&Shape) -> f64) -> Tray {
    let all: Vec<&'static Shape> = shapes().iter().collect();
    let mut tray: Tray = Vec::new();
    while tray.len() < TRAY_SIZE {
        let shape = *rng.weighted(&all, |s| weight(s));
        // Avoid dealing three copies of the same piece.
        if tray.len() == 2 && tray[0].id == shape.id && tray[1].id == shape.id {
            continue;
        }
        tray.push(shape);
    }
    tray
}

/// Builds a tray piece by piece, only choosing shapes that still fit after the
/// previous ones were placed at their best spot. Always solvable when at least
/// one shape fits the board. With `clearing`, it picks a shape that completes a
/// line whenever one exists, so the tray is sure to clear something.
fn constructive_tray(masks: &Masks, rng: &mut Rng, weight: &dyn Fn(&Shape) -> f64, clearing: bool) -> Tray {
    let all: Vec<&'static Shape> = shapes().iter().collect();
    let mut tray: Tray = Vec::new();
    let mut m = *masks;
    while tray.len() < TRAY_SIZE {
        let fitting: Vec<&'static Shape> = all.iter().copied().filter(|s| any_fit(&m, s)).collect();
        if fitting.is_empty() {
            tray.push(*rng.weighted(&all, |s| weight(s)));
            continue;
        }
        let clearers: Vec<&'static Shape> = if clearing { fitting.iter().copied().filter(|s| clears_line(&m, s)).collect() } else { Vec::new() };
        let pool = if clearers.is_empty() { &fitting } else { &clearers };
        let shape = *rng.weighted(pool, |s| weight(s));
        tray.push(shape);
        let mut best: Option<(Masks, u32)> = None;
        for (r, c) in spots(shape) {
            if fits_mask(&m, shape, r, c) {
                let res = place_mask(&m, shape, r, c);
                if best.is_none_or(|b| res.1 > b.1) {
                    best = Some(res);
                }
            }
        }
        m = best.expect("fitting shape has a spot").0;
    }
    tray
}

/// Deals a new tray of pieces. The tray is guaranteed to be placeable (in some
/// order) whenever that is possible at all, mirroring how Block Blast feels:
/// you only lose through your own placement choices.
///
/// A plain tray is a random one that, played out well, still leaves room on
/// the board (when such a tray turns up quickly). A lucky tray (see
/// is_lucky_deal) is the best of many random ones by tray_value: most lines
/// cleared, then the emptiest board left behind. On a crowded board a lucky
/// tray always clears at least one line when any piece can.
pub fn generate_tray(board: &Board, rng: &mut Rng, score: f64, lucky: bool) -> Tray {
    let masks = to_masks(board);
    let fill = filled_count(board) as f64 / (SIZE * SIZE) as f64;
    let weight = move |s: &Shape| shape_weight(s, score, fill);
    let wanted = if lucky { LUCKY_CANDIDATES } else { PLAIN_CANDIDATES };

    let mut chosen: Option<Tray> = None;
    let mut chosen_value = f64::NEG_INFINITY;
    let mut chosen_lines = 0;
    let mut found = 0;
    let mut attempt = 0;
    while attempt < CANDIDATE_ATTEMPTS && found < wanted {
        attempt += 1;
        let tray = random_tray(rng, &weight);
        if !is_solvable_default(&masks, &tray) {
            continue;
        }
        found += 1;
        let out = play_out(&masks, &tray);
        if !lucky && out.filled <= ROOMY_FILLED {
            return tray;
        }
        let value = if lucky { value_of(&out) } else { -(out.filled as f64) };
        if chosen.is_none() || value > chosen_value {
            chosen = Some(tray);
            chosen_value = value;
            chosen_lines = out.lines;
        }
    }
    if lucky && fill >= RESCUE_FILL && chosen_lines == 0 {
        // Nothing random cleared a line: build a tray around a piece that does.
        let tray = constructive_tray(&masks, rng, &weight, true);
        if tray_clears(&masks, &tray) > 0 {
            return tray;
        }
    }
    chosen.unwrap_or_else(|| constructive_tray(&masks, rng, &weight, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::board::create_board;
    use crate::game::board::tests::board_from;
    use crate::game::shapes::shape_by_id;

    fn ids(list: &[&str]) -> Vec<&'static Shape> {
        list.iter().map(|id| shape_by_id(id).unwrap()).collect()
    }

    #[test]
    fn is_deterministic_for_a_given_seed() {
        let a: Vec<_> = generate_tray(&create_board(), &mut Rng::new(42), 0.0, false).iter().map(|s| s.id.clone()).collect();
        let b: Vec<_> = generate_tray(&create_board(), &mut Rng::new(42), 0.0, false).iter().map(|s| s.id.clone()).collect();
        assert_eq!(a, b);
        assert_eq!(a.len(), TRAY_SIZE);
    }

    #[test]
    fn solver_accounts_for_line_clears_between_placements() {
        // O3 only fits once the bottom row is completed and cleared.
        let b = board_from(&["#######.", "#######.", "#######.", "#######.", "######..", "...#####", "...#####", "###....."]);
        let masks = to_masks(&b);
        assert!(is_solvable_default(&masks, &ids(&["O3-0", "I5-0"])));
        assert!(!is_solvable_default(&masks, &ids(&["O3-0", "dot-0"])));
    }

    #[test]
    fn always_deals_a_solvable_tray_when_one_exists() {
        let mut rng = Rng::new(7);
        let crowded = board_from(&["###.####", "###.####", "###.####", "#######.", "#######.", "##.#####", "##.#####", "........"]);
        for _ in 0..50 {
            let tray = generate_tray(&crowded, &mut rng, 3000.0, false);
            assert!(is_solvable(&to_masks(&crowded), &tray, 10_000_000));
        }
    }

    #[test]
    fn counts_lines_that_only_pieces_placed_together_can_clear() {
        let empty = to_masks(&create_board());
        assert_eq!(tray_clears(&empty, &ids(&["I4-0", "I4-0", "dot-0"])), 1);
        assert_eq!(tray_clears(&empty, &ids(&["O2-0", "O2-0", "O2-0"])), 0);
        let b = board_from(&["####....", "####....", "", "", "", "", "", ""]);
        assert_eq!(tray_clears(&to_masks(&b), &ids(&["O2-0", "O2-0", "dot-0"])), 2);
    }

    #[test]
    fn deals_lucky_trays_that_clear_more_lines_than_plain_ones() {
        let b = board_from(&["###.##..", "##..###.", "#....##.", "", "", "", "##...###", "###..###"]);
        let masks = to_masks(&b);
        let clears = |lucky: bool| {
            let mut rng = Rng::new(3);
            (0..40).map(|_| tray_clears(&masks, &generate_tray(&b, &mut rng, 20_000.0, lucky))).sum::<u32>() as f64
        };
        assert!(clears(true) > clears(false) * 1.25);
    }

    #[test]
    fn makes_lucky_trays_rarer_as_the_score_rises_but_never_rarer_than_every_3rd() {
        assert_eq!(luck_every(0.0), 1);
        let steps: Vec<u32> = LUCK_LEVELS.iter().map(|l| l.every).collect();
        let mut sorted = steps.clone();
        sorted.sort();
        assert_eq!(steps, sorted);
        assert_eq!(luck_every(1e9), *steps.last().unwrap());
        assert!(*steps.iter().max().unwrap() <= 3);
        assert_eq!(luck_every(20_000.0), 2);
        assert_eq!([0, 1].map(|s| is_lucky_deal(20_000.0, 0.0, s)), [false, true]);
        assert_eq!(luck_every(100_000.0), 3);
        assert_eq!([0, 1, 2].map(|s| is_lucky_deal(100_000.0, 0.0, s)), [false, false, true]);
    }

    #[test]
    fn always_deals_a_lucky_tray_onto_a_crowded_board() {
        assert!(RESCUE_FILL <= 0.45);
        assert!(is_lucky_deal(1e6, RESCUE_FILL, 0));
        assert!(!is_lucky_deal(1e6, RESCUE_FILL - 0.1, 0));
    }

    #[test]
    fn makes_a_crowded_lucky_tray_clear_a_line_whenever_pieces_can() {
        let b = board_from(&[".#.#.#.#", ".#.#.#.#", ".#.#.#.#", ".#.#.#.#", "###.####", ".#.#.#.#", ".#.#.#.#", "#.#.#.#."]);
        assert!(35.0 / 64.0 >= RESCUE_FILL);
        let masks = to_masks(&b);
        let mut rng = Rng::new(11);
        for _ in 0..30 {
            let tray = generate_tray(&b, &mut rng, 1e6, true);
            assert!(is_solvable(&masks, &tray, 10_000_000));
            assert!(tray_clears(&masks, &tray) > 0);
        }
    }

    #[test]
    fn counts_boxed_in_holes() {
        assert_eq!(holes(&to_masks(&create_board())), 0);
        assert_eq!(holes(&to_masks(&board_from(&[".#", "#", "", "", "", "", "", ""]))), 1);
        assert_eq!(holes(&to_masks(&board_from(&["", "", ".#.#", "#.#.", "", "", "", ""]))), 0);
        assert_eq!(holes(&to_masks(&board_from(&["", ".#", "#.#", ".#", "", "", "", ""]))), 1);
    }

    #[test]
    fn values_trays_that_leave_fewer_holes_higher() {
        let masks = to_masks(&board_from(&[".##.....", "#.......", "", "", "", "", "", ""]));
        let with_dot = tray_value(&masks, &ids(&["dot-0", "I2-0", "I2-0"]));
        let without_dot = tray_value(&masks, &ids(&["I2-0", "I2-0", "I2-0"]));
        assert!(with_dot - without_dot > 1.0);
    }

    #[test]
    fn ramps_the_piece_mix_up_gently_and_caps_it() {
        let big = shape_by_id("O3-0").unwrap();
        let small = shape_by_id("O2-0").unwrap();
        let ratio = |score: f64| shape_weight(big, score, 0.0) / shape_weight(small, score, 0.0);
        assert!(ratio(10_000.0) > ratio(0.0));
        assert!(ratio(50_000.0) > ratio(10_000.0));
        assert_eq!(difficulty(0.0), 0.0);
        assert!(difficulty(30_000.0) < 0.25);
        assert!(difficulty(1e9) < 1.0);
        assert_eq!(ratio(1e9), ratio(1e12));
    }

    #[test]
    fn favours_small_pieces_more_so_on_a_crowded_board() {
        let big = shape_by_id("O3-0").unwrap();
        let small = shape_by_id("I3-0").unwrap();
        assert!(shape_weight(small, 0.0, 0.0) > shape_weight(big, 0.0, 0.0) * 2.0);
        assert!(shape_weight(small, 0.0, 0.6) / shape_weight(big, 0.0, 0.6) > shape_weight(small, 0.0, 0.0) / shape_weight(big, 0.0, 0.0));
    }
}
