import { type Board, type Masks, SIZE, anyFit, filledCount, fitsMask, placeMask, toMasks } from './board';
import type { Rng } from './rng';
import { SHAPES, type Shape } from './shapes';

export const TRAY_SIZE = 3;

/** Score at which the piece mix reaches its hardest distribution. */
const HARDEST_AT = 150_000;
/**
 * How far towards the hard piece mix the game ever gets (0..1). Capped below
 * 1 so even very long runs never see big pieces outnumber medium ones.
 */
const MAX_DIFFICULTY = 0.85;
const CANDIDATE_ATTEMPTS = 60;
/** Solvable trays a lucky deal chooses the best one from. */
const LUCKY_CANDIDATES = 16;
/** Solvable trays a plain deal looks at to find one that leaves enough room. */
const PLAIN_CANDIDATES = 4;
/** A plain tray should leave at most this many cells filled when played out well. */
const ROOMY_FILLED = 40;
const SOLVER_BUDGET = 25_000;

/**
 * How often the dealer gives a hand. Below `until` points every `every`-th
 * tray is a lucky one: picked out of many random trays for the most lines it
 * clears and the most room it leaves. The rest are plain random trays.
 * Lucky trays never get rarer than every 3rd.
 */
export const LUCK_LEVELS = [
  { until: 3_000, every: 1 },
  { until: 40_000, every: 2 },
  { until: Infinity, every: 3 },
] as const;

/** When the board is at least this full, the next tray is always a lucky one. */
export const RESCUE_FILL = 0.45;

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

/** How hard the piece mix is at this score, from 0 up to {@link MAX_DIFFICULTY}. */
export function difficulty(score: number): number {
  return MAX_DIFFICULTY * Math.min(1, Math.max(0, score) / HARDEST_AT);
}

/** Weight of a shape given the current game situation. */
export function shapeWeight(shape: Shape, score: number, fill: number): number {
  const d = difficulty(score);
  // Crowded boards get smaller pieces more often, so a run doesn't end too abruptly.
  const crowded = Math.max(0, fill - 0.35) * 2.5;
  const tierFactor = [1.3 - 0.4 * d + crowded, 1, 0.5 + 0.6 * d - crowded * 0.5][shape.tier];
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

const FULL = (1 << SIZE) - 1;

/**
 * Empty cells boxed in on all four sides (by blocks or walls). Only a single
 * dot can ever fill them, so they make a board much harder to play.
 */
export function holes(masks: Masks): number {
  let n = 0;
  for (let r = 0; r < SIZE; r++) {
    const above = r === 0 ? FULL : masks[r - 1];
    const below = r === SIZE - 1 ? FULL : masks[r + 1];
    const row = masks[r];
    const left = ((row << 1) | 1) & FULL;
    const right = (row >> 1) | (1 << (SIZE - 1));
    n += popcount(~row & above & below & left & right & FULL);
  }
  return n;
}

export interface PlayOut {
  /** Lines cleared over the whole tray. */
  lines: number;
  /** Cells left filled afterwards. */
  filled: number;
  /** The board afterwards. */
  masks: Masks;
}

/**
 * Plays the tray out: its pieces placed one after another, in the best order,
 * each at the spot that clears most (or fits snuggest). Returns the lines
 * cleared and the board left behind. This catches pieces that only clear
 * together, like two I4s on an empty row.
 */
export function playOut(masks: Masks, tray: readonly Shape[]): PlayOut {
  let best: PlayOut = { lines: 0, filled: Infinity, masks };
  const run = (m: Masks, rest: readonly Shape[], lines: number) => {
    if (rest.length === 0) {
      const filled = m.reduce((n, row) => n + popcount(row), 0);
      if (lines > best.lines || (lines === best.lines && filled < best.filled)) best = { lines, filled, masks: m };
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

/**
 * How good a tray is for the player: lines cleared count most, then how much
 * room is left, with boxed-in holes counting as extra filled cells.
 */
const valueOf = (out: PlayOut) => out.lines * SIZE * SIZE - out.filled - 3 * holes(out.masks);
export const trayValue = (masks: Masks, tray: readonly Shape[]) => valueOf(playOut(masks, tray));

/** Whether some fitting placement of the shape completes a line. */
function clearsLine(masks: Masks, shape: Shape): boolean {
  for (let r = 0; r + shape.rows <= SIZE; r++)
    for (let c = 0; c + shape.cols <= SIZE; c++)
      if (fitsMask(masks, shape, r, c) && placeMask(masks, shape, r, c).lines > 0) return true;
  return false;
}

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
 * one shape fits the board. With `clearing`, it picks a shape that completes a
 * line whenever one exists, so the tray is sure to clear something.
 */
function constructiveTray(masks: Masks, rng: Rng, weight: (s: Shape) => number, clearing = false): Shape[] {
  const tray: Shape[] = [];
  let m = masks;
  while (tray.length < TRAY_SIZE) {
    const fitting = SHAPES.filter((s) => anyFit(m, s));
    if (fitting.length === 0) {
      tray.push(rng.weighted(SHAPES, weight));
      continue;
    }
    const clearers = clearing ? fitting.filter((s) => clearsLine(m, s)) : [];
    const shape = rng.weighted(clearers.length > 0 ? clearers : fitting, weight);
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
 * A plain tray is a random one that, played out well, still leaves room on
 * the board (when such a tray turns up quickly). A lucky tray (see
 * {@link isLuckyDeal}) is the best of many random ones by {@link trayValue}:
 * most lines cleared, then the emptiest board left behind. On a crowded board
 * a lucky tray always clears at least one line when any piece can.
 */
export function generateTray(board: Board, rng: Rng, score: number, lucky = false): Shape[] {
  const masks = toMasks(board);
  const fill = filledCount(board) / (SIZE * SIZE);
  const weight = (s: Shape) => shapeWeight(s, score, fill);
  const wanted = lucky ? LUCKY_CANDIDATES : PLAIN_CANDIDATES;

  let chosen: Shape[] | null = null;
  let chosenValue = -Infinity;
  let chosenLines = 0;
  let found = 0;
  for (let attempt = 0; attempt < CANDIDATE_ATTEMPTS && found < wanted; attempt++) {
    const tray = randomTray(rng, weight);
    if (!isSolvable(masks, tray)) continue;
    found++;
    const out = playOut(masks, tray);
    if (!lucky && out.filled <= ROOMY_FILLED) return tray;
    const value = lucky ? valueOf(out) : -out.filled;
    if (!chosen || value > chosenValue) {
      chosen = tray;
      chosenValue = value;
      chosenLines = out.lines;
    }
  }
  if (lucky && fill >= RESCUE_FILL && chosenLines === 0) {
    // Nothing random cleared a line: build a tray around a piece that does.
    const tray = constructiveTray(masks, rng, weight, true);
    if (trayClears(masks, tray) > 0) return tray;
  }
  return chosen ?? constructiveTray(masks, rng, weight);
}
