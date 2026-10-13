import { describe, expect, it } from 'vitest';
import { createBoard, idx } from '../../src/game/board';
import { BOARD_CLEAR_BONUS, COMBO_WINDOW, Game, linePoints } from '../../src/game/game';
import { shapeById } from '../../src/game/shapes';
import { boardFrom } from './helpers';

function withTray(game: Game, ids: (string | null)[]) {
  game.tray = ids.map((id, i) => (id ? { shape: shapeById(id), color: i + 1 } : null));
  game.over = false;
}

describe('Game', () => {
  it('starts with a full tray and an empty board', () => {
    const g = new Game(1);
    expect(g.tray.every((p) => p !== null)).toBe(true);
    expect(g.board.every((v) => v === 0)).toBe(true);
    expect(g.score).toBe(0);
  });

  it('scores one point per placed block', () => {
    const g = new Game(1);
    withTray(g, ['O2-0', 'dot-0', 'dot-0']);
    const res = g.place(0, 3, 3)!;
    expect(res.points).toBe(4);
    expect(g.score).toBe(4);
    expect(g.tray[0]).toBeNull();
    expect(g.board[idx(4, 4)]).toBe(1);
  });

  it('rejects invalid placements', () => {
    const g = new Game(1);
    withTray(g, ['O2-0', null, 'dot-0']);
    expect(g.place(0, 7, 7)).toBeNull();
    expect(g.place(1, 0, 0)).toBeNull();
    expect(g.score).toBe(0);
  });

  it('clears lines, scores them and tracks combo streaks', () => {
    const g = new Game(1);
    g.board = boardFrom(['####....', '####....', '', '', '', '', '', '#']);
    withTray(g, ['I4-0', 'I4-0', 'dot-0']);
    const first = g.place(0, 0, 4)!;
    expect(first.lines.rows).toEqual([0]);
    expect(first.combo).toBe(1);
    expect(first.points).toBe(4 + linePoints(1, 1));
    const second = g.place(1, 1, 4)!;
    expect(second.combo).toBe(2);
    expect(second.linePoints).toBe(linePoints(1, 2));
    expect(second.boardCleared).toBe(false);
  });

  it('awards a bonus when the whole board is cleared', () => {
    const g = new Game(1);
    g.board = boardFrom(['#######.']);
    withTray(g, ['dot-0', 'dot-0', 'dot-0']);
    const res = g.place(0, 0, 7)!;
    expect(res.boardCleared).toBe(true);
    expect(res.points).toBe(1 + linePoints(1, 1) + BOARD_CLEAR_BONUS);
  });

  it('ends the combo after too many placements without a clear', () => {
    const g = new Game(1);
    g.combo = 4;
    for (let i = 0; i < COMBO_WINDOW; i++) {
      withTray(g, ['dot-0', 'dot-0', 'dot-0']);
      g.place(0, 4, i);
      expect(g.combo).toBe(i < COMBO_WINDOW - 1 ? 4 : 0);
    }
  });

  it('deals a new tray once all pieces are used', () => {
    const g = new Game(1);
    withTray(g, ['dot-0', 'dot-0', 'dot-0']);
    expect(g.place(0, 0, 0)!.refilled).toBe(false);
    expect(g.place(1, 0, 1)!.refilled).toBe(false);
    expect(g.place(2, 0, 2)!.refilled).toBe(true);
    expect(g.tray.every((p) => p !== null)).toBe(true);
  });

  it('detects game over when no remaining piece fits', () => {
    const g = new Game(1);
    g.board = boardFrom(['.#.#.#.#', '#.#.#.#.', '.#.#.#.#', '#.#.#.#.', '.#.#.#.#', '#.#.#.#.', '.#.#.#.#', '#.#.#.#.']);
    withTray(g, ['dot-0', 'O2-0', 'I3-0']);
    const res = g.place(0, 0, 0)!;
    expect(res.gameOver).toBe(true);
    expect(g.over).toBe(true);
    expect(g.place(1, 0, 0)).toBeNull();
  });

  it('round-trips through serialization', () => {
    const g = new Game(99);
    g.board = createBoard();
    g.place(0, 0, 0);
    const copy = Game.deserialize(JSON.parse(JSON.stringify(g.serialize())));
    expect(copy.serialize()).toEqual(g.serialize());
    // The RNG continues identically, so future deals match.
    expect(copy.rng.next()).toBe(g.rng.next());
    expect(() => Game.deserialize({ v: 2 } as never)).toThrow();
  });

  it('plays full random games to completion without errors', () => {
    for (let seed = 1; seed <= 5; seed++) {
      const g = new Game(seed);
      let guard = 0;
      while (!g.over && guard++ < 2000) {
        let moved = false;
        for (let s = 0; s < 3 && !moved; s++)
          for (let r = 0; r < 8 && !moved; r++)
            for (let c = 0; c < 8 && !moved; c++) moved = !!g.place(s, r, c);
        expect(moved).toBe(true);
      }
      expect(g.over).toBe(true);
      expect(g.score).toBeGreaterThan(0);
    }
  });
});
