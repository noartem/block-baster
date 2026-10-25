import { type Page, expect, test } from '@playwright/test';
import { TOUCH_GAIN, expectOverlay, open, state, tap, waitUi } from './helpers';

/** Fires a touch pointer event on the canvas. */
function touch(page: Page, type: string, id: number, x: number, y: number) {
  return page.locator('#game').dispatchEvent(type, { pointerId: id, pointerType: 'touch', isPrimary: id === 1, clientX: x, clientY: y, bubbles: true });
}

/** Where a finger has to go for the piece picked up in `slot` to land with its top-left cell on (r, c). */
async function fingerFor(page: Page, slot: number, r: number, c: number) {
  return page.evaluate(
    ({ slot, r, c, gain }) => {
      const { layout, tray } = window.blockBaster.state();
      const p = tray[slot]!;
      const from = { x: layout.slots[slot].cx, y: layout.slots[slot].cy };
      // On pick-up the piece is centred above the finger, its bottom edge 1.1 cells higher.
      const start = { x: from.x - (p.cols * layout.cell) / 2, y: from.y - p.rows * layout.cell - layout.cell * 1.1 };
      const to = { x: layout.boardX + c * layout.cell, y: layout.boardY + r * layout.cell };
      return { from, finger: { x: from.x + (to.x - start.x) / gain.x, y: from.y + (to.y - start.y) / gain.y }, width: layout.width, height: layout.height };
    },
    { slot, r, c, gain: TOUCH_GAIN },
  );
}

test('touch drags reach every corner of the board', async ({ page }) => {
  await open(page);
  for (const [r, c] of [
    [0, 0],
    [0, 7],
    [7, 0],
    [7, 7],
  ]) {
    await page.evaluate(() => window.blockBaster.setTray(['dot-0', 'dot-0', 'dot-0']));
    const g = await fingerFor(page, 1, r, c);
    // The finger stays on the screen.
    expect(g.finger.x).toBeGreaterThanOrEqual(0);
    expect(g.finger.x).toBeLessThanOrEqual(g.width);
    expect(g.finger.y).toBeGreaterThanOrEqual(0);
    expect(g.finger.y).toBeLessThanOrEqual(g.height);
    await touch(page, 'pointerdown', 1, g.from.x, g.from.y);
    for (let i = 1; i <= 4; i++) await touch(page, 'pointermove', 1, g.from.x + ((g.finger.x - g.from.x) * i) / 4, g.from.y + ((g.finger.y - g.from.y) * i) / 4);
    await touch(page, 'pointerup', 1, g.finger.x, g.finger.y);
    expect((await state(page)).board[r * 8 + c], `cell ${r},${c}`).not.toBe(0);
  }
});

test('a drop a little off the grid snaps to the nearest cell', async ({ page }) => {
  await open(page);
  await page.evaluate(() => window.blockBaster.setTray(['O2-0', 'dot-0', 'dot-0']));
  const s = await state(page);
  const { layout } = s;
  await page.mouse.move(layout.slots[0].cx, layout.slots[0].cy);
  await page.mouse.down();
  // With a mouse the piece is held by its centre; aim at (3, 4) plus 0.4 cells right and down.
  const x = layout.boardX + (4 + 1 + 0.4) * layout.cell;
  const y = layout.boardY + (3 + 1 + 0.4) * layout.cell;
  await page.mouse.move(x, y, { steps: 8 });
  await page.mouse.up();
  const board = (await state(page)).board;
  expect([board[3 * 8 + 4], board[3 * 8 + 5], board[4 * 8 + 4], board[4 * 8 + 5]].every((v) => v !== 0)).toBe(true);
});

