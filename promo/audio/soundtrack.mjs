// Renders the game's own adaptive soundtrack (src/audio/music.ts) for the promo video.
//
// The built game is served by Vite and opened in Playwright; its Music instance renders into an
// OfflineAudioContext, following an intensity curve given as [time, score, speed] points (score
// progress and action speed, both 0..1). Math.random is seeded, so the result is reproducible.
//
// Usage (from the repository root, after `npm run build`):
//   node promo/audio/soundtrack.mjs <out.wav> <seconds> '<[[time, score, speed], ...]>'
// promo/audio/music.py runs this itself.

import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const PORT = 4180;
const SAMPLE_RATE = 44100;

const [out, secondsArg, pointsArg] = process.argv.slice(2);
const seconds = Number(secondsArg);
const points = JSON.parse(pointsArg ?? '[]');
if (!out || !(seconds > 0) || !points.length) {
  console.error('Usage: node promo/audio/soundtrack.mjs <out.wav> <seconds> <points json>');
  process.exit(1);
}

/**
 * Seeded Math.random (mulberry32); `__seed()` restarts the sequence. The game's render loop is
 * switched off, so nothing else draws random numbers while the music renders.
 */
const SEEDED_RANDOM = () => {
  window.requestAnimationFrame = () => 0;
  let a = 7;
  window.__seed = () => (a = 7);
  Math.random = () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
};

function serve() {
  const server = spawn('npx', ['vite', 'preview', '--port', String(PORT), '--strictPort'], { cwd: root, stdio: 'pipe', detached: true });
  return new Promise((ok, fail) => {
    server.stdout.on('data', (d) => String(d).includes(`${PORT}`) && ok(server));
    server.on('exit', (code) => fail(new Error(`vite preview exited with ${code}`)));
  });
}

const server = await serve();
const browser = await chromium.launch();
try {
  const page = await browser.newPage();
  await page.addInitScript(SEEDED_RANDOM);
  await page.goto(`http://localhost:${PORT}/?seed=7&reset`);
  await page.waitForSelector('body[data-ready=true]');
  const wav = await page.evaluate(
    async ({ seconds, points, sampleRate }) => {
      // Piecewise-linear input between the points.
      const input = (time) => {
        let i = 0;
        while (i < points.length - 1 && points[i + 1][0] <= time) i++;
        const [t0, s0, v0] = points[i];
        const [t1, s1, v1] = points[Math.min(i + 1, points.length - 1)];
        const k = t1 > t0 ? Math.min(1, Math.max(0, (time - t0) / (t1 - t0))) : 0;
        return { score: s0 + (s1 - s0) * k, speed: v0 + (v1 - v0) * k };
      };
      const ctx = new OfflineAudioContext(2, Math.ceil(sampleRate * seconds), sampleRate);
      // The game may have used random numbers while loading; start the music from a known state.
      window.__seed();
      const buffer = await window.blockBaster.music.render(ctx, seconds, input);
      // 16-bit stereo WAV, returned as base64.
      const n = buffer.length;
      const view = new DataView(new ArrayBuffer(44 + n * 4));
      const text = (at, s) => [...s].forEach((c, i) => view.setUint8(at + i, c.charCodeAt(0)));
      text(0, 'RIFF');
      view.setUint32(4, 36 + n * 4, true);
      text(8, 'WAVEfmt ');
      view.setUint32(16, 16, true);
      view.setUint16(20, 1, true);
      view.setUint16(22, 2, true);
      view.setUint32(24, sampleRate, true);
      view.setUint32(28, sampleRate * 4, true);
      view.setUint16(32, 4, true);
      view.setUint16(34, 16, true);
      text(36, 'data');
      view.setUint32(40, n * 4, true);
      const channels = [buffer.getChannelData(0), buffer.getChannelData(1)];
      for (let i = 0; i < n; i++)
        for (let c = 0; c < 2; c++) view.setInt16(44 + i * 4 + c * 2, Math.max(-1, Math.min(1, channels[c][i])) * 32767, true);
      const bytes = new Uint8Array(view.buffer);
      let bin = '';
      for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
      return btoa(bin);
    },
    { seconds, points, sampleRate: SAMPLE_RATE },
  );
  writeFileSync(out, Buffer.from(wav, 'base64'));
  console.log(`${out}: ${seconds.toFixed(1)}s`);
} finally {
  await browser.close();
  process.kill(-server.pid);
}
