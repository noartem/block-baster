import { colorFor, rgba, shade, shadeHex } from './palette';
import type { BlockKind, Style } from './styles';

/**
 * Procedurally generated block graphics, one look per style tier. Sprites are
 * rendered once per style/colour/size and cached.
 */
const cache = new Map<string, HTMLCanvasElement>();

export function blockSprite(color: number, size: number, dpr: number, style: Style): HTMLCanvasElement {
  const px = Math.max(4, Math.round(size * dpr));
  const key = `${style.key}:${color}:${px}`;
  let sprite = cache.get(key);
  if (!sprite) {
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = px;
    DRAW[style.tier.block](canvas.getContext('2d')!, colorFor(color, style.palette), px);
    sprite = canvas;
    cache.set(key, sprite);
  }
  return sprite;
}

/** Number of cached sprites (for leak tests). */
export function spriteCacheSize(): number {
  return cache.size;
}

export function clearSpriteCache(): void {
  cache.clear();
}

type Ctx = CanvasRenderingContext2D;

export function roundRect(ctx: Ctx, x: number, y: number, w: number, h: number, r: number) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

function poly(ctx: Ctx, pts: number[], fill: string) {
  ctx.beginPath();
  ctx.moveTo(pts[0], pts[1]);
  for (let i = 2; i < pts.length; i += 2) ctx.lineTo(pts[i], pts[i + 1]);
  ctx.closePath();
  ctx.fillStyle = fill;
  ctx.fill();
}

/** Small seeded PRNG, so textures like wood grain look the same on every redraw. */
export function rng(seed: number): () => number {
  let x = seed >>> 0 || 0x9e3779b9;
  return () => {
    x ^= x << 13;
    x ^= x >>> 17;
    x ^= x << 5;
    return (x >>> 0) / 4294967296;
  };
}

function hash(text: string): number {
  let h = 2166136261;
  for (let i = 0; i < text.length; i++) h = Math.imul(h ^ text.charCodeAt(i), 16777619);
  return h >>> 0;
}

/** Length of the outline of a rounded rectangle. */
export function roundRectLength(w: number, h: number, r: number): number {
  return 2 * (w - 2 * r) + 2 * (h - 2 * r) + 2 * Math.PI * r;
}

/**
 * The point at distance `d` along the outline of a rounded rectangle, clockwise
 * from the start of its top edge, and the outward normal there: [x, y, nx, ny].
 */
export function roundRectPoint(x: number, y: number, w: number, h: number, r: number, d: number): [number, number, number, number] {
  r = Math.max(1e-3, r);
  const sw = w - 2 * r;
  const sh = h - 2 * r;
  const arc = (Math.PI * r) / 2;
  const total = roundRectLength(w, h, r);
  let t = ((d % total) + total) % total;
  const lens = [sw, arc, sh, arc, sw, arc, sh, arc];
  let seg = 0;
  while (seg < 7 && t > lens[seg]) t -= lens[seg++];
  const corner = (cx: number, cy: number, a0: number): [number, number, number, number] => {
    const a = a0 + t / r;
    return [cx + Math.cos(a) * r, cy + Math.sin(a) * r, Math.cos(a), Math.sin(a)];
  };
  switch (seg) {
    case 0: return [x + r + t, y, 0, -1];
    case 1: return corner(x + w - r, y + r, -Math.PI / 2);
    case 2: return [x + w, y + r + t, 1, 0];
    case 3: return corner(x + w - r, y + h - r, 0);
    case 4: return [x + w - r - t, y + h, 0, 1];
    case 5: return corner(x + r, y + h - r, Math.PI / 2);
    case 6: return [x, y + h - r - t, -1, 0];
    default: return corner(x + r, y + r, Math.PI);
  }
}

/** A rounded rectangle outline that wiggles in and out, like piped icing. */
export function wavyRoundRect(ctx: Ctx, x: number, y: number, w: number, h: number, r: number, amp: number, wavelength: number) {
  const total = roundRectLength(w, h, r);
  const waves = Math.max(4, Math.round(total / wavelength));
  const steps = waves * 10;
  ctx.beginPath();
  for (let i = 0; i <= steps; i++) {
    const [px, py, nx, ny] = roundRectPoint(x, y, w, h, r, (i / steps) * total);
    const o = amp * Math.sin((i / steps) * waves * Math.PI * 2);
    if (i === 0) ctx.moveTo(px + nx * o, py + ny * o);
    else ctx.lineTo(px + nx * o, py + ny * o);
  }
  ctx.closePath();
}

