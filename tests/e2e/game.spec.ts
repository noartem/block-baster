import { type Page, expect, test } from '@playwright/test';

async function open(page: Page, query = '?seed=12345&reset') {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(m.text());
  });
  await page.goto(`/${query}`);
  await page.waitForSelector('body[data-ready=true]');
  return errors;
}

/** Finds a legal move for the first piece in the tray, in canvas coordinates. */
async function findMove(page: Page) {
  return page.evaluate(() => {
    const app = window.blockBaster;
    const { layout, game } = app;
    for (let slot = 0; slot < 3; slot++) {
      const piece = game.tray[slot];
      if (!piece) continue;
      for (let r = 0; r < 8; r++)
        for (let c = 0; c < 8; c++)
          if (game.canPlace(slot, r, c)) {
            const w = piece.shape.cols * layout.cell;
            const h = piece.shape.rows * layout.cell;
            return {
              slot,
              r,
              c,
              size: piece.shape.cells.length,
              from: { x: layout.slots[slot].cx, y: layout.slots[slot].cy },
              topLeft: { x: layout.boardX + c * layout.cell, y: layout.boardY + r * layout.cell },
              w,
              h,
              cell: layout.cell,
            };
          }
    }
    return null;
  });
}

test('loads without errors and deals three pieces', async ({ page }) => {
  const errors = await open(page);
  await expect(page.locator('#score')).toHaveText('0');
  const tray = await page.evaluate(() => window.blockBaster.game.tray.filter(Boolean).length);
  expect(tray).toBe(3);
  await expect(page.locator('#game')).toBeVisible();
  expect(errors).toEqual([]);
});

test('drags a piece onto the board with the mouse', async ({ page }) => {
  await open(page);
  const move = (await findMove(page))!;
  const box = (await page.locator('#game').boundingBox())!;
  await page.mouse.move(box.x + move.from.x, box.y + move.from.y);
  await page.mouse.down();
  // With a mouse the piece is held by its centre.
  await page.mouse.move(box.x + move.topLeft.x + move.w / 2, box.y + move.topLeft.y + move.h / 2, { steps: 10 });
  await page.mouse.up();
  await expect(page.locator('#score')).toHaveText(String(move.size));
  const state = await page.evaluate((slot) => ({ empty: window.blockBaster.game.tray[slot] === null, moves: window.blockBaster.game.moves }), move.slot);
  expect(state).toEqual({ empty: true, moves: 1 });
});

test('touch drags lift the piece above the finger', async ({ page }) => {
  await open(page);
  const move = (await findMove(page))!;
  const box = (await page.locator('#game').boundingBox())!;
  const fire = (type: string, x: number, y: number) =>
    page.locator('#game').dispatchEvent(type, {
      pointerId: 7,
      pointerType: 'touch',
      isPrimary: true,
      clientX: box.x + x,
      clientY: box.y + y,
      bubbles: true,
    });
  await fire('pointerdown', move.from.x, move.from.y);
  // The finger sits below the piece: centred horizontally, 1.1 cells under its bottom edge.
  const fx = move.topLeft.x + move.w / 2;
  const fy = move.topLeft.y + move.h + move.cell * 1.1;
  await fire('pointermove', fx, fy);
  await fire('pointerup', fx, fy);
  await expect(page.locator('#score')).toHaveText(String(move.size));
});

test('a piece dropped outside the board returns to the tray', async ({ page }) => {
  await open(page);
  const move = (await findMove(page))!;
  const box = (await page.locator('#game').boundingBox())!;
  await page.mouse.move(box.x + move.from.x, box.y + move.from.y);
  await page.mouse.down();
  await page.mouse.move(box.x + 5, box.y + 5, { steps: 5 });
  await page.mouse.up();
  const state = await page.evaluate(() => ({ tray: window.blockBaster.game.tray.filter(Boolean).length, score: window.blockBaster.game.score }));
  expect(state).toEqual({ tray: 3, score: 0 });
});

test('progress is saved and restored after a reload', async ({ page }) => {
  await open(page);
  const move = (await findMove(page))!;
  await page.evaluate(({ slot, r, c }) => window.blockBaster.commit(slot, r, c), move);
  const before = await page.evaluate(() => JSON.stringify(window.blockBaster.game.serialize()));
  await page.goto('/');
  await page.waitForSelector('body[data-ready=true]');
  const after = await page.evaluate(() => JSON.stringify(window.blockBaster.game.serialize()));
  expect(after).toBe(before);
  await expect(page.locator('#score')).toHaveText(String(move.size));
  await expect(page.locator('#best')).toHaveText(String(move.size));
});

test('shows the game over screen and starts a new game', async ({ page }) => {
  await open(page);
  await page.evaluate(() => {
    const app = window.blockBaster;
    // Checkerboard with the top-left cell free: only single cells fit, never a 2x2 square.
    for (let i = 0; i < 64; i++) app.game.board[i] = (Math.floor(i / 8) + i) % 2 === 0 ? 0 : 3;
    app.debugSetTray(['dot-0', 'O2-0', 'O2-0']);
    app.commit(0, 0, 0);
  });
  await expect(page.locator('#gameover')).toBeVisible({ timeout: 5000 });
  await expect(page.locator('#final-score')).toHaveText('1');
  await expect(page.locator('#record')).toBeVisible();
  await page.locator('#again-btn').click();
  await expect(page.locator('#gameover')).toBeHidden();
  await expect(page.locator('#score')).toHaveText('0');
  const fresh = await page.evaluate(() => ({
    over: window.blockBaster.game.over,
    filled: window.blockBaster.game.board.filter((v) => v).length,
  }));
  expect(fresh).toEqual({ over: false, filled: 0 });
});

test('menu toggles settings and persists them', async ({ page }) => {
  await open(page);
  await page.locator('#menu-btn').click();
  await expect(page.locator('#menu')).toBeVisible();
  // "Exit" only exists inside the Android app.
  await expect(page.locator('#exit-btn')).toBeHidden();
  await page.locator('label:has(#sound-toggle)').click();
  await expect(page.locator('#sound-toggle')).not.toBeChecked();
  await page.locator('#resume-btn').click();
  await expect(page.locator('#menu')).toBeHidden();
  await page.goto('/');
  await page.waitForSelector('body[data-ready=true]');
  await page.locator('#menu-btn').click();
  await expect(page.locator('#sound-toggle')).not.toBeChecked();
  await expect(page.locator('#haptics-toggle')).toBeChecked();
});

test('ships a web app manifest and service worker for offline play', async ({ page, request }) => {
  await open(page);
  const manifestHref = await page.locator('link[rel=manifest]').getAttribute('href');
  expect(manifestHref).toBeTruthy();
  const manifest = await (await request.get(`/${manifestHref!.replace(/^\.\//, '')}`)).json();
  expect(manifest.name).toBe('Block Baster');
  expect(manifest.icons.length).toBeGreaterThanOrEqual(3);
  expect((await request.get('/sw.js')).ok()).toBe(true);
});
