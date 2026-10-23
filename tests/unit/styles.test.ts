import { describe, expect, it } from 'vitest';
import { Game } from '../../src/game/game';
import { shapeById } from '../../src/game/shapes';
import { TIERS, formatScore, nextTier, styleFor, tierFor } from '../../src/render/styles';
import { boardFrom } from './helpers';

describe('styles', () => {
  it('unlocks tiers from Classic to Legendary at rising scores', () => {
    expect(TIERS.map((t) => t.name)).toEqual(['Classic', 'Cardboard', 'Candy', 'Gingerbread', 'Wooden', 'Gems', 'Iron', 'Neon', 'Aurora', 'Legendary']);
    expect(TIERS.map((t) => t.minScore)).toEqual([0, 5_000, 12_000, 25_000, 40_000, 60_000, 90_000, 130_000, 200_000, 350_000]);
    expect(tierFor(0).name).toBe('Classic');
    expect(tierFor(4_999).level).toBe(0);
    expect(tierFor(5_000).name).toBe('Cardboard');
    expect(tierFor(39_999).name).toBe('Gingerbread');
    expect(tierFor(99_999).name).toBe('Iron');
    expect(tierFor(5_000_000).name).toBe('Legendary');
    expect(nextTier(tierFor(5_000_000))).toBeNull();
  });

  it('numbers tiers by their position, with each next tier needing a higher score', () => {
    TIERS.forEach((tier, i) => {
      expect(tier.level).toBe(i);
      if (i > 0) expect(tier.minScore).toBeGreaterThan(TIERS[i - 1].minScore);
      expect(nextTier(tier)).toBe(TIERS[i + 1] ?? null);
    });
  });

  it('gives every tier its own blocks and frame', () => {
    expect(new Set(TIERS.map((t) => t.block)).size).toBe(TIERS.length);
    expect(new Set(TIERS.map((t) => t.frame)).size).toBe(TIERS.length);
  });

  it('cycles through the variants of a tier on board clears', () => {
    const n = TIERS[0].variants.length;
    expect(styleFor(0, 0).key).toBe('0.0');
    expect(styleFor(0, 1).key).toBe('0.1');
    expect(styleFor(0, n).key).toBe('0.0');
    expect(styleFor(60_000, 2).key).toBe('5.2');
    expect(styleFor(60_000, 2).tier.name).toBe('Gems');
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
