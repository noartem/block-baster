/**
 * Turns the state of the game into the two numbers the soundtrack follows:
 * score progress and how fast the player is moving. Kept free of Web Audio so
 * it can be unit-tested.
 */

/** Tempo of the calm lofi start and of the full EDM peak. */
export const BPM_MIN = 76;
export const BPM_MAX = 150;
/** Above 1 the tempo stays low for longer and speeds up near the end; 1 is linear. */
export const BPM_CURVE = 1.6;

/** Weights of score progress and action speed in the overall intensity. */
export const W_SCORE = 0.65;
export const W_SPEED = 0.35;

/** Score at which the music reaches full EDM: the best score, within these bounds. */
export const TARGET_MIN = 1000;
export const TARGET_MAX = 4000;

/** Moves within this window count towards the action speed... */
export const SPEED_WINDOW_MS = 6000;
/** ...and this many moves in it is full speed. */
export const SPEED_MAX_MOVES = 5;
/** Each step of a combo streak adds this much speed. */
export const SPEED_PER_COMBO = 0.1;

/** Smoothing time constants, in seconds. Speed rises fast and falls slowly so a short pause doesn't drop the music back to lofi. */
export const SMOOTH_SCORE = 2;
export const SMOOTH_SPEED_UP = 0.35;
export const SMOOTH_SPEED_DOWN = 3.3;

export const clamp01 = (x: number): number => Math.min(1, Math.max(0, Number(x) || 0));

/** 0 below `a`, 1 above `b`, linear in between. */
export const ramp = (x: number, a: number, b: number): number => clamp01((x - a) / (b - a));

/** How far the score has come towards the full-energy target, 0..1. */
export function scoreProgress(score: number, best: number): number {
  const target = Math.min(TARGET_MAX, Math.max(TARGET_MIN, best));
  return clamp01(score / target);
}

/** Overall intensity 0..1: progress sets the stage, speed adds energy on top. */
export function intensity(score: number, speed: number): number {
  return clamp01(W_SCORE * clamp01(score) + W_SPEED * clamp01(speed));
}

export function bpmFor(e: number): number {
  return BPM_MIN + (BPM_MAX - BPM_MIN) * clamp01(e) ** BPM_CURVE;
}

/** Counts recent moves to measure how fast the player is acting. */
export class ActionMeter {
  private moves: number[] = [];
  private combo = 0;

  move(now: number, combo: number): void {
    this.moves.push(now);
    this.combo = combo;
    this.trim(now);
  }

  reset(): void {
    this.moves.length = 0;
    this.combo = 0;
  }

  speed(now: number): number {
    this.trim(now);
    const combo = this.moves.length ? Math.min(this.combo, 5) : 0;
    return clamp01(this.moves.length / SPEED_MAX_MOVES + SPEED_PER_COMBO * combo);
  }

  private trim(now: number): void {
    while (this.moves.length && now - this.moves[0] > SPEED_WINDOW_MS) this.moves.shift();
  }
}

export interface MusicState {
  /** Overall intensity 0..1: picks the genre and the tempo. */
  e: number;
  /** Smoothed action speed 0..1: adds small details on top. */
  speed: number;
  bpm: number;
}

/** Smooths the raw inputs over time and derives the music state from them. */
export class IntensityTracker {
  private score = 0;
  private speed = 0;
  state: MusicState = { e: 0, speed: 0, bpm: BPM_MIN };

  /** Advances by `dt` seconds towards the raw `score` and `speed` (both 0..1). */
  update(dt: number, score: number, speed: number): MusicState {
    const follow = (from: number, to: number, tau: number) => from + (to - from) * (1 - Math.exp(-Math.max(0, dt) / tau));
    this.score = follow(this.score, clamp01(score), SMOOTH_SCORE);
    const target = clamp01(speed);
    this.speed = follow(this.speed, target, target > this.speed ? SMOOTH_SPEED_UP : SMOOTH_SPEED_DOWN);
    const e = intensity(this.score, this.speed);
    this.state = { e, speed: this.speed, bpm: bpmFor(e) };
    return this.state;
  }

  /** Jumps straight to a state (for previews and tests). */
  set(score: number, speed: number): MusicState {
    this.score = clamp01(score);
    this.speed = clamp01(speed);
    return this.update(0, this.score, this.speed);
  }
}