/** A four-pointed twinkle. */
function sparkle(ctx: Ctx, x: number, y: number, r: number, alpha: number) {
  ctx.fillStyle = `rgba(255,255,255,${alpha})`;
  ctx.beginPath();
  ctx.moveTo(x, y - r);
  ctx.quadraticCurveTo(x, y, x + r, y);
  ctx.quadraticCurveTo(x, y, x, y + r);
  ctx.quadraticCurveTo(x, y, x - r, y);
  ctx.quadraticCurveTo(x, y, x, y - r);
  ctx.fill();
}

/** Classic: a bevelled square with a light top-left edge, dark bottom-right edge and a soft glossy face. */
function jewel(ctx: Ctx, base: string, s: number) {
  const gap = Math.max(1, s * 0.03);
  const x0 = gap;
  const x1 = s - gap;
  const radius = s * 0.1;
  const bevel = s * 0.15;

  ctx.save();
  roundRect(ctx, x0, x0, x1 - x0, x1 - x0, radius);
  ctx.clip();

  ctx.fillStyle = base;
  ctx.fillRect(0, 0, s, s);

  const i0 = x0 + bevel;
  const i1 = x1 - bevel;
  poly(ctx, [x0, x0, x1, x0, i1, i0, i0, i0], shade(base, 0.42)); // top
  poly(ctx, [x0, x0, i0, i0, i0, i1, x0, x1], shade(base, 0.2)); // left
  poly(ctx, [x1, x0, x1, x1, i1, i1, i1, i0], shade(base, -0.18)); // right
  poly(ctx, [x0, x1, i0, i1, i1, i1, x1, x1], shade(base, -0.32)); // bottom

  const face = ctx.createLinearGradient(0, i0, 0, i1);
  face.addColorStop(0, shade(base, 0.12));
  face.addColorStop(1, shade(base, -0.06));
  ctx.fillStyle = face;
  ctx.fillRect(i0, i0, i1 - i0, i1 - i0);

  // Glossy highlight in the upper-left of the face.
  const gloss = ctx.createRadialGradient(i0 + s * 0.12, i0 + s * 0.1, 0, i0 + s * 0.12, i0 + s * 0.1, s * 0.32);
  gloss.addColorStop(0, 'rgba(255,255,255,0.45)');
  gloss.addColorStop(1, 'rgba(255,255,255,0)');
  ctx.fillStyle = gloss;
  ctx.fillRect(i0, i0, i1 - i0, i1 - i0);
  ctx.restore();

  ctx.lineWidth = Math.max(1, s * 0.025);
  ctx.strokeStyle = shade(base, -0.45);
  roundRect(ctx, x0 + ctx.lineWidth / 2, x0 + ctx.lineWidth / 2, x1 - x0 - ctx.lineWidth, x1 - x0 - ctx.lineWidth, radius);
  ctx.stroke();
}

