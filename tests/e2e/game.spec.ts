import { expect, test } from '@playwright/test';
import { TOUCH_GAIN, expectOverlay, findMove, open, slide, state, tap, waitReady } from './helpers';

test('loads without errors and deals three pieces', async ({ page }) => {
  const errors = await open(page);
  const s = await state(page);
  expect(s.score).toBe(0);
  expect(s.tray.filter(Boolean).length).toBe(3);
  await expect(page.locator('#game')).toBeVisible();
  expect(errors).toEqual([]);
});

test('drags a piece onto the board with the mouse', async ({ page }) => {
  await open(page);
  const move = (await findMove(page))!;
  await page.mouse.move(move.from.x, move.from.y);
  await page.mouse.down();
  // With a mouse the piece is held by its centre.
  await page.mouse.move(move.topLeft.x + move.w / 2, move.topLeft.y + move.h / 2, { steps: 10 });
  await page.mouse.up();
  await expect.poll(async () => (await state(page)).score).toBe(move.size);
  const s = await state(page);
  expect({ empty: s.tray[move.slot] === null, moves: s.moves }).toEqual({ empty: true, moves: 1 });
});

test('touch drags lift the piece above the finger and move it faster', async ({ page }) => {
  await open(page);
  const move = (await findMove(page))!;
  const fire = (type: string, x: number, y: number) =>
    page.locator('#game').dispatchEvent(type, { pointerId: 7, pointerType: 'touch', isPrimary: true, clientX: x, clientY: y, bubbles: true });
  await fire('pointerdown', move.from.x, move.from.y);
  // The piece starts centred above the finger, its bottom edge 1.1 cells higher,
  // then moves TOUCH_GAIN times as far as the finger does.
  const startX = move.from.x - move.w / 2;
  const startY = move.from.y - move.h - move.cell * 1.1;
  const fx = move.from.x + (move.topLeft.x - startX) / TOUCH_GAIN.x;
  const fy = move.from.y + (move.topLeft.y - startY) / TOUCH_GAIN.y;
  await fire('pointermove', fx, fy);
  // Lifting the finger may slide it a little; the drop keeps the last move's spot.
  await fire('pointerup', fx + move.cell, fy + move.cell);
  await expect.poll(async () => (await state(page)).score).toBe(move.size);
});

test('a piece dropped outside the board returns to the tray', async ({ page }) => {
  await open(page);
  const move = (await findMove(page))!;
  await page.mouse.move(move.from.x, move.from.y);
  await page.mouse.down();
  await page.mouse.move(5, 5, { steps: 5 });
  await page.mouse.up();
  const s = await state(page);
  expect({ tray: s.tray.filter(Boolean).length, score: s.score }).toEqual({ tray: 3, score: 0 });
});

test('progress is saved and restored after a reload', async ({ page }) => {
  await open(page);
  const move = (await findMove(page))!;
  await page.evaluate(({ slot, r, c }) => window.blockBaster.commit(slot, r, c), move);
  const before = await page.evaluate(() => JSON.stringify(window.blockBaster.serialize()));
  await page.goto('/');
  await waitReady(page);
  const after = await page.evaluate(() => JSON.stringify(window.blockBaster.serialize()));
  expect(after).toBe(before);
  const s = await state(page);
  expect(s.score).toBe(move.size);
  expect(s.best).toBe(move.size);
});

test('shows the game over screen and starts a new game', async ({ page }) => {
  await open(page);
  await page.evaluate(() => {
    const bb = window.blockBaster;
    // Checkerboard with the top-left cell free: only single cells fit, never a 2x2 square.
    bb.setBoard(Array.from({ length: 64 }, (_, i) => ((Math.floor(i / 8) + i) % 2 === 0 ? 0 : 3)));
    bb.setTray(['dot-0', 'O2-0', 'O2-0']);
    bb.commit(0, 0, 0);
  });
  await expectOverlay(page, 'gameover');
  let s = await state(page);
  expect(s.score).toBe(1);
  expect(s.record).toBe(true);
  // Let the card pop in, then press "Play again".
  await page.waitForTimeout(350);
  await tap(page, 'again');
  await expectOverlay(page, 'none');
  s = await state(page);
  expect({ score: s.score, over: s.over, filled: s.board.filter((v) => v).length }).toEqual({ score: 0, over: false, filled: 0 });
});

test('clearing the board switches the style and high scores unlock new tiers', async ({ page }) => {
  const errors = await open(page);
  await expect(page.locator('body')).toHaveAttribute('data-style', '0.0');
  expect((await state(page)).style.name).toBe('Classic · Lagoon');
  await page.evaluate(() => {
    const bb = window.blockBaster;
    const board = Array(64).fill(0);
    for (let c = 0; c < 7; c++) board[c] = 2;
    bb.setBoard(board);
    bb.setTray(['dot-0', 'dot-0', 'dot-0']);
    bb.commit(0, 0, 7);
  });
  await expect(page.locator('body')).toHaveAttribute('data-style', '0.1');
  expect((await state(page)).style.name).toBe('Classic · Ember');

  await page.evaluate(() => {
    const bb = window.blockBaster;
    bb.set({ score: 129_990 });
    const board = Array(64).fill(0);
    for (let c = 0; c < 7; c++) board[8 + c] = 4;
    bb.setBoard(board);
    bb.commit(1, 1, 7);
  });
  await expect(page.locator('body')).toHaveAttribute('data-tier', '7');
  expect((await state(page)).style.name).toContain('Neon');
  await page.waitForTimeout(1500);
  expect(errors).toEqual([]);
});

