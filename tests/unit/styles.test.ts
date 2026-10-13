import { describe, expect, it } from 'vitest';
import { Game } from '../../src/game/game';
import { shapeById } from '../../src/game/shapes';
import { TIERS, formatScore, nextTier, styleFor, tierFor } from '../../src/render/styles';
import { boardFrom } from './helpers';

describe('styles', () => {
  it('unlocks tiers at 50k, 100k, 200k, 500k and 1M', () => {
    expect(TIERS.map((t) => t.minScore)).toEqual([0, 50_000, 100_000, 200_000, 500_000, 1_000_000]);
    expect(tierFor(0).name).toBe('Classic');
    expect(tierFor(49_999).level).toBe(0);
    expect(tierFor(50_000).level).toBe(1);
    expect(tierFor(199_999).level).toBe(2);
    expect(tierFor(5_000_000).name).toBe('Legendary');
    expect(nextTier(tierFor(5_000_000))).toBeNull();
  });

  it('cycles through the variants of a tier on board clears', () => {
    const n = TIERS[0].variants.length;
    expect(styleFor(0, 0).key).toBe('0.0');
    expect(styleFor(0, 1).key).toBe('0.1');
    expect(styleFor(0, n).key).toBe('0.0');
    expect(styleFor(120_000, 2).key).toBe('2.2');
  });

  it('gives every variant seven valid block colours and hex palette colours', () => {
    for (const tier of TIERS) {
      expect(tier.variants.length).toBeGreaterThanOrEqual(2);
      for (const p of tier.variants) {
        expect(p.blocks).toHaveLength(7);
        const colors = [...p.blocks, ...p.bg, p.board, p.boardEdge, p.cell, p.cellInner, p.accent, p.glow, p.glow2, p.card, p.cardEdge];
        for (const c of colors) expect(c).toMatch(/^#[0-9a-f]{6}$/);
      }
    }
  });

  it('formats compact scores', () => {
    expect(formatScore(50_000)).toBe('50k');
    expect(formatScore(1_000_000)).toBe('1M');
    expect(formatScore(950)).toBe('950');
  });
});

describe('board clears', () => {
  it('are counted and survive serialization, including older saves without them', () => {
    const g = new Game(1);
    g.board = boardFrom(['#######.']);
    g.tray = [{ shape: shapeById('dot-0'), color: 1 }, { shape: shapeById('dot-0'), color: 2 }, null];
    expect(g.place(0, 0, 7)!.boardCleared).toBe(true);
    expect(g.boardClears).toBe(1);
    expect(Game.deserialize(JSON.parse(JSON.stringify(g.serialize()))).boardClears).toBe(1);
    const old = { ...g.serialize() };
    delete old.clears;
    expect(Game.deserialize(old).boardClears).toBe(0);
  });
});
