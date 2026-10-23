/** The test and tool API of the web build (see web/index.html and src/platform/web.rs). */

interface BBPiece {
  id: string;
  rows: number;
  cols: number;
  color: number;
  cells: [number, number][];
}

interface BBRect {
  x: number;
  y: number;
  w: number;
  h: number;
  cx: number;
  cy: number;
}

interface BBState {
  score: number;
  shownScore: number;
  best: number;
  bestAtStart: number;
  moves: number;
  combo: number;
  over: boolean;
  lucky: boolean;
  boardClears: number;
  board: number[];
  tray: (BBPiece | null)[];
  layout: {
    width: number;
    height: number;
    cell: number;
    boardX: number;
    boardY: number;
    boardSize: number;
    trayCell: number;
    slots: { cx: number; cy: number; x: number; y: number; w: number; h: number }[];
  };
  settings: { sound: number; music: number; haptics: number };
  style: { key: string; tier: number; name: string };
  overlay: 'none' | 'menu' | 'gameover';
  record: boolean;
  helpOpen: boolean;
  /** The open card is still popping in. */
  popping: boolean;
  music: { playing: boolean; bpm: number; e: number; volume: number };
  soundVolume: number;
  stats: { sprites: number; particles: number; rings: number; beams: number; streaks: number; texts: number; animating: boolean; wasmMemory: number };
  ui: Record<'menuButton' | 'resume' | 'restart' | 'again' | 'exit' | 'help' | 'music' | 'sound' | 'haptics', BBRect | null>;
}

interface BBMove {
  points: number;
  lines: number;
  combo: number;
  boardCleared: boolean;
  gameOver: boolean;
  refilled: boolean;
}

interface BlockBaster {
  call(cmd: Record<string, unknown>): unknown;
  state(): BBState;
  debugStats(): BBState['stats'];
  canPlace(slot: number, r: number, c: number): boolean;
  commit(slot: number, r: number, c: number): BBMove | null;
  setTray(ids: (string | null)[], colors?: number[]): void;
  setBoard(board: number[] | number[][]): void;
  set(values: Partial<{ score: number; combo: number; moves: number; best: number; bestAtStart: number }>): void;
  settle(): void;
  updateStyle(delay?: number): void;
  newGame(seed?: number): void;
  serialize(): Record<string, unknown>;
  setSetting(key: 'sound' | 'music' | 'haptics', value: number): void;
  openMenu(): void;
  closeMenu(): void;
  invalidate(): void;
}

interface Window {
  blockBaster: BlockBaster;
}
