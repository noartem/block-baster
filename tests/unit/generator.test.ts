import { describe, expect, it } from 'vitest';
import { createBoard, toMasks } from '../../src/game/board';
import {
  LUCK_LEVELS,
  RESCUE_FILL,
  TRAY_SIZE,
  generateTray,
  isLuckyDeal,
  isSolvable,
  luckEvery,
  shapeWeight,
  trayClears,
} from '../../src/game/generator';
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

  it('counts lines that only pieces placed together can clear', () => {
    const empty = toMasks(createBoard());
    expect(trayClears(empty, ['I4-0', 'I4-0', 'dot-0'].map(shapeById))).toBe(1);
    expect(trayClears(empty, ['O2-0', 'O2-0', 'O2-0'].map(shapeById))).toBe(0);
    const b = boardFrom(['####....', '####....', '', '', '', '', '', '']);
    expect(trayClears(toMasks(b), ['O2-0', 'O2-0', 'dot-0'].map(shapeById))).toBe(2);
  });

  it('deals lucky trays that clear more lines than plain ones', () => {
    const b = boardFrom(['###.##..', '##..###.', '#....##.', '', '', '', '##...###', '###..###']);
    const masks = toMasks(b);
    const clears = (lucky: boolean) => {
      const rng = new Rng(3);
      let sum = 0;
      for (let i = 0; i < 40; i++) sum += trayClears(masks, generateTray(b, rng, 20_000, lucky));
      return sum;
    };
    expect(clears(true)).toBeGreaterThan(clears(false) * 1.25);
  });

  it('makes lucky trays rarer as the score rises, but never stops them', () => {
    expect(luckEvery(0)).toBe(1);
    const steps = LUCK_LEVELS.map((l) => l.every);
    expect(steps).toEqual([...steps].sort((a, b) => a - b));
    expect(luckEvery(1e9)).toBe(steps[steps.length - 1]);
    // Every 3rd tray at 20k: two plain trays, then a lucky one.
    expect(luckEvery(20_000)).toBe(3);
    expect([0, 1, 2].map((since) => isLuckyDeal(20_000, 0, since))).toEqual([false, false, true]);
  });

  it('always deals a lucky tray onto a crowded board', () => {
    expect(isLuckyDeal(1e6, RESCUE_FILL, 0)).toBe(true);
    expect(isLuckyDeal(1e6, RESCUE_FILL - 0.1, 0)).toBe(false);
  });

  it('makes large pieces more likely as the score rises', () => {
    const big = shapeById('O3-0');
    const small = shapeById('O2-0');
    expect(shapeWeight(big, 10000, 0) / shapeWeight(small, 10000, 0)).toBeGreaterThan(
      shapeWeight(big, 0, 0) / shapeWeight(small, 0, 0),
    );
  });
});
