import type { SavedGame } from './game/game';

const KEYS = { best: 'bb.best', bestAtStart: 'bb.bestAtStart', game: 'bb.game', settings: 'bb.settings' } as const;

/** Levels 0..1; 0 turns the channel off. */
export interface Settings {
  sound: number;
  music: number;
  /** Vibration strength: 0.5 is normal. */
  haptics: number;
}

export const DEFAULT_SETTINGS: Settings = { sound: 1, music: 1, haptics: 0.5 };

/** Reads a level, upgrading the on/off switches of older versions. */
function level(value: unknown, fallback: number): number {
  if (typeof value === 'boolean') return value ? fallback : 0;
  return typeof value === 'number' && Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : fallback;
}

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
  loadSettings: (): Settings => {
    const saved = read<Record<keyof Settings, unknown>>(KEYS.settings);
    return {
      sound: level(saved?.sound, DEFAULT_SETTINGS.sound),
      music: level(saved?.music, DEFAULT_SETTINGS.music),
      haptics: level(saved?.haptics, DEFAULT_SETTINGS.haptics),
    };
  },
  saveSettings: (s: Settings) => write(KEYS.settings, s),
};
