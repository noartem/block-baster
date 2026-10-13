import {
  type Board,
  type ClearedCell,
  type Lines,
  canPlace,
  canPlaceAnywhere,
  clearLines,
  createBoard,
  findFullLines,
  isEmpty,
  placeShape,
} from './board';
import { TRAY_SIZE, generateTray } from './generator';
import { Rng } from './rng';
import { type Shape, shapeById } from './shapes';

export const COLOR_COUNT = 7;
/** Placements without a clear after which the combo streak ends. */
export const COMBO_WINDOW = 3;
export const BOARD_CLEAR_BONUS = 300;

export interface Piece {
  shape: Shape;
  /** 1-based colour id. */
  color: number;
}

export interface PlaceResult {
  slot: number;
  piece: Piece;
  row: number;
  col: number;
  lines: Lines;
  cleared: ClearedCell[];
  /** Points gained by this move (placement + lines + bonuses). */
  points: number;
  linePoints: number;
  /** Current combo streak after this move (0 = no streak). */
  combo: number;
  boardCleared: boolean;
  /** True when a fresh tray was dealt after this move. */
  refilled: boolean;
  gameOver: boolean;
}

export interface SavedGame {
  v: 1;
  board: number[];
  tray: ({ id: string; color: number } | null)[];
  score: number;
  combo: number;
  sinceClear: number;
  moves: number;
  /** Times the whole board was cleared; picks the style variant. Missing in older saves. */
  clears?: number;
  rng: number;
  over: boolean;
}

/** Points for clearing `lines` lines at once with the given combo streak. */
export function linePoints(lines: number, combo: number): number {
  if (lines <= 0) return 0;
  return 10 * lines * (lines + 1) * Math.max(1, combo);
}

export class Game {
  board: Board = createBoard();
  tray: (Piece | null)[] = [];
  score = 0;
  combo = 0;
  sinceClear = 0;
  moves = 0;
  boardClears = 0;
  over = false;
  rng: Rng;

  constructor(seed?: number) {
    this.rng = new Rng(seed);
    this.deal();
  }

  /** Deals a new set of pieces into the tray. */
  private deal(): void {
    const shapes = generateTray(this.board, this.rng, this.score);
    const used = new Set<number>();
    this.tray = shapes.map((shape) => {
      // Prefer distinct colours inside one tray.
      let color = 1 + this.rng.int(COLOR_COUNT);
      for (let i = 0; i < 4 && used.has(color); i++) color = 1 + this.rng.int(COLOR_COUNT);
      used.add(color);
      return { shape, color };
    });
  }

  canPlace(slot: number, row: number, col: number): boolean {
    const piece = this.tray[slot];
    return !!piece && !this.over && canPlace(this.board, piece.shape, row, col);
  }

  /** Whether the piece in the slot fits anywhere on the board right now. */
  isPlayable(slot: number): boolean {
    const piece = this.tray[slot];
    return !!piece && canPlaceAnywhere(this.board, piece.shape);
  }

  hasMoves(): boolean {
    return this.tray.some((_, i) => this.isPlayable(i));
  }

  place(slot: number, row: number, col: number): PlaceResult | null {
    if (!this.canPlace(slot, row, col)) return null;
    const piece = this.tray[slot]!;
    placeShape(this.board, piece.shape, row, col, piece.color);
    this.tray[slot] = null;
    this.moves++;

    const lines = findFullLines(this.board);
    const count = lines.rows.length + lines.cols.length;
    const cleared = clearLines(this.board, lines);
    if (count > 0) {
      this.combo++;
      this.sinceClear = 0;
    } else if (++this.sinceClear >= COMBO_WINDOW) {
      this.combo = 0;
    }

    const boardCleared = count > 0 && isEmpty(this.board);
    if (boardCleared) this.boardClears++;
    const lp = linePoints(count, this.combo);
    const points = piece.shape.cells.length + lp + (boardCleared ? BOARD_CLEAR_BONUS : 0);
    this.score += points;

    let refilled = false;
    if (this.tray.every((p) => p === null)) {
      this.deal();
      refilled = true;
    }
    this.over = !this.hasMoves();

    return {
      slot,
      piece,
      row,
      col,
      lines,
      cleared,
      points,
      linePoints: lp,
      combo: count > 0 ? this.combo : 0,
      boardCleared,
      refilled,
      gameOver: this.over,
    };
  }

  serialize(): SavedGame {
    return {
      v: 1,
      board: Array.from(this.board),
      tray: this.tray.map((p) => (p ? { id: p.shape.id, color: p.color } : null)),
      score: this.score,
      combo: this.combo,
      sinceClear: this.sinceClear,
      moves: this.moves,
      clears: this.boardClears,
      rng: this.rng.state,
      over: this.over,
    };
  }

  static deserialize(data: SavedGame): Game {
    if (data?.v !== 1 || data.board.length !== 64 || data.tray.length !== TRAY_SIZE) {
      throw new Error('Unsupported save');
    }
    const game = new Game(0);
    game.board = Uint8Array.from(data.board);
    game.tray = data.tray.map((p) => (p ? { shape: shapeById(p.id), color: p.color } : null));
    game.score = data.score;
    game.combo = data.combo;
    game.sinceClear = data.sinceClear;
    game.moves = data.moves;
    game.boardClears = data.clears ?? 0;
    game.rng = new Rng(data.rng);
    game.over = data.over;
    return game;
  }
}
