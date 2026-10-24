/**
 * Captures README media from the real game: staged screenshots, a showcase
 * banner and a short gameplay recording. Run with `npm run screenshots`;
 * CI attaches the output (screenshots/) to every GitHub release.
 */
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, renameSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { type Page, expect, test } from '@playwright/test';

const OUT = 'screenshots';
const VIEWPORT = { width: 390, height: 844 };

// 0 = empty, 1..7 = block colours (red, orange, yellow, green, cyan, blue, purple).
const SHOWCASE_BOARD = [
  [0, 0, 7, 7, 0, 1, 0, 0],
  [3, 0, 7, 7, 0, 1, 2, 2],
  [3, 3, 0, 4, 4, 1, 2, 0],
  [6, 6, 6, 4, 0, 5, 5, 5],
  [6, 0, 2, 2, 2, 5, 0, 0],
  [4, 4, 2, 0, 3, 5, 0, 7],
  [4, 4, 1, 1, 3, 0, 0, 0],
  [5, 5, 1, 1, 3, 0, 0, 0],
];

test.describe.configure({ mode: 'serial' });
test.use({ viewport: VIEWPORT, deviceScaleFactor: 2 });

test.beforeAll(() => mkdirSync(OUT, { recursive: true }));

async function open(page: Page) {
  await page.goto('/?seed=2024&reset');
  await page.waitForFunction(() => document.body.dataset.ready === 'true' && !!window.blockBaster);
}

/** Sets up a game position: board, tray, score, best and combo streak. */
async function stage(
  page: Page,
  opts: { board: number[][]; tray: string[]; score: number; best: number; combo?: number },
) {
  await page.evaluate((o) => {
    const bb = window.blockBaster;
    bb.setBoard(o.board);
    bb.set({ score: o.score, combo: o.combo ?? 0, best: o.best, bestAtStart: o.best });
    bb.setTray(o.tray);
    // Show the staged numbers at once instead of counting up to them.
    bb.settle();
  }, opts);
  await page.waitForTimeout(500);
}

/** Canvas-relative coordinates of a tray slot and of a board drop target (mouse holds pieces by their centre). */
async function geometry(page: Page, slot: number, r: number, c: number) {
  // The canvas fills the page, so canvas coordinates are page coordinates.
  return page.evaluate(
    ({ slot, r, c }) => {
      const { layout, tray } = window.blockBaster.state();
      const shape = tray[slot]!;
      return {
        from: { x: layout.slots[slot].cx, y: layout.slots[slot].cy },
        to: { x: layout.boardX + (c + shape.cols / 2) * layout.cell, y: layout.boardY + (r + shape.rows / 2) * layout.cell },
      };
    },
    { slot, r, c },
  );
}

/** Moves the mouse in small timed steps so recordings show a smooth drag. */
async function glide(page: Page, from: { x: number; y: number }, to: { x: number; y: number }, ms = 380) {
  const steps = Math.round(ms / 16);
  for (let i = 1; i <= steps; i++) {
    const t = i / steps;
    const e = t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
    await page.mouse.move(from.x + (to.x - from.x) * e, from.y + (to.y - from.y) * e);
    await page.waitForTimeout(16);
  }
}

async function shot(page: Page, name: string) {
  await page.screenshot({ path: join(OUT, name) });
}

test('gameplay screenshots', async ({ page }) => {
  await open(page);
  await stage(page, { board: SHOWCASE_BOARD, tray: ['R23-0', 'V5-1', 'T4-0'], score: 2860, best: 4120, combo: 2 });
  await shot(page, 'screenshot-board.png');

  // Hold the 2x3 piece over the bottom-right corner: two rows and a column light up.
  const { from, to } = await geometry(page, 0, 6, 5);
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await glide(page, from, to);
  await page.waitForTimeout(250);
  await shot(page, 'screenshot-drag.png');

  await page.mouse.up();
  await page.waitForTimeout(170);
  await shot(page, 'screenshot-clear.png');
  expect(await page.evaluate(() => window.blockBaster.state().combo)).toBe(3);
});

test('game over screenshot', async ({ page }) => {
  await open(page);
  // Two isolated holes per row and column: a single block fits, nothing bigger does.
  const board = Array.from({ length: 8 }, (_, r) =>
    Array.from({ length: 8 }, (_, c) =>
      c === (r * 3) % 8 || c === (r * 3 + 4) % 8 ? 0 : 1 + ((Math.floor(r / 2) * 3 + Math.floor(c / 2)) % 7),
    ),
  );
  await stage(page, { board, tray: ['dot-0', 'O2-0', 'I3-0'], score: 4869, best: 4120 });
  await page.evaluate(() => window.blockBaster.commit(0, 0, 0));
  await expect.poll(() => page.evaluate(() => window.blockBaster.state().overlay), { timeout: 10_000 }).toBe('gameover');
  expect(await page.evaluate(() => window.blockBaster.state().record)).toBe(true);
  await page.waitForTimeout(600);
  await shot(page, 'screenshot-gameover.png');
});

