// Captures real screenshots and a gameplay clip of Block Baster for the promo video.
//
// The game is built and served by Vite, then driven by Playwright. An init script replaces
// `performance.now` and `requestAnimationFrame` with a virtual clock, so every animation frame is
// rendered exactly when we ask for it: the clip is perfectly smooth no matter how slow screenshots
// are, and the run is reproducible.
//
// Output (all in promo/media):
//   screen-*.png         phone screenshots (390x844 CSS px at 2x)
//   desktop.png          the game in a desktop browser window
//   gameplay.mp4         30 fps gameplay clip, encoded with the system ffmpeg
//   gameplay-touch.json  finger position per clip frame (for the touch indicator overlay)
//
// Usage (from the repository root): npm run build && node promo/capture/capture.mjs

import { chromium } from '@playwright/test';
import { spawn, execFileSync } from 'node:child_process';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const media = resolve(here, '../media');
const framesDir = resolve(here, '../.frames');
const PORT = 4179;
const BASE = `http://localhost:${PORT}/`;
const FPS = 30;
const STEP = 1000 / FPS;
const PHONE = { viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, hasTouch: true, isMobile: true };

/** Virtual clock: time only moves when `__step(ms)` is called. */
const VIRTUAL_TIME = () => {
  let now = 1000;
  let queue = [];
  performance.now = () => now;
  window.requestAnimationFrame = (cb) => {
    queue.push(cb);
    return queue.length;
  };
  window.cancelAnimationFrame = () => {};
  window.__step = (ms) => {
    now += ms;
    const callbacks = queue;
    queue = [];
    for (const cb of callbacks) cb(now);
  };
  // No sound while capturing.
  window.AudioContext = undefined;
  window.webkitAudioContext = undefined;
};

function serve() {
  // Detached, so the whole process group (npx and vite) can be stopped at the end.
  const server = spawn('npx', ['vite', 'preview', '--port', String(PORT), '--strictPort'], { cwd: root, stdio: 'pipe', detached: true });
  return new Promise((ok, fail) => {
    server.stdout.on('data', (d) => String(d).includes(`${PORT}`) && ok(server));
    server.on('exit', (code) => fail(new Error(`vite preview exited with ${code}`)));
  });
}

async function openGame(context, query = '?seed=7&reset') {
  const page = await context.newPage();
  await page.addInitScript(VIRTUAL_TIME);
  await page.goto(BASE + query);
  await page.waitForSelector('body[data-ready=true]');
  await page.evaluate(() => document.fonts.ready);
  await step(page, 40);
  return page;
}

const step = (page, frames = 1) => page.evaluate(({ n, ms }) => { for (let i = 0; i < n; i++) window.__step(ms); }, { n: frames, ms: STEP });

/**
 * Stages the board for the clip: three moves that clear one row, then two rows, then a row and a
 * column at once, which empties the whole board.
 */
async function stageClip(page, { score, best }) {
  await page.evaluate(({ score, best }) => {
    const app = window.blockBaster;
    const g = app.game;
    // Colour map, one character per cell: '.' empty, digits are colour ids.
    const rows = [
      '...4....',
      '...4....',
      '...6....',
      '...6....',
      '557.3311',
      '557222..',
      '117622..',
      '11766...',
    ];
    rows.forEach((line, r) => [...line].forEach((ch, c) => (g.board[r * 8 + c] = ch === '.' ? 0 : Number(ch))));
    app.debugSetTray(['I3-0', 'O2-0', 'I4-1']);
    const colors = [5, 2, 7];
    g.tray.forEach((p, i) => (p.color = colors[i]));
    g.score = score;
    g.moves = 37;
    app.best = best;
    app.invalidate();
  }, { score, best });
  // Let the HUD settle on the staged score instead of counting up to it.
  await step(page, 90);
}

/** Canvas-relative geometry of a move: where the piece sits in the tray and where the finger must go. */
async function moveGeometry(page, slot, r, c) {
  return page.evaluate(({ slot, r, c }) => {
    const { layout, game } = window.blockBaster;
    const piece = game.tray[slot];
    const w = piece.shape.cols * layout.cell;
    const h = piece.shape.rows * layout.cell;
    const left = layout.boardX + c * layout.cell;
    const top = layout.boardY + r * layout.cell;
    // Touch drags hold the piece 1.1 cells above the finger (see app.ts).
    return { from: { x: layout.slots[slot].cx, y: layout.slots[slot].cy }, to: { x: left + w / 2, y: top + h + layout.cell * 1.1 } };
  }, { slot, r, c });
}

const ease = (t) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);

