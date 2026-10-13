import { type Board, type Masks, SIZE, anyFit, filledCount, fitsMask, placeMask, toMasks } from './board';
import type { Rng } from './rng';
import { SHAPES, type Shape } from './shapes';

export const TRAY_SIZE = 3;

/** Score at which the piece mix reaches its hardest distribution. */
const HARDEST_AT = 6000;
const CANDIDATE_ATTEMPTS = 30;
const SOLVER_BUDGET = 25_000;

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

/** Highest number of lines the shape can clear with a single placement. */
export function bestClear(masks: Masks, shape: Shape): number {
  let best = 0;
  for (let r = 0; r + shape.rows <= SIZE; r++)
    for (let c = 0; c + shape.cols <= SIZE; c++)
      if (fitsMask(masks, shape, r, c)) best = Math.max(best, placeMask(masks, shape, r, c).lines);
  return best;
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
 */
export function generateTray(board: Board, rng: Rng, score: number): Shape[] {
  const masks = toMasks(board);
  const fill = filledCount(board) / (SIZE * SIZE);
  const weight = (s: Shape) => shapeWeight(s, score, fill);
  // Sometimes prefer a tray that lets the player clear lines right away.
  const assist = rng.next() < 0.45 - 0.25 * Math.min(1, score / HARDEST_AT);

  let chosen: Shape[] | null = null;
  let chosenValue = -1;
  let found = 0;
  for (let attempt = 0; attempt < CANDIDATE_ATTEMPTS; attempt++) {
    const tray = randomTray(rng, weight);
    if (!isSolvable(masks, tray)) continue;
    if (!assist) return tray;
    const value = tray.reduce((sum, s) => sum + bestClear(masks, s), 0);
    if (value > chosenValue) {
      chosen = tray;
      chosenValue = value;
    }
    if (++found >= 4) break;
  }
  return chosen ?? constructiveTray(masks, rng, weight);
}