test('showcase banner', async ({ page }) => {
  const img = (name: string) => `data:image/png;base64,${readFileSync(join(OUT, name)).toString('base64')}`;
  const font = (name: string) => readFileSync(`assets/fonts/${name}.ttf`).toString('base64');
  await page.setViewportSize({ width: 1600, height: 900 });
  await page.setContent(`<!doctype html><html><head><style>
    @font-face { font-family: 'Baloo 2'; font-weight: 800; src: url(data:font/ttf;base64,${font('Baloo2-ExtraBold')}) format('truetype'); }
    @font-face { font-family: 'Baloo 2'; font-weight: 600; src: url(data:font/ttf;base64,${font('Baloo2-SemiBold')}) format('truetype'); }
    * { box-sizing: border-box; margin: 0; }
    body { width: 1600px; height: 900px; overflow: hidden; font-family: 'Baloo 2', sans-serif; color: #fff;
      background: radial-gradient(circle at 20% 15%, #2ec2bd 0%, #168a8e 35%, #0a3f4a 100%); display: flex; align-items: center; }
    .copy { width: 560px; padding-left: 90px; }
    .copy img { width: 120px; height: 120px; filter: drop-shadow(0 10px 20px rgba(0,0,0,.35)); }
    h1 { font-size: 92px; font-weight: 800; line-height: .95; margin: 22px 0 18px; text-shadow: 0 6px 0 rgba(0,0,0,.25); }
    p { font-size: 30px; font-weight: 600; color: #c8f1ec; line-height: 1.3; }
    .pills { display: flex; gap: 12px; margin-top: 30px; flex-wrap: wrap; }
    .pill { background: #ffb547; color: #4a2a00; border-radius: 999px; padding: 6px 20px; font-size: 24px; font-weight: 800; box-shadow: 0 4px 0 #b88233; }
    .phones { position: relative; flex: 1; height: 900px; }
    .phone { position: absolute; top: 50%; width: 330px; padding: 12px; border-radius: 48px; background: #061b20;
      box-shadow: 0 30px 60px rgba(0,0,0,.45), inset 0 0 0 3px #1d4a52; }
    .phone img { display: block; width: 100%; border-radius: 38px; }
    .p1 { left: 20px;  transform: translateY(-50%) rotate(-7deg) scale(.9); }
    .p2 { left: 330px; transform: translateY(-52%); z-index: 2; }
    .p3 { left: 640px; transform: translateY(-50%) rotate(7deg) scale(.9); }
  </style></head><body>
    <div class="copy">
      <img src="data:image/png;base64,${readFileSync('public/pwa-512.png').toString('base64')}" />
      <h1>Block<br />Baster</h1>
      <p>The relaxing block puzzle. Fill rows and columns, chain combos, beat your best.</p>
      <div class="pills"><span class="pill">Free</span><span class="pill">Open source</span><span class="pill">No ads</span></div>
    </div>
    <div class="phones">
      <div class="phone p1"><img src="${img('screenshot-drag.png')}" /></div>
      <div class="phone p2"><img src="${img('screenshot-clear.png')}" /></div>
      <div class="phone p3"><img src="${img('screenshot-gameover.png')}" /></div>
    </div>
  </body></html>`);
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(200);
  await page.screenshot({ path: join(OUT, 'showcase.png') });
});

test('gameplay recording', async ({ browser }) => {
  test.setTimeout(120_000);
  const videoDir = join(OUT, 'video-tmp');
  rmSync(videoDir, { recursive: true, force: true });
  const context = await browser.newContext({ viewport: VIEWPORT, recordVideo: { dir: videoDir, size: VIEWPORT } });
  const page = await context.newPage();
  const recordingStarted = Date.now();
  await open(page);
  await stage(page, { board: SHOWCASE_BOARD, tray: ['R23-0', 'V5-1', 'T4-0'], score: 2860, best: 4120, combo: 2 });
  const trimStart = (Date.now() - recordingStarted) / 1000;

  // A simple bot: prefer moves that clear the most lines, then the tidiest fit.
  for (let move = 0; move < 7; move++) {
    type Move = { slot: number; r: number; c: number; value: number };
    const best = await page.evaluate((): Move | null => {
      const bb = window.blockBaster;
      const { board, tray } = bb.state();
      let pick = null as Move | null;
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
            let touch = 0;
            for (const [dr, dc] of piece.cells)
              for (const [y, x] of [[r + dr - 1, c + dc], [r + dr + 1, c + dc], [r + dr, c + dc - 1], [r + dr, c + dc + 1]])
                touch += y < 0 || y > 7 || x < 0 || x > 7 || board[y * 8 + x] !== 0 ? 1 : 0;
            const value = lines * 100 + touch;
            if (!pick || value > pick.value) pick = { slot, r, c, value };
          }
      });
      return pick;
    });
    if (!best) break;
    const { from, to } = await geometry(page, best.slot, best.r, best.c);
    await page.mouse.move(from.x, from.y);
    await page.waitForTimeout(150);
    await page.mouse.down();
    await glide(page, from, to);
    await page.waitForTimeout(120);
    await page.mouse.up();
    await page.waitForTimeout(best.value >= 100 ? 900 : 450);
  }
  await page.waitForTimeout(400);
  const video = page.video()!;
  await context.close();
  const webm = join(OUT, 'gameplay.webm');
  renameSync(await video.path(), webm);
  rmSync(videoDir, { recursive: true, force: true });

  execFileSync('ffmpeg', [
    '-y', '-loglevel', 'error',
    '-ss', trimStart.toFixed(2),
    '-i', webm,
    // The bottom fifth of the phone screen is empty space below the tray; crop it away.
    '-vf', 'fps=15,crop=iw:ih*0.8:0:0,scale=360:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=160:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle',
    '-loop', '0',
    join(OUT, 'gameplay.gif'),
  ]);
});
