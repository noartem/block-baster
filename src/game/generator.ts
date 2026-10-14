import { type Board, type Masks, SIZE, anyFit, filledCount, fitsMask, placeMask, toMasks } from './board';
import type { Rng } from './rng';
import { SHAPES, type Shape } from './shapes';

export const TRAY_SIZE = 3;

/** Score at which the piece mix reaches its hardest distribution. */
const HARDEST_AT = 30_000;
const CANDIDATE_ATTEMPTS = 40;
/** Solvable trays a lucky deal chooses the best one from. */
const LUCKY_CANDIDATES = 12;
const SOLVER_BUDGET = 25_000;

/**
 * How often the dealer gives a hand. Below `until` points every `every`-th
 * tray is a lucky one: picked out of several random trays for the most lines
 * it clears and the most room it leaves. The rest are plain random trays.
 */
export const LUCK_LEVELS = [
  { until: 2_000, every: 1 },
  { until: 10_000, every: 2 },
  { until: 50_000, every: 3 },
  { until: Infinity, every: 4 },
] as const;

/** When the board is at least this full, the next tray is always a lucky one. */
export const RESCUE_FILL = 0.5;

/** Every how many trays a lucky one comes at this score. */
export function luckEvery(score: number): number {
  return LUCK_LEVELS.find((l) => score < l.until)!.every;
}

/**
 * Whether the next tray should be a lucky one, given the score, how full the
 * board is (0..1) and how many plain trays were dealt since the last lucky one.
 */
export function isLuckyDeal(score: number, fill: number, sinceLucky: number): boolean {
  return sinceLucky + 1 >= luckEvery(score) || fill >= RESCUE_FILL;
}

/** Weight of a shape given the current game situation. */
export function shapeWeight(shape: Shape, score: number, fill: number): number {
  const d = Math.min(1, score / HARDEST_AT);
  // Crowded boards get smaller pieces more often, so a run doesn't end too abruptly.
  const crowded = Math.max(0, fill - 0.45) * 2;
  const tierFactor = [1.25 - 0.45 * d + crowded, 1, 0.55 + 0.65 * d - crowded * 0.4][shape.tier];
  return shape.weight * Math.max(0.05, tierFactor);
}

/**
 * Returns true if every shape can be placed on the board in some order
 * (accounting for lines cleared between placements). Returns false when the
 * search budget runs out, so callers treat "unknown" as "not solvable".
 */
export function isSolvable(masks: Masks, shapes: readonly Shape[], budget = SOLVER_BUDGET): boolean {
  let nodes = 0;
  const solve = (m: Masks, rest: readonly Shape[]): boolean => {
    if (rest.length === 0) return true;
    const tried = new Set<string>();
    for (let i = 0; i < rest.length; i++) {
      const shape = rest[i];
      if (tried.has(shape.id)) continue;
      tried.add(shape.id);
      const others = rest.filter((_, j) => j !== i);
      for (let r = 0; r + shape.rows <= SIZE; r++) {
        for (let c = 0; c + shape.cols <= SIZE; c++) {
          if (!fitsMask(m, shape, r, c)) continue;
          if (++nodes > budget) return false;
          if (others.length === 0) return true;
          if (solve(placeMask(m, shape, r, c).masks, others)) return true;
        }
      }
    }
    return false;
  };
  return solve(masks, shapes);
}

const WALLS = 0b10_0000_0001;
const popcount = (x: number) => {
  let n = 0;
  for (; x; x &= x - 1) n++;
  return n;
};

/** How many filled cells or walls touch the shape's cells when placed at (r, c). */
function contact(masks: Masks, shape: Shape, r: number, c: number): number {
  // Rows padded with a wall bit on each side; rows outside the board are all wall.
  const row = (i: number) => (i < 0 || i >= SIZE ? 0x3ff : (masks[i] << 1) | WALLS);
  let n = 0;
  shape.rowMasks.forEach((m, i) => {
    const cells = m << (c + 1);
    const here = row(r + i);
    n += popcount(cells & row(r + i - 1)) + popcount(cells & row(r + i + 1)) + popcount((cells >> 1) & here) + popcount((cells << 1) & here);
  });
  return n;
}

/**
 * Plays the tray out: its pieces placed one after another, in the best order,
 * each at the spot that clears most (or fits snuggest). Returns the lines
 * cleared and the cells left filled. This catches pieces that only clear
 * together, like two I4s on an empty row.
 */
