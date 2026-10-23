import { type Page, expect } from '@playwright/test';

/** Touch drags move the piece faster than the finger (TOUCH_GAIN in src/input.rs). */
export const TOUCH_GAIN = { x: 1.25, y: 1.5 };

/** Opens the game and collects page errors. */
export async function open(page: Page, query = '?seed=12345&reset') {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(m.text());
  });
  await page.goto(`/${query}`);
  await waitReady(page);
  return errors;
}

export async function waitReady(page: Page) {
  await page.waitForFunction(() => document.body.dataset.ready === 'true' && !!window.blockBaster);
}

export const state = (page: Page) => page.evaluate(() => window.blockBaster.state());

/** Waits for the game to show an overlay ('none', 'menu' or 'gameover'). */
export async function expectOverlay(page: Page, overlay: string, timeout = 5000) {
  await expect.poll(() => page.evaluate(() => document.body.dataset.overlay), { timeout }).toBe(overlay);
}

/** Clicks a UI element drawn on the canvas, by name (see BBState.ui). */
export async function tap(page: Page, name: keyof BBState['ui']) {
  const rect = (await state(page)).ui[name];
  expect(rect, `${name} is on screen`).toBeTruthy();
  await page.mouse.click(rect!.cx, rect!.cy);
}

/** Sets a menu slider to `value` (0..1) by clicking its track. */
export async function slide(page: Page, name: 'music' | 'sound' | 'haptics', value: number) {
  const rect = (await state(page)).ui[name];
  expect(rect, `${name} slider is on screen`).toBeTruthy();
  // The hit area reaches 14 px past each end of the track.
  await page.mouse.click(rect!.x + 14 + (rect!.w - 28) * value, rect!.cy);
}

/** Finds a legal move for the first piece in the tray that fits somewhere. */
export async function findMove(page: Page) {
  return page.evaluate(() => {
    const bb = window.blockBaster;
    const { layout, tray } = bb.state();
    for (let slot = 0; slot < 3; slot++) {
      const piece = tray[slot];
      if (!piece) continue;
      for (let r = 0; r < 8; r++)
        for (let c = 0; c < 8; c++)
          if (bb.canPlace(slot, r, c)) {
            return {
              slot,
              r,
              c,
              size: piece.cells.length,
              from: { x: layout.slots[slot].cx, y: layout.slots[slot].cy },
              topLeft: { x: layout.boardX + c * layout.cell, y: layout.boardY + r * layout.cell },
              w: piece.cols * layout.cell,
              h: piece.rows * layout.cell,
              cell: layout.cell,
            };
          }
    }
    return null;
  });
}
