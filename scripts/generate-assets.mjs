// Generates every raster asset (PWA icons, favicon, Android launcher icons and
// splash screens) from vector artwork defined in this file.
// Usage: npm run assets
import { mkdirSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import sharp from 'sharp';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const out = (p) => {
  const file = join(root, p);
  mkdirSync(dirname(file), { recursive: true });
  return file;
};

const COLORS = ['#ef4b5a', '#f7962f', '#f6cd30', '#4cc85d', '#2fbde8', '#4f6ef2', '#a75ae6'];

function mix(hex, amount) {
  const n = parseInt(hex.slice(1), 16);
  const t = amount > 0 ? 255 : 0;
  const a = Math.abs(amount);
  const ch = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => Math.round(v + (t - v) * a));
  return `rgb(${ch.join(',')})`;
}

/** One bevelled block, same look as the in-game sprite. */
function block(x, y, s, color) {
  const b = s * 0.15;
  const r = s * 0.1;
  const id = `c${Math.round(x)}_${Math.round(y)}`;
  return `
  <g>
    <clipPath id="${id}"><rect x="${x}" y="${y}" width="${s}" height="${s}" rx="${r}"/></clipPath>
    <g clip-path="url(#${id})">
      <rect x="${x}" y="${y}" width="${s}" height="${s}" fill="${color}"/>
      <polygon points="${x},${y} ${x + s},${y} ${x + s - b},${y + b} ${x + b},${y + b}" fill="${mix(color, 0.42)}"/>
      <polygon points="${x},${y} ${x + b},${y + b} ${x + b},${y + s - b} ${x},${y + s}" fill="${mix(color, 0.2)}"/>
      <polygon points="${x + s},${y} ${x + s},${y + s} ${x + s - b},${y + s - b} ${x + s - b},${y + b}" fill="${mix(color, -0.18)}"/>
      <polygon points="${x},${y + s} ${x + b},${y + s - b} ${x + s - b},${y + s - b} ${x + s},${y + s}" fill="${mix(color, -0.32)}"/>
      <rect x="${x + b}" y="${y + b}" width="${s - 2 * b}" height="${s - 2 * b}" fill="${mix(color, 0.08)}"/>
      <ellipse cx="${x + b + s * 0.16}" cy="${y + b + s * 0.13}" rx="${s * 0.15}" ry="${s * 0.09}" fill="#fff" opacity="0.4"/>
    </g>
    <rect x="${x + 1}" y="${y + 1}" width="${s - 2}" height="${s - 2}" rx="${r}" fill="none" stroke="${mix(color, -0.45)}" stroke-width="${s * 0.025}"/>
  </g>`;
}

/**
 * The logo: a small 3x3 board where an L-piece of blocks is landing.
 * `scale` (0..1) controls how much of the canvas the artwork covers.
 */
function logo(size, scale, { background = true, round = 0.22 } = {}) {
  const art = size * scale;
  const o = (size - art) / 2;
  const cell = art / 3;
  const layout = [
    [0, 0, 0],
    [0, 1, 2],
    [1, 1, 2],
    [2, 0, 3],
    [2, 1, 4],
    [2, 2, 5],
    [0, 2, 6],
  ];
  const blocks = layout
    .map(([r, c, k]) => block(o + c * cell + cell * 0.03, o + r * cell + cell * 0.03, cell * 0.94, COLORS[k]))
    .join('');
  const bg = background
    ? `<defs><linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#4f6be0"/><stop offset="1" stop-color="#232f80"/></linearGradient></defs>
       <rect width="${size}" height="${size}" rx="${size * round}" fill="url(#bg)"/>
       <rect x="${o - cell * 0.12}" y="${o - cell * 0.12}" width="${art + cell * 0.24}" height="${art + cell * 0.24}" rx="${cell * 0.25}" fill="#1b2556"/>`
    : '';
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">${bg}${blocks}</svg>`;
}