test('music starts on the first touch, speeds up with the score and can be turned off', async ({ page }) => {
  const errors = await open(page);
  await page.mouse.click(5, 300);
  await expect.poll(async () => (await state(page)).music.playing).toBe(true);
  expect((await state(page)).music.bpm).toBeLessThan(80);
  await page.evaluate(() => window.blockBaster.set({ score: 100_000 }));
  await expect.poll(async () => (await state(page)).music.bpm, { timeout: 15_000 }).toBeGreaterThan(100);
  await tap(page, 'menuButton');
  await expectOverlay(page, 'menu');
  await page.waitForTimeout(350);
  await slide(page, 'music', 0);
  let s = await state(page);
  expect(s.settings.music).toBe(0);
  expect(s.music.playing).toBe(false);
  await slide(page, 'music', 0.6);
  await expect.poll(async () => (await state(page)).music.playing).toBe(true);
  s = await state(page);
  expect(s.music.volume).toBeCloseTo(0.6);
  expect(errors).toEqual([]);
});

test('menu sliders set the volumes and persist them', async ({ page }) => {
  await open(page);
  await tap(page, 'menuButton');
  await expectOverlay(page, 'menu');
  await page.waitForTimeout(350);
  let s = await state(page);
  // "Exit" only exists inside the apps.
  expect(s.ui.exit).toBeNull();
  expect(s.settings.sound).toBe(1);
  await slide(page, 'sound', 0.4);
  s = await state(page);
  expect(s.settings.sound).toBeCloseTo(0.4);
  expect(s.soundVolume).toBeCloseTo(0.4);
  await tap(page, 'resume');
  await expectOverlay(page, 'none');
  await page.goto('/');
  await waitReady(page);
  s = await state(page);
  expect(s.settings.sound).toBeCloseTo(0.4);
  expect(s.settings.music).toBe(1);
});

test('the menu closes on Escape and on a tap outside the card', async ({ page }) => {
  await open(page);
  await page.keyboard.press('Escape');
  await expectOverlay(page, 'menu');
  await page.keyboard.press('Escape');
  await expectOverlay(page, 'none');
  await tap(page, 'menuButton');
  await expectOverlay(page, 'menu');
  await page.waitForTimeout(350);
  await page.mouse.click(4, 4);
  await expectOverlay(page, 'none');
});

test('vibration settings only show up on devices that can vibrate', async ({ page, isMobile }) => {
  await open(page);
  await tap(page, 'menuButton');
  await page.waitForTimeout(350);
  const s = await state(page);
  if (isMobile) {
    expect(s.ui.haptics).not.toBeNull();
    expect(s.settings.haptics).toBe(0.5);
  } else {
    expect(s.ui.haptics).toBeNull();
  }
});

test('turns the on/off switches of older versions into levels', async ({ page }) => {
  await open(page);
  await page.evaluate(() => localStorage.setItem('bb.settings', JSON.stringify({ sound: false, music: true, haptics: true })));
  await page.goto('/');
  await waitReady(page);
  expect((await state(page)).settings).toEqual({ sound: 0, music: 1, haptics: 0.5 });
});

test('continues a game saved by the old web version', async ({ page }) => {
  await open(page);
  await page.evaluate(() => {
    const board = Array(64).fill(0);
    board[63] = 3;
    localStorage.setItem('bb.best', '5000');
    localStorage.setItem(
      'bb.game',
      JSON.stringify({ v: 1, board, tray: [{ id: 'L4-2', color: 5 }, null, { id: 'dot-0', color: 1 }], score: 120, combo: 1, sinceClear: 0, moves: 12, rng: 123456, over: false }),
    );
  });
  await page.goto('/');
  await waitReady(page);
  const s = await state(page);
  expect({ score: s.score, best: s.best, moves: s.moves, cell: s.board[63], piece: s.tray[0]?.id }).toEqual({ score: 120, best: 5000, moves: 12, cell: 3, piece: 'L4-2' });
});

test('ships a web app manifest and service worker for offline play', async ({ page, request }) => {
  await open(page);
  const manifestHref = await page.locator('link[rel=manifest]').getAttribute('href');
  expect(manifestHref).toBeTruthy();
  const manifest = await (await request.get(`/${manifestHref!.replace(/^\.\//, '')}`)).json();
  expect(manifest.name).toBe('Block Baster');
  expect(manifest.icons.length).toBeGreaterThanOrEqual(3);
  const sw = await request.get('/sw.js');
  expect(sw.ok()).toBe(true);
  expect(await sw.text()).not.toContain('__BUILD__');
});
