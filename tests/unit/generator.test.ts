import { describe, expect, it } from 'vitest';
import { createBoard, toMasks } from '../../src/game/board';
import { OPENING_UNTIL, TRAY_SIZE, bestClear, generateTray, isSolvable, shapeWeight, trayClears } from '../../src/game/generator';
import { Rng } from '../../src/game/rng';
import { shapeById } from '../../src/game/shapes';
import { boardFrom } from './helpers';

describe('generator', () => {
  it('is deterministic for a given seed', () => {
    const a = generateTray(createBoard(), new Rng(42), 0).map((s) => s.id);
    const b = generateTray(createBoard(), new Rng(42), 0).map((s) => s.id);
    expect(a).toEqual(b);
    expect(a).toHaveLength(TRAY_SIZE);
  });

  it('solver accounts for line clears between placements', () => {
    // O3 only fits once the bottom row is completed and cleared.
    const b = boardFrom([
      '#######.',
      '#######.',
      '#######.',
      '#######.',
      '######..',
      '...#####',
      '...#####',
      '###.....',
    ]);
    const masks = toMasks(b);
    const O3 = shapeById('O3-0');
    expect(isSolvable(masks, [O3, shapeById('I5-0')])).toBe(true);
    expect(isSolvable(masks, [O3, shapeById('dot-0')])).toBe(false);
  });

  it('always deals a solvable tray when one exists', () => {
    const rng = new Rng(7);
    const crowded = boardFrom([
      '###.####',
      '###.####',
      '###.####',
      '#######.',
      '#######.',
      '##.#####',
      '##.#####',
      '........',
    ]);
    for (let i = 0; i < 50; i++) {
      const tray = generateTray(crowded, rng, 3000);
      expect(isSolvable(toMasks(crowded), tray, 1e7)).toBe(true);
    }
  });

  it('measures the best single-placement clear', () => {
    const b = boardFrom(['###.####', '###.####', '###.####', '', '', '', '', '']);
    expect(bestClear(toMasks(b), shapeById('I3-1'))).toBe(3);
    expect(bestClear(toMasks(b), shapeById('dot-0'))).toBe(1);
  });

  it('counts lines that only pieces placed together can clear', () => {
    const empty = toMasks(createBoard());
    expect(trayClears(empty, ['I4-0', 'I4-0', 'dot-0'].map(shapeById))).toBe(1);
    expect(trayClears(empty, ['O2-0', 'O2-0', 'O2-0'].map(shapeById))).toBe(0);
    const b = boardFrom(['####....', '####....', '', '', '', '', '', '']);
    expect(trayClears(toMasks(b), ['O2-0', 'O2-0', 'dot-0'].map(shapeById))).toBe(2);
  });

  it('deals more line clears in the opening than later on', () => {
    const b = boardFrom(['###.##..', '##..###.', '#....##.', '', '', '', '##...###', '###..###']);
    const masks = toMasks(b);
    const clears = (score: number) => {
      const rng = new Rng(3);
      let sum = 0;
      for (let i = 0; i < 40; i++) sum += trayClears(masks, generateTray(b, rng, score));
      return sum;
    };
    expect(clears(0)).toBeGreaterThan(clears(OPENING_UNTIL * 3));
  });

  it('makes large pieces more likely as the score rises', () => {
    const big = shapeById('O3-0');
    const small = shapeById('O2-0');
    expect(shapeWeight(big, 10000, 0) / shapeWeight(small, 10000, 0)).toBeGreaterThan(
      shapeWeight(big, 0, 0) / shapeWeight(small, 0, 0),
    );
  });
});