function backgroundOnly(size) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}"><defs><linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#4f6be0"/><stop offset="1" stop-color="#232f80"/></linearGradient></defs><rect width="${size}" height="${size}" fill="url(#bg)"/></svg>`;
}

function splash(w, h) {
  const s = Math.min(w, h) * 0.32;
  const inner = logo(s, 0.86, { round: 0.22 }).replace(/<svg[^>]*>|<\/svg>/g, '');
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}">
    <defs><linearGradient id="sbg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#4560d6"/><stop offset="0.5" stop-color="#2f43ab"/><stop offset="1" stop-color="#232f80"/></linearGradient></defs>
    <rect width="${w}" height="${h}" fill="url(#sbg)"/>
    <g transform="translate(${(w - s) / 2},${(h - s) / 2 - s * 0.15})">${inner}</g>
    <text x="${w / 2}" y="${(h + s) / 2 + s * 0.12}" font-family="Arial Rounded MT Bold, Arial, sans-serif" font-weight="900" font-size="${s * 0.2}" fill="#fff" text-anchor="middle">BLOCK BASTER</text>
  </svg>`;
}

const png = (svg, file, size) =>
  sharp(Buffer.from(svg))
    .resize(size, size)
    .png({ compressionLevel: 9 })
    .toFile(out(file));

// --- Web / PWA -------------------------------------------------------------
writeFileSync(out('public/favicon.svg'), logo(64, 0.8));
writeFileSync(out('assets/icon.svg'), logo(1024, 0.72));
await png(logo(512, 0.72), 'public/pwa-512.png', 512);
await png(logo(192, 0.72), 'public/pwa-192.png', 192);
await png(logo(180, 0.72, { round: 0 }), 'public/apple-touch-icon.png', 180);
await png(logo(512, 0.56, { round: 0 }), 'public/pwa-maskable-512.png', 512);
await png(logo(1024, 0.72), 'assets/icon-1024.png', 1024);

// --- Android ---------------------------------------------------------------
const res = 'android/app/src/main/res';
if (existsSync(join(root, res))) {
  const densities = { mdpi: 1, hdpi: 1.5, xhdpi: 2, xxhdpi: 3, xxxhdpi: 4 };
  for (const [d, k] of Object.entries(densities)) {
    await png(logo(512, 0.72, { round: 0.22 }), `${res}/mipmap-${d}/ic_launcher.png`, Math.round(48 * k));
    await png(logo(512, 0.72, { round: 0.5 }), `${res}/mipmap-${d}/ic_launcher_round.png`, Math.round(48 * k));
    // Adaptive icon foreground: 108dp canvas, artwork inside the 66dp safe zone.
    await png(logo(432, 0.5, { background: false }), `${res}/mipmap-${d}/ic_launcher_foreground.png`, Math.round(108 * k));
    await png(backgroundOnly(432), `${res}/mipmap-${d}/ic_launcher_background.png`, Math.round(108 * k));
  }
  const splashes = {
    drawable: [480, 320],
    'drawable-port-mdpi': [320, 480],
    'drawable-port-hdpi': [480, 800],
    'drawable-port-xhdpi': [720, 1280],
    'drawable-port-xxhdpi': [960, 1600],
    'drawable-port-xxxhdpi': [1280, 1920],
    'drawable-land-mdpi': [480, 320],
    'drawable-land-hdpi': [800, 480],
    'drawable-land-xhdpi': [1280, 720],
    'drawable-land-xxhdpi': [1600, 960],
    'drawable-land-xxxhdpi': [1920, 1280],
  };
  for (const [dir, [w, h]] of Object.entries(splashes)) {
    await sharp(Buffer.from(splash(w, h))).png({ compressionLevel: 9 }).toFile(out(`${res}/${dir}/splash.png`));
  }
  console.log('Android icons and splash screens written.');
}
console.log('Web assets written.');
