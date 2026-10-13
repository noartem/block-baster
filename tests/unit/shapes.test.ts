import { describe, expect, it } from 'vitest';
import { SHAPES, shapeById } from '../../src/game/shapes';

describe('shapes', () => {
  it('has unique ids', () => {
    expect(new Set(SHAPES.map((s) => s.id)).size).toBe(SHAPES.length);
  });

  it('generates the expected number of distinct rotations', () => {
    const count = (family: string) => SHAPES.filter((s) => s.family === family).length;
    expect(count('dot')).toBe(1);
    expect(count('I3')).toBe(2);
    expect(count('O2')).toBe(1);
    expect(count('V3')).toBe(4);
    expect(count('L4')).toBe(4);
    expect(count('T4')).toBe(4);
    expect(count('S4')).toBe(2);
    expect(count('V5')).toBe(4);
  });

  it('normalizes cells to start at the origin and fit their bounding box', () => {
    for (const s of SHAPES) {
      expect(Math.min(...s.cells.map((c) => c[0]))).toBe(0);
      expect(Math.min(...s.cells.map((c) => c[1]))).toBe(0);
      for (const [r, c] of s.cells) {
        expect(r).toBeLessThan(s.rows);
        expect(c).toBeLessThan(s.cols);
        expect(s.rowMasks[r] & (1 << c)).toBeTruthy();
      }
      expect(s.weight).toBeGreaterThan(0);
    }
  });

  it('looks shapes up by id', () => {
    expect(shapeById('O3-0').cells).toHaveLength(9);
    expect(() => shapeById('nope')).toThrow();
  });
});