/** Candy: a rounded, glossy jelly sweet with a big shine on top. */
function candy(ctx: Ctx, base: string, s: number) {
  const gap = Math.max(1, s * 0.04);
  const w = s - gap * 2;
  const radius = s * 0.26;

  ctx.save();
  roundRect(ctx, gap, gap, w, w, radius);
  ctx.clip();
  const body = ctx.createLinearGradient(0, gap, 0, s - gap);
  body.addColorStop(0, shade(base, 0.25));
  body.addColorStop(0.55, base);
  body.addColorStop(1, shade(base, -0.28));
  ctx.fillStyle = body;
  ctx.fillRect(0, 0, s, s);

  // Inner glow from the bottom, like light passing through jelly.
  const inner = ctx.createRadialGradient(s / 2, s * 0.95, 0, s / 2, s * 0.95, s * 0.6);
  inner.addColorStop(0, rgba('#ffffff', 0.35));
  inner.addColorStop(1, rgba('#ffffff', 0));
  ctx.fillStyle = inner;
  ctx.fillRect(0, 0, s, s);

  // Big shine across the top half.
  ctx.fillStyle = 'rgba(255,255,255,0.5)';
  ctx.beginPath();
  ctx.ellipse(s * 0.5, s * 0.27, s * 0.34, s * 0.15, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = 'rgba(255,255,255,0.9)';
  ctx.beginPath();
  ctx.ellipse(s * 0.3, s * 0.26, s * 0.07, s * 0.045, -0.5, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();

  ctx.lineWidth = Math.max(1, s * 0.035);
  ctx.strokeStyle = shade(base, -0.4);
  const h = ctx.lineWidth / 2;
  roundRect(ctx, gap + h, gap + h, w - h * 2, w - h * 2, radius);
  ctx.stroke();
}

/** Gems: a cut stone with eight facets around an octagonal table and a sparkle. */
function gem(ctx: Ctx, base: string, s: number) {
  const a = Math.max(1, s * 0.03);
  const b = s - a;
  const t0 = s * 0.22;
  const t1 = s - t0;
  const k = s * 0.1; // corner cut of the table
  const radius = s * 0.08;

  ctx.save();
  roundRect(ctx, a, a, b - a, b - a, radius);
  ctx.clip();
  const O = [a, a, b, a, b, b, a, b];
  const T = [t0 + k, t0, t1 - k, t0, t1, t0 + k, t1, t1 - k, t1 - k, t1, t0 + k, t1, t0, t1 - k, t0, t0 + k];
  const o = (i: number) => [O[i * 2], O[i * 2 + 1]];
  const t = (i: number) => [T[i * 2], T[i * 2 + 1]];
  poly(ctx, [...o(0), ...o(1), ...t(1), ...t(0)], shade(base, 0.45)); // top
  poly(ctx, [...o(1), ...t(2), ...t(1)], shade(base, 0.2));
  poly(ctx, [...o(1), ...o(2), ...t(3), ...t(2)], shade(base, -0.2)); // right
  poly(ctx, [...o(2), ...t(4), ...t(3)], shade(base, -0.42));
  poly(ctx, [...o(2), ...o(3), ...t(5), ...t(4)], shade(base, -0.35)); // bottom
  poly(ctx, [...o(3), ...t(6), ...t(5)], shade(base, -0.1));
  poly(ctx, [...o(3), ...o(0), ...t(7), ...t(6)], shade(base, 0.15)); // left
  poly(ctx, [...o(0), ...t(0), ...t(7)], shade(base, 0.55));

  const table = ctx.createLinearGradient(t0, t0, t1, t1);
  table.addColorStop(0, shade(base, 0.3));
  table.addColorStop(0.5, base);
  table.addColorStop(1, shade(base, -0.12));
  poly(ctx, T, '#000');
  ctx.fillStyle = table;
  ctx.fill();
  // Light streak across the table.
  poly(ctx, [t0 + k, t0, t0 + k * 2.6, t0, t0, t0 + k * 2.6, t0, t0 + k], 'rgba(255,255,255,0.4)');
  ctx.restore();

  ctx.lineWidth = Math.max(1, s * 0.025);
  ctx.strokeStyle = shade(base, -0.55);
  roundRect(ctx, a + ctx.lineWidth / 2, a + ctx.lineWidth / 2, b - a - ctx.lineWidth, b - a - ctx.lineWidth, radius);
  ctx.stroke();
  sparkle(ctx, s * 0.3, s * 0.3, s * 0.16, 0.95);
}

/** Neon: a dark tile outlined by a glowing tube. */
function neon(ctx: Ctx, base: string, s: number) {
  const pad = s * 0.12;
  const w = s - pad * 2;
  const radius = s * 0.14;
  const face = ctx.createLinearGradient(0, pad, 0, s - pad);
  face.addColorStop(0, rgba(base, 0.38));
  face.addColorStop(1, rgba(base, 0.14));
  roundRect(ctx, pad, pad, w, w, radius);
  ctx.fillStyle = '#05010f';
  ctx.fill();
  ctx.fillStyle = face;
  ctx.fill();

  ctx.shadowColor = base;
  ctx.shadowBlur = s * 0.16;
  ctx.lineWidth = Math.max(1.5, s * 0.075);
  ctx.strokeStyle = base;
  ctx.stroke();
  ctx.stroke();
  ctx.shadowBlur = 0;
  // Hot white core of the tube.
  ctx.lineWidth = Math.max(1, s * 0.025);
  ctx.strokeStyle = shade(base, 0.65);
  ctx.stroke();
  // Small inner square for depth.
  const ip = s * 0.34;
  roundRect(ctx, ip, ip, s - ip * 2, s - ip * 2, s * 0.06);
  ctx.fillStyle = rgba(base, 0.35);
  ctx.fill();
}

/** Aurora: a translucent glass tile with a bright rim and a diagonal reflection. */
function glass(ctx: Ctx, base: string, s: number) {
  const gap = Math.max(1, s * 0.04);
  const w = s - gap * 2;
  const radius = s * 0.18;

  ctx.save();
  roundRect(ctx, gap, gap, w, w, radius);
  ctx.clip();
  const body = ctx.createLinearGradient(gap, gap, s - gap, s - gap);
  body.addColorStop(0, rgba(base, 0.95));
  body.addColorStop(0.5, rgba(base, 0.6));
  body.addColorStop(1, rgba(base, 0.85));
  ctx.fillStyle = body;
  ctx.fillRect(0, 0, s, s);
  // Glow from the centre.
  const core = ctx.createRadialGradient(s / 2, s / 2, 0, s / 2, s / 2, s * 0.5);
  core.addColorStop(0, 'rgba(255,255,255,0.4)');
  core.addColorStop(1, 'rgba(255,255,255,0)');
  ctx.fillStyle = core;
  ctx.fillRect(0, 0, s, s);
  // Diagonal reflection bands.
  poly(ctx, [0, s * 0.55, s * 0.55, 0, s * 0.75, 0, 0, s * 0.75], 'rgba(255,255,255,0.28)');
  poly(ctx, [0, s * 0.88, s * 0.88, 0, s * 0.94, 0, 0, s * 0.94], 'rgba(255,255,255,0.18)');
  ctx.restore();

  ctx.lineWidth = Math.max(1, s * 0.05);
  const rim = ctx.createLinearGradient(0, 0, s, s);
  rim.addColorStop(0, 'rgba(255,255,255,0.95)');
  rim.addColorStop(0.5, rgba(base, 0.6));
  rim.addColorStop(1, 'rgba(255,255,255,0.6)');
  ctx.strokeStyle = rim;
  const h = ctx.lineWidth / 2;
  roundRect(ctx, gap + h, gap + h, w - h * 2, w - h * 2, radius);
  ctx.stroke();
  sparkle(ctx, s * 0.27, s * 0.27, s * 0.12, 0.9);
}

/** Legendary: a jewel set in a bevelled gold frame. */
function royal(ctx: Ctx, base: string, s: number) {
  const gap = Math.max(1, s * 0.025);
  const w = s - gap * 2;
  const radius = s * 0.12;

  const gold = ctx.createLinearGradient(0, 0, s, s);
  gold.addColorStop(0, '#fff6c8');
  gold.addColorStop(0.3, '#ffd34d');
  gold.addColorStop(0.65, '#c8901a');
  gold.addColorStop(1, '#7a5208');
  roundRect(ctx, gap, gap, w, w, radius);
  ctx.fillStyle = gold;
  ctx.fill();

  const f = s * 0.15;
  const fw = s - f * 2;
  const face = ctx.createRadialGradient(s * 0.38, s * 0.35, 0, s / 2, s / 2, s * 0.5);
  face.addColorStop(0, shade(base, 0.45));
  face.addColorStop(0.45, base);
  face.addColorStop(1, shade(base, -0.5));
  roundRect(ctx, f, f, fw, fw, s * 0.07);
  ctx.fillStyle = face;
  ctx.fill();
  ctx.lineWidth = Math.max(1, s * 0.03);
  ctx.strokeStyle = '#6b4500';
  ctx.stroke();
  // Inner bevel of the frame.
  ctx.lineWidth = Math.max(1, s * 0.02);
  ctx.strokeStyle = 'rgba(255,250,220,0.7)';
  roundRect(ctx, gap + s * 0.03, gap + s * 0.03, w - s * 0.06, w - s * 0.06, radius * 0.8);
  ctx.stroke();
  ctx.lineWidth = Math.max(1, s * 0.025);
  ctx.strokeStyle = '#5a3a00';
  roundRect(ctx, gap, gap, w, w, radius);
  ctx.stroke();
  sparkle(ctx, s * 0.36, s * 0.34, s * 0.14, 0.95);
}

/** Cardboard: a matte piece of corrugated kraft board printed in the block colour, with a strip of tape over a corner. */
function cardboard(ctx: Ctx, base: string, s: number) {
  const gap = Math.max(1, s * 0.035);
  const w = s - gap * 2;
  const radius = s * 0.05;
  const rand = rng(hash(base) ^ 0xc0ffee);

  ctx.save();
  roundRect(ctx, gap, gap, w, w, radius);
  ctx.clip();
  // Ink soaked into brown paper: the kraft shows through and dulls it a little.
  ctx.fillStyle = base;
  ctx.fillRect(0, 0, s, s);
  ctx.fillStyle = 'rgba(176,128,72,0.26)';
  ctx.fillRect(0, 0, s, s);
  const light = ctx.createLinearGradient(0, 0, 0, s);
  light.addColorStop(0, 'rgba(255,240,215,0.16)');
  light.addColorStop(1, 'rgba(60,30,0,0.16)');
  ctx.fillStyle = light;
  ctx.fillRect(0, 0, s, s);

  // Flutes of the corrugation pressing through the liner.
  const flute = s * 0.1;
  for (let x = gap + flute * 0.3; x < s; x += flute) {
    ctx.fillStyle = 'rgba(70,35,0,0.09)';
    ctx.fillRect(x, 0, flute * 0.42, s);
    ctx.fillStyle = 'rgba(255,245,225,0.07)';
    ctx.fillRect(x + flute * 0.5, 0, flute * 0.18, s);
  }
  // Paper fibres.
  ctx.lineWidth = Math.max(0.5, s * 0.008);
  for (let i = 0; i < 14; i++) {
    const x = rand() * s;
    const y = rand() * s;
    const a = rand() * Math.PI;
    const l = s * (0.03 + rand() * 0.05);
    ctx.strokeStyle = rand() < 0.5 ? 'rgba(60,30,0,0.16)' : 'rgba(255,240,210,0.14)';
    ctx.beginPath();
    ctx.moveTo(x, y);
    ctx.lineTo(x + Math.cos(a) * l, y + Math.sin(a) * l);
    ctx.stroke();
  }

  // Printed "this way up" arrows in a darker ink.
  ctx.strokeStyle = rgba(shadeHex(base, -0.55), 0.55);
  ctx.lineWidth = Math.max(1, s * 0.035);
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';
  for (const ax of [s * 0.2, s * 0.33]) {
    ctx.beginPath();
    ctx.moveTo(ax, s * 0.8);
    ctx.lineTo(ax, s * 0.62);
    ctx.moveTo(ax - s * 0.05, s * 0.67);
    ctx.lineTo(ax, s * 0.62);
    ctx.lineTo(ax + s * 0.05, s * 0.67);
    ctx.stroke();
  }
  ctx.beginPath();
  ctx.moveTo(s * 0.13, s * 0.85);
  ctx.lineTo(s * 0.4, s * 0.85);
  ctx.stroke();

  // A strip of packing tape across the top-right corner.
  ctx.save();
  ctx.translate(s * 0.8, s * 0.2);
  ctx.rotate(Math.PI / 4);
  const tw = s * 0.62;
  const th = s * 0.2;
  ctx.fillStyle = 'rgba(246,228,186,0.72)';
  ctx.fillRect(-tw / 2, -th / 2, tw, th);
  ctx.fillStyle = 'rgba(255,255,255,0.32)';
  ctx.fillRect(-tw / 2, -th / 2, tw, th * 0.28);
  ctx.fillStyle = 'rgba(120,80,30,0.25)';
  ctx.fillRect(-tw / 2, th / 2 - Math.max(0.5, s * 0.015), tw, Math.max(0.5, s * 0.015));
  ctx.restore();
  ctx.restore();

  // Crisp cut edge: light on top and left, darker all round.
  ctx.lineWidth = Math.max(1, s * 0.03);
  ctx.strokeStyle = shade(base, -0.5);
  const h = ctx.lineWidth / 2;
  roundRect(ctx, gap + h, gap + h, w - h * 2, w - h * 2, radius);
  ctx.stroke();
  ctx.lineWidth = Math.max(1, s * 0.02);
  ctx.strokeStyle = 'rgba(255,240,215,0.35)';
  ctx.beginPath();
  ctx.moveTo(gap + h * 2, s - gap - h * 3);
  ctx.lineTo(gap + h * 2, gap + h * 2);
  ctx.lineTo(s - gap - h * 3, gap + h * 2);
  ctx.stroke();
}

/** Gingerbread: a baked cookie with a coloured glaze, white piped icing around it and a sugar pearl. */
function gingerbread(ctx: Ctx, base: string, s: number) {
  const gap = Math.max(1, s * 0.03);
  const w = s - gap * 2;
  const radius = s * 0.2;
  const rand = rng(hash(base) ^ 0x6b1d);

  ctx.save();
  roundRect(ctx, gap, gap, w, w, radius);
  ctx.clip();
  const dough = ctx.createRadialGradient(s * 0.38, s * 0.32, s * 0.05, s / 2, s / 2, s * 0.72);
  dough.addColorStop(0, '#d9944f');
  dough.addColorStop(0.55, '#b66a30');
  dough.addColorStop(1, '#7d3f17');
  ctx.fillStyle = dough;
  ctx.fillRect(0, 0, s, s);
  // Baked freckles and spice specks.
  for (let i = 0; i < 16; i++) {
    ctx.fillStyle = rand() < 0.7 ? 'rgba(92,40,10,0.45)' : 'rgba(255,214,160,0.35)';
    ctx.beginPath();
    ctx.arc(gap + rand() * w, gap + rand() * w, s * (0.008 + rand() * 0.012), 0, Math.PI * 2);
    ctx.fill();
  }

  // Coloured glaze in the middle.
  const g0 = s * 0.2;
  const gw = s - g0 * 2;
  const gr = s * 0.13;
  roundRect(ctx, g0 + s * 0.01, g0 + s * 0.025, gw, gw, gr);
  ctx.fillStyle = 'rgba(70,28,6,0.4)';
  ctx.fill();
  const glaze = ctx.createLinearGradient(0, g0, 0, g0 + gw);
  glaze.addColorStop(0, shade(base, 0.3));
  glaze.addColorStop(0.5, base);
  glaze.addColorStop(1, shade(base, -0.18));
  roundRect(ctx, g0, g0, gw, gw, gr);
  ctx.fillStyle = glaze;
  ctx.fill();
  ctx.save();
  ctx.clip();
  ctx.fillStyle = 'rgba(255,255,255,0.38)';
  ctx.beginPath();
  ctx.ellipse(s * 0.42, s * 0.3, s * 0.2, s * 0.07, -0.25, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();

  // Piped royal icing: a wavy white line framing the glaze, with a soft shadow.
  const pi = s * 0.125;
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';
  ctx.lineWidth = Math.max(1.2, s * 0.05);
  ctx.strokeStyle = 'rgba(80,30,5,0.4)';
  ctx.save();
  ctx.translate(s * 0.01, s * 0.02);
  wavyRoundRect(ctx, pi, pi, s - pi * 2, s - pi * 2, s * 0.15, s * 0.022, s * 0.11);
  ctx.stroke();
  ctx.restore();
  ctx.strokeStyle = '#fffaf2';
  wavyRoundRect(ctx, pi, pi, s - pi * 2, s - pi * 2, s * 0.15, s * 0.022, s * 0.11);
  ctx.stroke();
  ctx.restore();

  // A sugar pearl on the glaze.
  const px = s * 0.66;
  const py = s * 0.64;
  const pr = s * 0.065;
  const pearl = ctx.createRadialGradient(px - pr * 0.35, py - pr * 0.4, 0, px, py, pr);
  pearl.addColorStop(0, '#ffffff');
  pearl.addColorStop(0.6, '#f4ede4');
  pearl.addColorStop(1, '#bfae9c');
  ctx.fillStyle = 'rgba(60,20,0,0.3)';
  ctx.beginPath();
  ctx.arc(px + pr * 0.15, py + pr * 0.3, pr, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = pearl;
  ctx.beginPath();
  ctx.arc(px, py, pr, 0, Math.PI * 2);
  ctx.fill();

  ctx.lineWidth = Math.max(1, s * 0.03);
  ctx.strokeStyle = '#5a2a0c';
  const h = ctx.lineWidth / 2;
  roundRect(ctx, gap + h, gap + h, w - h * 2, w - h * 2, radius);
  ctx.stroke();
}

/** Wooden: two stained planks with flowing grain, a knot and a satin varnish. */
function wood(ctx: Ctx, base: string, s: number) {
  const gap = Math.max(1, s * 0.03);
  const w = s - gap * 2;
  const radius = s * 0.07;
  const rand = rng(hash(base) ^ 0x0a4);

  ctx.save();
  roundRect(ctx, gap, gap, w, w, radius);
  ctx.clip();
  ctx.fillStyle = shade(base, 0.08);
  ctx.fillRect(0, 0, s, s);

  // The stain soaks into the wood: multiply light and dark wood tones over the colour.
  ctx.globalCompositeOperation = 'multiply';
  const tone = ctx.createLinearGradient(0, 0, s, 0);
  tone.addColorStop(0, '#f2d3a8');
  tone.addColorStop(0.5, '#ffe9c8');
  tone.addColorStop(1, '#e6be8a');
  ctx.fillStyle = tone;
  ctx.fillRect(0, 0, s, s);
  const half = s / 2;
  // A knot on one of the planks; the grain flows around it.
  const kx = s * (0.3 + rand() * 0.4);
  const knotPlank = rand() < 0.5 ? 0 : 1;
  const ky = (knotPlank + 0.5) * half;
  for (let plank = 0; plank < 2; plank++) {
    const y0 = plank * half;
    // Slight tone difference between the planks.
    ctx.fillStyle = plank ? 'rgba(170,120,70,0.22)' : 'rgba(255,230,190,0.18)';
    ctx.fillRect(0, y0, s, half);
    // Grain lines running along the plank: long gentle waves, darker late wood and fine streaks.
    const lines = 7;
    const phase = rand() * Math.PI * 2;
    const freq = (0.8 + rand()) * (Math.PI / s);
    for (let i = 0; i < lines; i++) {
      const yy = y0 + ((i + 0.25 + rand() * 0.5) / lines) * half;
      const amp = s * (0.01 + rand() * 0.018);
      const fine = i % 2 === 1;
      ctx.strokeStyle = `rgba(90,45,10,${fine ? 0.15 + rand() * 0.12 : 0.3 + rand() * 0.25})`;
      ctx.lineWidth = Math.max(0.5, s * (fine ? 0.007 : 0.012 + rand() * 0.014));
      const away = plank === knotPlank ? Math.sign(yy - ky) || 1 : 0;
      const push = away * s * 0.07 * Math.max(0, 1 - Math.abs(yy - ky) / (half * 0.75));
      ctx.beginPath();
      for (let x = 0; x <= s; x += s / 24) {
        const bump = push * Math.exp(-(((x - kx) / (s * 0.13)) ** 2));
        const y = yy + Math.sin(x * freq + phase + i * 0.45) * amp + Math.sin(x * freq * 3.1 + i) * amp * 0.25 + bump;
        if (x === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();
    }
  }
  for (let i = 3; i >= 1; i--) {
    ctx.strokeStyle = `rgba(80,38,8,${0.22 + 0.1 * (3 - i)})`;
    ctx.lineWidth = Math.max(0.6, s * 0.014);
    ctx.beginPath();
    ctx.ellipse(kx, ky, s * 0.045 * i, s * 0.022 * i, 0, 0, Math.PI * 2);
    ctx.stroke();
  }
  ctx.fillStyle = 'rgba(70,30,5,0.55)';
  ctx.beginPath();
  ctx.ellipse(kx, ky, s * 0.03, s * 0.014, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.globalCompositeOperation = 'source-over';

  // Seam between the planks.
  ctx.fillStyle = rgba(shadeHex(base, -0.7), 0.6);
  ctx.fillRect(0, half - Math.max(0.75, s * 0.016), s, Math.max(1, s * 0.03));
  ctx.fillStyle = 'rgba(255,240,215,0.25)';
  ctx.fillRect(0, half + Math.max(0.75, s * 0.016), s, Math.max(0.5, s * 0.012));
  // Satin varnish sheen.
  const sheen = ctx.createLinearGradient(0, 0, 0, s);
  sheen.addColorStop(0, 'rgba(255,255,255,0.28)');
  sheen.addColorStop(0.18, 'rgba(255,255,255,0.06)');
  sheen.addColorStop(0.5, 'rgba(255,255,255,0)');
  sheen.addColorStop(0.62, 'rgba(255,255,255,0.08)');
  sheen.addColorStop(1, 'rgba(0,0,0,0.18)');
  ctx.fillStyle = sheen;
  ctx.fillRect(0, 0, s, s);
  ctx.restore();

  // Rounded-over edges.
  ctx.lineWidth = Math.max(1, s * 0.03);
  ctx.strokeStyle = shade(base, -0.6);
  const h = ctx.lineWidth / 2;
  roundRect(ctx, gap + h, gap + h, w - h * 2, w - h * 2, radius);
  ctx.stroke();
  ctx.lineWidth = Math.max(1, s * 0.022);
  ctx.strokeStyle = 'rgba(255,236,205,0.4)';
  roundRect(ctx, gap + h * 2.4, gap + h * 2.4, w - h * 4.8, w - h * 4.8, radius * 0.7);
  ctx.stroke();
}

/** A domed steel rivet. */
export function rivet(ctx: Ctx, x: number, y: number, r: number) {
  ctx.fillStyle = 'rgba(0,0,0,0.45)';
  ctx.beginPath();
  ctx.arc(x + r * 0.25, y + r * 0.35, r * 1.05, 0, Math.PI * 2);
  ctx.fill();
  const g = ctx.createRadialGradient(x - r * 0.35, y - r * 0.4, 0, x, y, r);
  g.addColorStop(0, '#ffffff');
  g.addColorStop(0.35, '#d5dbe2');
  g.addColorStop(0.8, '#7b838e');
  g.addColorStop(1, '#4a5058');
  ctx.fillStyle = g;
  ctx.beginPath();
  ctx.arc(x, y, r, 0, Math.PI * 2);
  ctx.fill();
}

/** Iron: a painted, brushed metal plate with a hard bevel and a rivet in each corner. */
function iron(ctx: Ctx, base: string, s: number) {
  const gap = Math.max(1, s * 0.025);
  const x0 = gap;
  const x1 = s - gap;
  const radius = s * 0.06;
  const bevel = s * 0.1;
  const rand = rng(hash(base) ^ 0x1e0);

  ctx.save();
  roundRect(ctx, x0, x0, x1 - x0, x1 - x0, radius);
  ctx.clip();
  const plate = ctx.createLinearGradient(0, 0, s, s);
  plate.addColorStop(0, shade(base, 0.3));
  plate.addColorStop(0.45, base);
  plate.addColorStop(1, shade(base, -0.35));
  ctx.fillStyle = plate;
  ctx.fillRect(0, 0, s, s);
  // Brushed finish: fine horizontal scratches.
  for (let i = 0; i < 34; i++) {
    const y = rand() * s;
    const x = rand() * s * 0.6 - s * 0.1;
    ctx.fillStyle = rand() < 0.5 ? `rgba(255,255,255,${0.05 + rand() * 0.1})` : `rgba(0,0,0,${0.05 + rand() * 0.1})`;
    ctx.fillRect(x, y, s * (0.4 + rand() * 0.6), Math.max(0.5, s * 0.008));
  }
  // Metallic sheen band.
  const sheen = ctx.createLinearGradient(0, 0, s, s);
  sheen.addColorStop(0.2, 'rgba(255,255,255,0)');
  sheen.addColorStop(0.38, 'rgba(255,255,255,0.32)');
  sheen.addColorStop(0.46, 'rgba(255,255,255,0.05)');
  sheen.addColorStop(0.6, 'rgba(255,255,255,0.14)');
  sheen.addColorStop(0.75, 'rgba(255,255,255,0)');
  ctx.fillStyle = sheen;
  ctx.fillRect(0, 0, s, s);

  const i0 = x0 + bevel;
  const i1 = x1 - bevel;
  poly(ctx, [x0, x0, x1, x0, i1, i0, i0, i0], 'rgba(255,255,255,0.45)'); // top
  poly(ctx, [x0, x0, i0, i0, i0, i1, x0, x1], 'rgba(255,255,255,0.2)'); // left
  poly(ctx, [x1, x0, x1, x1, i1, i1, i1, i0], 'rgba(0,0,0,0.25)'); // right
  poly(ctx, [x0, x1, i0, i1, i1, i1, x1, x1], 'rgba(0,0,0,0.42)'); // bottom
  ctx.restore();

  // Engraved line around the face.
  ctx.lineWidth = Math.max(1, s * 0.018);
  ctx.strokeStyle = rgba(shadeHex(base, -0.7), 0.55);
  ctx.strokeRect(i0, i0, i1 - i0, i1 - i0);
  ctx.strokeStyle = 'rgba(255,255,255,0.25)';
  ctx.strokeRect(i0 + ctx.lineWidth, i0 + ctx.lineWidth, i1 - i0 - ctx.lineWidth * 2, i1 - i0 - ctx.lineWidth * 2);

  const rv = s * 0.215;
  for (const [x, y] of [[rv, rv], [s - rv, rv], [rv, s - rv], [s - rv, s - rv]]) rivet(ctx, x, y, s * 0.055);

  ctx.lineWidth = Math.max(1, s * 0.025);
  ctx.strokeStyle = shade(base, -0.65);
  roundRect(ctx, x0 + ctx.lineWidth / 2, x0 + ctx.lineWidth / 2, x1 - x0 - ctx.lineWidth, x1 - x0 - ctx.lineWidth, radius);
  ctx.stroke();
}

const DRAW: Record<BlockKind, (ctx: Ctx, base: string, s: number) => void> = {
  jewel,
  cardboard,
  candy,
  gingerbread,
  wood,
  gem,
  iron,
  neon,
  glass,
  royal,
};
