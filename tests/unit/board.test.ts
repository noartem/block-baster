import { describe, expect, it } from 'vitest';
import {
  canPlace,
  canPlaceAnywhere,
  clearLines,
  createBoard,
  findFullLines,
  idx,
  placeMask,
  placeShape,
  previewLines,
  toMasks,
} from '../../src/game/board';
import { shapeById } from '../../src/game/shapes';
import { boardFrom } from './helpers';

const I5h = shapeById('I5-0');
const I5v = shapeById('I5-1');
const O2 = shapeById('O2-0');

describe('board', () => {
  it('rejects out-of-bounds and overlapping placements', () => {
    const b = createBoard();
    expect(canPlace(b, I5h, 0, 3)).toBe(true);
    expect(canPlace(b, I5h, 0, 4)).toBe(false);
    expect(canPlace(b, I5v, 4, 0)).toBe(false);
    expect(canPlace(b, O2, -1, 0)).toBe(false);
    placeShape(b, O2, 0, 0, 3);
    expect(b[idx(1, 1)]).toBe(3);
    expect(canPlace(b, O2, 1, 1)).toBe(false);
    expect(canPlace(b, O2, 2, 2)).toBe(true);
  });

  it('finds and clears rows and columns simultaneously', () => {
    const b = boardFrom([
      '########',
      '#.......',
      '#.......',
      '#.......',
      '#.......',
      '#.......',
      '#.......',
      '#......#',
    ]);
    const lines = findFullLines(b);
    expect(lines).toEqual({ rows: [0], cols: [0] });
    const cleared = clearLines(b, lines);
    // 8 + 8 - 1 shared corner cell
    expect(cleared).toHaveLength(15);
    expect(b[idx(7, 7)]).toBe(1);
    expect(b.filter((v) => v !== 0)).toHaveLength(1);
  });

  it('previews lines without mutating the board', () => {
    const b = boardFrom(['###.....', '', '', '', '', '', '', '']);
    const before = b.slice();
    expect(previewLines(b, I5h, 0, 3)).toEqual({ rows: [0], cols: [] });
    expect(b).toEqual(before);
  });

  it('detects when a shape cannot fit anywhere', () => {
    const b = boardFrom(['#.#.#.#.', '.#.#.#.#', '#.#.#.#.', '.#.#.#.#', '#.#.#.#.', '.#.#.#.#', '#.#.#.#.', '.#.#.#.#']);
    expect(canPlaceAnywhere(b, shapeById('dot-0'))).toBe(true);
    expect(canPlaceAnywhere(b, shapeById('I2-0'))).toBe(false);
    expect(canPlaceAnywhere(b, O2)).toBe(false);
  });

  it('bitmask placement matches the array implementation', () => {
    const b = boardFrom(['###.####', '###.####', '...#....', '...#....', '...#....', '...#....', '...#....', '...#....']);
    const { masks, lines } = placeMask(toMasks(b), shapeById('I2-1'), 0, 3);
    const copy = b.slice();
    placeShape(copy, shapeById('I2-1'), 0, 3, 1);
    const found = findFullLines(copy);
    clearLines(copy, found);
    expect(lines).toBe(found.rows.length + found.cols.length);
    expect(lines).toBe(3);
    expect(masks).toEqual(toMasks(copy));
  });
});
