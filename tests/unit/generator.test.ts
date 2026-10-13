import { describe, expect, it } from 'vitest';
import { createBoard, toMasks } from '../../src/game/board';
import { TRAY_SIZE, bestClear, generateTray, isSolvable, shapeWeight } from '../../src/game/generator';
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

  it('makes large pieces more likely as the score rises', () => {
    const big = shapeById('O3-0');
    const small = shapeById('O2-0');
    expect(shapeWeight(big, 10000, 0) / shapeWeight(small, 10000, 0)).toBeGreaterThan(
      shapeWeight(big, 0, 0) / shapeWeight(small, 0, 0),
    );
  });
});