test('a second finger neither presses buttons nor grabs another piece mid-drag', async ({ page }) => {
  await open(page);
  await page.evaluate(() => window.blockBaster.setTray(['dot-0', 'dot-0', 'dot-0']));
  const menu = await waitUi(page, 'menuButton');
  const g = await fingerFor(page, 0, 4, 4);
  const s = await state(page);
  await touch(page, 'pointerdown', 1, g.from.x, g.from.y);
  expect((await state(page)).dragging).toBe(0);
  // Second finger: taps the menu button, then tries to pick up the middle piece.
  await touch(page, 'pointerdown', 2, menu.cx, menu.cy);
  await touch(page, 'pointerup', 2, menu.cx, menu.cy);
  await touch(page, 'pointerdown', 2, s.layout.slots[1].cx, s.layout.slots[1].cy);
  await touch(page, 'pointermove', 2, s.layout.slots[1].cx, s.layout.slots[1].cy - 100);
  await touch(page, 'pointerup', 2, s.layout.slots[1].cx, s.layout.slots[1].cy - 100);
  expect((await state(page)).overlay).toBe('none');
  expect((await state(page)).dragging).toBe(0);
  // The first finger still drops its piece.
  await touch(page, 'pointermove', 1, g.finger.x, g.finger.y);
  await touch(page, 'pointerup', 1, g.finger.x, g.finger.y);
  const after = await state(page);
  expect(after.board[4 * 8 + 4]).not.toBe(0);
  expect(after.tray.filter(Boolean).length).toBe(2);
});

test('opening the menu mid-drag returns the piece', async ({ page }) => {
  await open(page);
  const s = await state(page);
  await page.mouse.move(s.layout.slots[0].cx, s.layout.slots[0].cy);
  await page.mouse.down();
  await page.mouse.move(s.layout.boardX + s.layout.boardSize / 2, s.layout.boardY + s.layout.boardSize / 2, { steps: 5 });
  expect((await state(page)).dragging).toBe(0);
  await page.keyboard.press('Escape');
  await expectOverlay(page, 'menu');
  expect((await state(page)).dragging).toBeNull();
  await page.mouse.up();
  const after = await state(page);
  expect({ moves: after.moves, tray: after.tray.filter(Boolean).length }).toEqual({ moves: 0, tray: 3 });
});

test('losing focus mid-drag returns the piece', async ({ page }) => {
  await open(page);
  const s = await state(page);
  await page.mouse.move(s.layout.slots[0].cx, s.layout.slots[0].cy);
  await page.mouse.down();
  await page.mouse.move(s.layout.boardX + s.layout.boardSize / 2, s.layout.boardY + s.layout.boardSize / 2, { steps: 5 });
  await page.evaluate(() => window.dispatchEvent(new Event('blur')));
  expect((await state(page)).dragging).toBeNull();
  await page.mouse.up();
  expect((await state(page)).moves).toBe(0);
});

test('right and middle clicks do not pick up pieces', async ({ page }) => {
  await open(page);
  const s = await state(page);
  for (const button of ['right', 'middle'] as const) {
    await page.mouse.move(s.layout.slots[0].cx, s.layout.slots[0].cy);
    await page.mouse.down({ button });
    expect((await state(page)).dragging, button).toBeNull();
    await page.mouse.up({ button });
  }
});

test('the help card scrolls by swiping, even from a button, and by mouse wheel', async ({ page }) => {
  await open(page);
  await tap(page, 'menuButton');
  await expectOverlay(page, 'menu');
  await tap(page, 'help');
  await expect.poll(async () => (await state(page)).helpOpen).toBe(true);
  await page.waitForTimeout(100);
  test.skip(!(await state(page)).canScroll, 'The help card fits on this screen');

  // A swipe starting on the help button scrolls instead of closing the help.
  const help = await waitUi(page, 'help');
  await touch(page, 'pointerdown', 1, help.cx, help.cy);
  for (let i = 1; i <= 5; i++) await touch(page, 'pointermove', 1, help.cx, help.cy - i * 20);
  await touch(page, 'pointerup', 1, help.cx, help.cy - 100);
  let s = await state(page);
  expect(s.helpOpen).toBe(true);
  expect(s.scroll).toBeGreaterThan(50);

  // Firefox-style wheel events count lines.
  await page.evaluate(() =>
    document.getElementById('game')!.dispatchEvent(new WheelEvent('wheel', { deltaY: -2, deltaMode: WheelEvent.DOM_DELTA_LINE, bubbles: true, cancelable: true })),
  );
  const before = s.scroll;
  s = await state(page);
  expect(s.scroll).toBeLessThan(before - 40);
});
