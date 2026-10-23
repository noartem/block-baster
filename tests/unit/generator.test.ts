import { describe, expect, it } from 'vitest';
import { createBoard, toMasks } from '../../src/game/board';
import {
  LUCK_LEVELS,
  RESCUE_FILL,
  TRAY_SIZE,
  difficulty,
  generateTray,
  holes,
  isLuckyDeal,
  isSolvable,
  luckEvery,
  shapeWeight,
  trayClears,
  trayValue,
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

  it('makes lucky trays rarer as the score rises, but never rarer than every 3rd', () => {
    expect(luckEvery(0)).toBe(1);
    const steps = LUCK_LEVELS.map((l) => l.every);
    expect(steps).toEqual([...steps].sort((a, b) => a - b));
    expect(luckEvery(1e9)).toBe(steps[steps.length - 1]);
    expect(Math.max(...steps)).toBeLessThanOrEqual(3);
    // Every 2nd tray for a long stretch, then every 3rd.
    expect(luckEvery(20_000)).toBe(2);
    expect([0, 1].map((since) => isLuckyDeal(20_000, 0, since))).toEqual([false, true]);
    expect(luckEvery(100_000)).toBe(3);
    expect([0, 1, 2].map((since) => isLuckyDeal(100_000, 0, since))).toEqual([false, false, true]);
  });

  it('always deals a lucky tray onto a crowded board', () => {
    expect(RESCUE_FILL).toBeLessThanOrEqual(0.45);
    expect(isLuckyDeal(1e6, RESCUE_FILL, 0)).toBe(true);
    expect(isLuckyDeal(1e6, RESCUE_FILL - 0.1, 0)).toBe(false);
  });

  it('makes a crowded lucky tray clear a line whenever pieces can', () => {
    // Only dots can complete a line here (row 4, columns 1, 5 and 7), while
    // upright I2–I4s fit the gaps without clearing anything.
    const b = boardFrom([
      '.#.#.#.#',
      '.#.#.#.#',
      '.#.#.#.#',
      '.#.#.#.#',
      '###.####',
      '.#.#.#.#',
      '.#.#.#.#',
      '#.#.#.#.',
    ]);
    expect(35 / 64).toBeGreaterThanOrEqual(RESCUE_FILL);
    const masks = toMasks(b);
    const rng = new Rng(11);
    for (let i = 0; i < 30; i++) {
      const tray = generateTray(b, rng, 1e6, true);
      expect(isSolvable(masks, tray, 1e7)).toBe(true);
      expect(trayClears(masks, tray)).toBeGreaterThan(0);
    }
  });

  it('counts boxed-in holes', () => {
    expect(holes(toMasks(createBoard()))).toBe(0);
    expect(holes(toMasks(boardFrom(['.#', '#', '', '', '', '', '', ''])))).toBe(1);
    expect(holes(toMasks(boardFrom(['', '', '.#.#', '#.#.', '', '', '', ''])))).toBe(0);
    expect(holes(toMasks(boardFrom(['', '.#', '#.#', '.#', '', '', '', ''])))).toBe(1);
  });

  it('values trays that leave fewer holes higher', () => {
    // A dot fills the boxed-in corner; three I2s leave it open.
    const masks = toMasks(boardFrom(['.##.....', '#.......', '', '', '', '', '', '']));
    const withDot = trayValue(masks, ['dot-0', 'I2-0', 'I2-0'].map(shapeById));
    const withoutDot = trayValue(masks, ['I2-0', 'I2-0', 'I2-0'].map(shapeById));
    // More than the one extra cell the I2s add.
    expect(withDot - withoutDot).toBeGreaterThan(1);
  });

  it('ramps the piece mix up gently and caps it', () => {
    const big = shapeById('O3-0');
    const small = shapeById('O2-0');
    const ratio = (score: number) => shapeWeight(big, score, 0) / shapeWeight(small, score, 0);
    expect(ratio(10_000)).toBeGreaterThan(ratio(0));
    expect(ratio(50_000)).toBeGreaterThan(ratio(10_000));
    expect(difficulty(0)).toBe(0);
    expect(difficulty(30_000)).toBeLessThan(0.25);
    expect(difficulty(1e9)).toBeLessThan(1);
    expect(ratio(1e9)).toBe(ratio(1e12));
  });

  it('favours small pieces, more so on a crowded board', () => {
    const big = shapeById('O3-0');
    const small = shapeById('I3-0');
    expect(shapeWeight(small, 0, 0)).toBeGreaterThan(shapeWeight(big, 0, 0) * 2);
    expect(shapeWeight(small, 0, 0.6) / shapeWeight(big, 0, 0.6)).toBeGreaterThan(
      shapeWeight(small, 0, 0) / shapeWeight(big, 0, 0),
    );
  });
});