export function playOut(masks: Masks, tray: readonly Shape[]): { lines: number; filled: number } {
  let best = { lines: 0, filled: Infinity };
  const run = (m: Masks, rest: readonly Shape[], lines: number) => {
    if (rest.length === 0) {
      const filled = m.reduce((n, row) => n + popcount(row), 0);
      if (lines > best.lines || (lines === best.lines && filled < best.filled)) best = { lines, filled };
      return;
    }
    for (let i = 0; i < rest.length; i++) {
      const shape = rest[i];
      if (rest.findIndex((s) => s.id === shape.id) !== i) continue;
      let pick: { masks: Masks; lines: number; contact: number } | null = null;
      for (let r = 0; r + shape.rows <= SIZE; r++)
        for (let c = 0; c + shape.cols <= SIZE; c++) {
          if (!fitsMask(m, shape, r, c)) continue;
          const res = placeMask(m, shape, r, c);
          const touch = contact(m, shape, r, c);
          if (!pick || res.lines > pick.lines || (res.lines === pick.lines && touch > pick.contact)) pick = { ...res, contact: touch };
        }
      if (pick) run(pick.masks, rest.filter((_, j) => j !== i), lines + pick.lines);
    }
  };
  run(masks, tray, 0);
  return best;
}

/** Lines the tray clears when played out (see {@link playOut}). */
export const trayClears = (masks: Masks, tray: readonly Shape[]) => playOut(masks, tray).lines;

function randomTray(rng: Rng, weight: (s: Shape) => number): Shape[] {
  const tray: Shape[] = [];
  while (tray.length < TRAY_SIZE) {
    const shape = rng.weighted(SHAPES, weight);
    // Avoid dealing three copies of the same piece.
    if (tray.length === 2 && tray[0].id === shape.id && tray[1].id === shape.id) continue;
    tray.push(shape);
  }
  return tray;
}

/**
 * Builds a tray piece by piece, only choosing shapes that still fit after the
 * previous ones were placed at their best spot. Always solvable when at least
 * one shape fits the board.
 */
function constructiveTray(masks: Masks, rng: Rng, weight: (s: Shape) => number): Shape[] {
  const tray: Shape[] = [];
  let m = masks;
  while (tray.length < TRAY_SIZE) {
    const fitting = SHAPES.filter((s) => anyFit(m, s));
    if (fitting.length === 0) {
      tray.push(rng.weighted(SHAPES, weight));
      continue;
    }
    const shape = rng.weighted(fitting, weight);
    tray.push(shape);
    let best: { masks: Masks; lines: number } | null = null;
    for (let r = 0; r + shape.rows <= SIZE; r++)
      for (let c = 0; c + shape.cols <= SIZE; c++)
        if (fitsMask(m, shape, r, c)) {
          const res = placeMask(m, shape, r, c);
          if (!best || res.lines > best.lines) best = res;
        }
    m = best!.masks;
  }
  return tray;
}

/**
 * Deals a new tray of pieces. The tray is guaranteed to be placeable (in some
 * order) whenever that is possible at all, mirroring how Block Blast feels:
 * you only lose through your own placement choices.
 *
 * A lucky tray (see {@link isLuckyDeal}) is the best of several random ones:
 * most lines cleared when played out, then the emptiest board left behind.
 */
export function generateTray(board: Board, rng: Rng, score: number, lucky = false): Shape[] {
  const masks = toMasks(board);
  const fill = filledCount(board) / (SIZE * SIZE);
  const weight = (s: Shape) => shapeWeight(s, score, fill);

  let chosen: Shape[] | null = null;
  let chosenValue = -Infinity;
  let found = 0;
  for (let attempt = 0; attempt < CANDIDATE_ATTEMPTS; attempt++) {
    const tray = randomTray(rng, weight);
    if (!isSolvable(masks, tray)) continue;
    if (!lucky) return tray;
    const { lines, filled } = playOut(masks, tray);
    const value = lines * SIZE * SIZE - filled;
    if (!chosen || value > chosenValue) {
      chosen = tray;
      chosenValue = value;
    }
    if (++found >= LUCKY_CANDIDATES) break;
  }
  return chosen ?? constructiveTray(masks, rng, weight);
}