async function recordClip(page) {
  rmSync(framesDir, { recursive: true, force: true });
  mkdirSync(framesDir, { recursive: true });
  const canvas = page.locator('#game');
  const box = await canvas.boundingBox();
  const touch = [];
  let frame = 0;
  let finger = null;

  const fire = (type, p) =>
    canvas.dispatchEvent(type, { pointerId: 11, pointerType: 'touch', isPrimary: true, clientX: box.x + p.x, clientY: box.y + p.y, bubbles: true });

  const snap = async (name) => {
    await step(page);
    const path = join(framesDir, `f${String(frame).padStart(4, '0')}.png`);
    await page.screenshot({ path });
    if (name) await page.screenshot({ path: join(media, `screen-${name}.png`) });
    // Stored in screenshot pixels (2x) relative to the page; null when the finger is up.
    touch.push(finger ? { x: (box.x + finger.x) * 2, y: (box.y + finger.y) * 2, down: finger.down } : null);
    frame++;
  };
  const hold = async (frames, stills = {}) => {
    for (let i = 0; i < frames; i++) await snap(stills[i]);
  };

  async function drag(slot, r, c) {
    const { from, to } = await moveGeometry(page, slot, r, c);
    // Finger arrives at the piece.
    finger = { ...from, down: 0 };
    for (let i = 1; i <= 6; i++) {
      finger = { ...from, down: i / 6 };
      await snap();
    }
    await fire('pointerdown', from);
    const travel = 20;
    for (let i = 1; i <= travel; i++) {
      const t = ease(i / travel);
      finger = { x: from.x + (to.x - from.x) * t, y: from.y + (to.y - from.y) * t, down: 1 };
      await fire('pointermove', finger);
      await snap();
    }
    // A short pause over the target shows the line preview.
    await hold(7);
    await fire('pointerup', finger);
    for (let i = 1; i <= 6; i++) {
      finger = { ...finger, down: 1 - i / 6 };
      await snap();
    }
    finger = null;
  }

  await hold(15, { 0: 'board' });
  await drag(0, 7, 5);
  await hold(24);
  await drag(1, 5, 6);
  await hold(34);
  await drag(2, 4, 3);
  await hold(54, { 14: 'board-clear' });

  writeFileSync(join(media, 'gameplay-touch.json'), JSON.stringify({ fps: FPS, width: 780, height: 1688, frames: touch }));
  execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-framerate', String(FPS), '-i', join(framesDir, 'f%04d.png'),
    '-c:v', 'libx264', '-preset', 'slow', '-crf', '16', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', join(media, 'gameplay.mp4')]);
  rmSync(framesDir, { recursive: true, force: true });
  console.log(`gameplay.mp4: ${frame} frames`);
}

async function captureMenu(context) {
  const page = await openGame(context);
  await stageClip(page, { score: 1960, best: 2150 });
  await page.locator('#menu-btn').click();
  await step(page, 10);
  await page.screenshot({ path: join(media, 'screen-menu.png') });
  await page.close();
}

async function captureGameOver(context) {
  const page = await openGame(context);
  await page.evaluate(() => {
    const app = window.blockBaster;
    const rows = [
      '22.11.44',
      '2.71.334',
      '6677.3.5',
      '.6.7555.',
      '33.1.2.2',
      '3.11.222',
      '45.66.7.',
      '445.6677',
    ];
    rows.forEach((line, r) => [...line].forEach((ch, c) => (app.game.board[r * 8 + c] = ch === '.' ? 0 : Number(ch))));
    app.game.score = 2486;
    app.best = 2150;
    // Dropping the dot leaves only an O3 and an I5 that fit nowhere.
    app.debugSetTray(['dot-0', 'O3-0', 'I5-1']);
    if (!app.commit(0, 0, 2)?.gameOver) throw new Error('staged game over did not end the game');
  });
  // The game over timers use real time; keep the virtual clock running meanwhile.
  for (let i = 0; i < 90 && !(await page.locator('#gameover').isVisible()); i++) {
    await step(page, 3);
    await page.waitForTimeout(50);
  }
  await step(page, 20);
  await page.screenshot({ path: join(media, 'screen-gameover.png') });
  await page.close();
}

async function captureDesktop(browser) {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1.5 });
  const page = await openGame(context);
  await stageClip(page, { score: 1960, best: 2150 });
  await page.screenshot({ path: join(media, 'desktop.png') });
  await context.close();
}

mkdirSync(media, { recursive: true });
const server = await serve();
const browser = await chromium.launch();
try {
  const phone = await browser.newContext(PHONE);
  const page = await openGame(phone);
  await stageClip(page, { score: 1960, best: 2150 });
  await recordClip(page);
  await page.close();
  await captureMenu(phone);
  await captureGameOver(phone);
  await phone.close();
  await captureDesktop(browser);
} finally {
  await browser.close();
  process.kill(-server.pid);
}
console.log('Captured into', media);
