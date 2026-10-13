import { describe, expect, it } from 'vitest';
import { SNAP_RADIUS, nearestFit } from '../../src/input';

describe('nearestFit', () => {
  const only = (spots: [number, number][]) => (r: number, c: number) => spots.some(([sr, sc]) => sr === r && sc === c);

  it('picks the rounded cell when it fits', () => {
    expect(nearestFit(2.3, 4.6, () => true)).toEqual({ r: 2, c: 5 });
  });

  it('pulls a near miss onto the closest spot that fits', () => {
    expect(nearestFit(2.3, 4.6, only([[3, 4], [2, 4]]))).toEqual({ r: 2, c: 4 });
    expect(nearestFit(0.4, 0.4, only([[1, 1]]))).toEqual({ r: 1, c: 1 });
  });

  it('ignores spots outside the snap radius', () => {
    expect(nearestFit(2, 2, only([[2, 2 + Math.ceil(SNAP_RADIUS + 0.01)]]))).toBeNull();
    expect(nearestFit(2, 2, () => false)).toBeNull();
  });
});
