import { type CDPSession, type Page, expect, test } from '@playwright/test';
import { TOUCH_GAIN, waitReady } from './helpers';

/**
 * Plays many moves with real touch drags and checks that memory reaches a
 * steady state: the WebAssembly memory, the JS heap, DOM nodes and event
 * listeners must not keep growing, and the game's own caches and effect lists
 * must stay bounded.
 */

type Move = { slot: number; r: number; c: number };

/** Picks the move that clears the most lines (so effects get exercised), or null when nothing fits. */
function pickMove(page: Page) {
  return page.evaluate((): Move | null => {
    const bb = window.blockBaster;
    const { board, tray } = bb.state();
    let best: (Move & { lines: number }) | null = null;
    tray.forEach((piece, slot) => {
      if (!piece) return;
      for (let r = 0; r < 8; r++)
        for (let c = 0; c < 8; c++) {
          if (!bb.canPlace(slot, r, c)) continue;
          const b = board.slice();
          for (const [dr, dc] of piece.cells) b[(r + dr) * 8 + c + dc] = 1;
          let lines = 0;
          for (let i = 0; i < 8; i++) {
            let row = true;
            let col = true;
            for (let j = 0; j < 8; j++) {
              row &&= b[i * 8 + j] !== 0;
              col &&= b[j * 8 + i] !== 0;
            }
            lines += Number(row) + Number(col);
          }
          if (!best || lines > best.lines) best = { slot, r, c, lines };
        }
    });
    return best;
  });
}

async function touchDrag(page: Page, move: Move | null, slot: number) {
  const g = await page.evaluate(
    ({ slot, move }) => {
      const { layout, tray } = window.blockBaster.state();
      const shape = tray[slot]!;
      const w = shape.cols * layout.cell;
      const h = shape.rows * layout.cell;
      const from = { x: layout.slots[slot].cx, y: layout.slots[slot].cy };
      // Without a move, aim at the top-left corner of the screen: the piece flies back.
      const to = move ? { x: layout.boardX + move.c * layout.cell, y: layout.boardY + move.r * layout.cell } : { x: 0, y: 0 };
      return { from, start: { x: from.x - w / 2, y: from.y - h - layout.cell * 1.1 }, to };
    },
    { slot, move },
  );
  const finger = { x: g.from.x + (g.to.x - g.start.x) / TOUCH_GAIN.x, y: g.from.y + (g.to.y - g.start.y) / TOUCH_GAIN.y };
  const fire = (type: string, x: number, y: number) =>
    page.locator('#game').dispatchEvent(type, { pointerId: 3, pointerType: 'touch', isPrimary: true, clientX: x, clientY: y, bubbles: true });
  await fire('pointerdown', g.from.x, g.from.y);
  // Let the pick-up animation render: the piece grows through many sizes.
  await page.waitForTimeout(140);
  for (let i = 1; i <= 6; i++) {
    await fire('pointermove', g.from.x + ((finger.x - g.from.x) * i) / 6, g.from.y + ((finger.y - g.from.y) * i) / 6);
    await page.waitForTimeout(16);
  }
  await fire('pointerup', finger.x, finger.y);
}

/** Plays `moves` moves, sometimes dropping a piece off the board first, and restarts on game over. */
async function play(page: Page, moves: number) {
  for (let i = 0; i < moves; i++) {
    const move = await pickMove(page);
    if (!move) {
      await page.evaluate(() => window.blockBaster.newGame());
      continue;
    }
    if (i % 4 === 0) {
      await touchDrag(page, null, move.slot);
      await page.waitForTimeout(220);
    }
    const before = await page.evaluate(() => window.blockBaster.state().moves);
    await touchDrag(page, move, move.slot);
    expect(await page.evaluate(() => window.blockBaster.state().moves)).toBe(before + 1);
    await page.waitForTimeout(120);
  }
  await expect.poll(() => page.evaluate(() => window.blockBaster.debugStats().animating), { timeout: 5000 }).toBe(false);
}

async function snapshot(page: Page, cdp: CDPSession) {
  // Collect twice: the first pass can leave objects that only become garbage once finalizers run.
  await cdp.send('HeapProfiler.collectGarbage');
  await cdp.send('HeapProfiler.collectGarbage');
  const { metrics } = await cdp.send('Performance.getMetrics');
  const m = Object.fromEntries(metrics.map((x) => [x.name, x.value]));
  const stats = await page.evaluate(() => window.blockBaster.debugStats());
  return { heap: m.JSHeapUsedSize, nodes: m.Nodes, listeners: m.JSEventListeners, ...stats };
}

test('memory stays flat over a long game', async ({ page, browserName }) => {
  // Heap metrics come from the Chrome DevTools Protocol; touch drags are the phone's input path.
  test.skip(browserName !== 'chromium' || test.info().project.name !== 'mobile', 'Runs on the mobile Chromium project only');
  test.setTimeout(180_000);
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Performance.enable');
  await page.goto('/?seed=777&reset');
  await waitReady(page);

  // Warm up: sprite cache for every colour, first effects and saves.
  await play(page, 20);
  const warm = await snapshot(page, cdp);
  await play(page, 60);
  const after = await snapshot(page, cdp);
  test.info().annotations.push({ type: 'memory', description: JSON.stringify({ warm, after }) });

  // One sprite per colour (plus gray for pieces that don't fit) at one size.
  expect(after.sprites).toBeLessThanOrEqual(8);
  expect(after.particles).toBe(0);
  expect(after.rings).toBe(0);
  expect(after.beams).toBe(0);
  expect(after.streaks).toBe(0);
  expect(after.texts).toBe(0);
  expect(after.listeners).toBe(warm.listeners);
  expect(after.nodes - warm.nodes).toBeLessThanOrEqual(20);
  expect(after.heap - warm.heap).toBeLessThan(1024 * 1024);
  // WebAssembly memory only ever grows, so growth here would be a leak.
  expect(after.wasmMemory - warm.wasmMemory).toBeLessThanOrEqual(2 * 1024 * 1024);
});
