import { describe, expect, it } from 'vitest';
import { computeLayout, slotAt } from '../../src/render/layout';

describe('layout', () => {
  for (const [w, h] of [
    [360, 640],
    [390, 760],
    [412, 870],
    [768, 1000],
    [1280, 720],
    [320, 480],
  ]) {
    it(`fits board and tray inside ${w}x${h}`, () => {
      const l = computeLayout(w, h);
      expect(l.boardSize % 8).toBe(0);
      expect(l.boardX).toBeGreaterThanOrEqual(0);
      expect(l.boardX + l.boardSize).toBeLessThanOrEqual(w);
      expect(l.boardY).toBeGreaterThanOrEqual(0);
      for (const s of l.slots) {
        // Slots sit below the board and hold a 5-long piece at tray scale.
        expect(s.cy).toBeGreaterThan(l.boardY + l.boardSize);
        expect(s.cy + l.trayCell * 2.5).toBeLessThanOrEqual(h);
        expect(l.trayCell * 5).toBeLessThanOrEqual(s.w);
      }
      expect(l.slots[0].x + l.slots[0].w).toBeLessThanOrEqual(l.slots[1].x + 0.001);
    });
  }

  it('hit-tests tray slots', () => {
    const l = computeLayout(390, 760);
    expect(slotAt(l, l.slots[1].cx, l.slots[1].cy)).toBe(1);
    expect(slotAt(l, l.boardX + 5, l.boardY + 5)).toBe(-1);
  });
});
