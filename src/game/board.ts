import type { Shape } from './shapes';

export const SIZE = 8;
const FULL_ROW = (1 << SIZE) - 1;

/**
 * The board is a flat array of SIZE*SIZE cells. 0 means empty, any other value
 * is the colour id (1-based) of the block occupying that cell.
 */
export type Board = Uint8Array;

export interface Lines {
  rows: number[];
  cols: number[];
}

export interface ClearedCell {
  r: number;
  c: number;
  color: number;
}

export const createBoard = (): Board => new Uint8Array(SIZE * SIZE);

export const idx = (r: number, c: number) => r * SIZE + c;

export function canPlace(board: Board, shape: Shape, r: number, c: number): boolean {
  if (r < 0 || c < 0 || r + shape.rows > SIZE || c + shape.cols > SIZE) return false;
  for (const [dr, dc] of shape.cells) if (board[idx(r + dr, c + dc)] !== 0) return false;
  return true;
}

/** Writes the shape into the board. Caller must check canPlace first. */
export function placeShape(board: Board, shape: Shape, r: number, c: number, color: number): void {
  for (const [dr, dc] of shape.cells) board[idx(r + dr, c + dc)] = color;
}

export function findFullLines(board: Board): Lines {
  const rows: number[] = [];
  const cols: number[] = [];
  for (let r = 0; r < SIZE; r++) {
    let full = true;
    for (let c = 0; c < SIZE && full; c++) full = board[idx(r, c)] !== 0;
    if (full) rows.push(r);
  }
  for (let c = 0; c < SIZE; c++) {
    let full = true;
    for (let r = 0; r < SIZE && full; r++) full = board[idx(r, c)] !== 0;
    if (full) cols.push(c);
  }
  return { rows, cols };
}

/** Empties the given rows/cols and returns the removed cells (each cell once). */
export function clearLines(board: Board, lines: Lines): ClearedCell[] {
  const cleared: ClearedCell[] = [];
  const mark = (r: number, c: number) => {
    const i = idx(r, c);
    if (board[i] !== 0) {
      cleared.push({ r, c, color: board[i] });
      board[i] = 0;
    }
  };
  for (const r of lines.rows) for (let c = 0; c < SIZE; c++) mark(r, c);
  for (const c of lines.cols) for (let r = 0; r < SIZE; r++) mark(r, c);
  return cleared;
}

/** Lines that would be completed if the shape were placed at (r, c). */
export function previewLines(board: Board, shape: Shape, r: number, c: number): Lines {
  const copy = board.slice();
  placeShape(copy, shape, r, c, 1);
  return findFullLines(copy);
}

export function canPlaceAnywhere(board: Board, shape: Shape): boolean {
  return anyFit(toMasks(board), shape);
}

export function isEmpty(board: Board): boolean {
  return board.every((v) => v === 0);
}

export function filledCount(board: Board): number {
  let n = 0;
  for (const v of board) if (v !== 0) n++;
  return n;
}

// ---------------------------------------------------------------------------
// Bitmask representation used by the (hot) solver: one 8-bit mask per row.
// ---------------------------------------------------------------------------

export type Masks = number[];

export function toMasks(board: Board): Masks {
  const masks = new Array<number>(SIZE).fill(0);
  for (let r = 0; r < SIZE; r++)
    for (let c = 0; c < SIZE; c++) if (board[idx(r, c)] !== 0) masks[r] |= 1 << c;
  return masks;
}

export function fitsMask(masks: Masks, shape: Shape, r: number, c: number): boolean {
  const rm = shape.rowMasks;
  for (let i = 0; i < rm.length; i++) if ((masks[r + i] & (rm[i] << c)) !== 0) return false;
  return true;
}

export function anyFit(masks: Masks, shape: Shape): boolean {
  for (let r = 0; r + shape.rows <= SIZE; r++)
    for (let c = 0; c + shape.cols <= SIZE; c++) if (fitsMask(masks, shape, r, c)) return true;
  return false;
}

/** Places a shape on a copy of the masks, clears full lines and returns the result. */
export function placeMask(masks: Masks, shape: Shape, r: number, c: number): { masks: Masks; lines: number } {
  const next = masks.slice();
  const rm = shape.rowMasks;
  for (let i = 0; i < rm.length; i++) next[r + i] |= rm[i] << c;
  let colFull = FULL_ROW;
  for (let i = 0; i < SIZE; i++) colFull &= next[i];
  let lines = 0;
  for (let i = 0; i < SIZE; i++) {
    if (next[i] === FULL_ROW) {
      next[i] = 0;
      lines++;
    }
  }
  if (colFull) {
    for (let i = 0; i < SIZE; i++) next[i] &= ~colFull;
    for (let b = colFull; b; b &= b - 1) lines++;
  }
  return { masks: next, lines };
}
