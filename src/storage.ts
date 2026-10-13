import type { SavedGame } from './game/game';

const KEYS = { best: 'bb.best', bestAtStart: 'bb.bestAtStart', game: 'bb.game', settings: 'bb.settings' } as const;

export interface Settings {
  sound: boolean;
  music: boolean;
  haptics: boolean;
}

const DEFAULT_SETTINGS: Settings = { sound: true, music: true, haptics: true };

function read<T>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : null;
  } catch {
    return null;
  }
}

function write(key: string, value: unknown): void {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Storage may be unavailable (private mode, quota); the game still works.
  }
}

export const storage = {
  loadBest: (): number => Number(read<number>(KEYS.best)) || 0,
  saveBest: (best: number) => write(KEYS.best, best),
  /** Best score when the current game started, to tell a new record from a tie. */
  loadBestAtStart: (): number | null => read<number>(KEYS.bestAtStart),
  saveBestAtStart: (best: number) => write(KEYS.bestAtStart, best),
  loadGame: (): SavedGame | null => read<SavedGame>(KEYS.game),
  saveGame: (game: SavedGame | null) => write(KEYS.game, game),
  loadSettings: (): Settings => ({ ...DEFAULT_SETTINGS, ...read<Partial<Settings>>(KEYS.settings) }),
  saveSettings: (s: Settings) => write(KEYS.settings, s),
};
