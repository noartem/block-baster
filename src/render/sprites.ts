import { colorFor, rgba, shade } from './palette';
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

function roundRect(ctx: Ctx, x: number, y: number, w: number, h: number, r: number) {
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

const DRAW: Record<BlockKind, (ctx: Ctx, base: string, s: number) => void> = { jewel, candy, gem, neon, glass, royal };
