import { describe, expect, it } from 'vitest';
import {
  ActionMeter,
  BPM_MAX,
  BPM_MIN,
  IntensityTracker,
  SPEED_MAX_MOVES,
  SPEED_WINDOW_MS,
  TARGET_MAX,
  TARGET_MIN,
  bpmFor,
  intensity,
  ramp,
  scoreProgress,
} from '../../src/audio/intensity';
import { chordAt } from '../../src/audio/music';

describe('music intensity', () => {
  it('ramps linearly between two thresholds', () => {
    expect(ramp(0.1, 0.2, 0.4)).toBe(0);
    expect(ramp(0.3, 0.2, 0.4)).toBeCloseTo(0.5);
    expect(ramp(0.9, 0.2, 0.4)).toBe(1);
  });

  it('measures score progress against the best score, within bounds', () => {
    expect(scoreProgress(0, 0)).toBe(0);
    expect(scoreProgress(TARGET_MIN / 2, 0)).toBeCloseTo(0.5);
    expect(scoreProgress(1500, 3000)).toBeCloseTo(0.5);
    expect(scoreProgress(TARGET_MAX, 1e6)).toBe(1);
    expect(scoreProgress(5000, 2000)).toBe(1);
  });

  it('weighs progress over speed, and needs both for full intensity', () => {
    expect(intensity(0, 0)).toBe(0);
    expect(intensity(1, 0)).toBeGreaterThan(intensity(0, 1));
    expect(intensity(1, 0)).toBeLessThan(1);
    expect(intensity(1, 1)).toBe(1);
  });

  it('keeps the tempo low for a while and reaches the maximum at full intensity', () => {
    expect(bpmFor(0)).toBe(BPM_MIN);
    expect(bpmFor(1)).toBe(BPM_MAX);
    expect(bpmFor(0.5)).toBeLessThan((BPM_MIN + BPM_MAX) / 2);
  });

  it('counts moves within the window and adds combos on top', () => {
    const m = new ActionMeter();
    expect(m.speed(0)).toBe(0);
    for (let i = 0; i < SPEED_MAX_MOVES; i++) m.move(i * 500, 0);
    expect(m.speed(SPEED_MAX_MOVES * 500)).toBe(1);
    m.reset();
    m.move(0, 0);
    m.move(100, 2);
    expect(m.speed(200)).toBeCloseTo(2 / SPEED_MAX_MOVES + 0.2);
    // Old moves (and their combo) drop out of the window.
    expect(m.speed(100 + SPEED_WINDOW_MS + 1)).toBe(0);
  });

  it('follows the inputs smoothly: speed rises fast and falls slowly', () => {
    const t = new IntensityTracker();
    t.update(0.5, 0, 1);
    const risen = t.state.speed;
    expect(risen).toBeGreaterThan(0.6);
    t.update(0.5, 0, 0);
    expect(t.state.speed).toBeGreaterThan(risen * 0.8);
    for (let i = 0; i < 300; i++) t.update(0.1, 1, 1);
    expect(t.state.e).toBeCloseTo(1, 3);
    expect(t.state.bpm).toBeCloseTo(BPM_MAX, 1);
  });

  it('plays the progression over 8 bars with two chords in the last one', () => {
    expect([0, 1, 2, 3, 4, 5, 6, 7].map((bar) => chordAt(bar, 0))).toEqual([0, 1, 2, 3, 4, 5, 6, 7]);
    expect(chordAt(7, 8)).toBe(8);
    expect(chordAt(15, 15)).toBe(8);
    expect(chordAt(8, 0)).toBe(0);
  });
});
