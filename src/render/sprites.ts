import { colorFor, shade } from './palette';

/**
 * Procedurally generated "jewel" block graphics: a bevelled square with a
 * light top-left edge, dark bottom-right edge and a soft glossy face. Sprites
 * are rendered once per colour/size and cached.
 */
const cache = new Map<string, HTMLCanvasElement>();

export function blockSprite(color: number, size: number, dpr: number): HTMLCanvasElement {
  const px = Math.max(4, Math.round(size * dpr));
  const key = `${color}:${px}`;
  let sprite = cache.get(key);
  if (!sprite) {
    sprite = drawBlock(colorFor(color), px);
    cache.set(key, sprite);
  }
  return sprite;
}

export function clearSpriteCache(): void {
  cache.clear();
}

function roundRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

function drawBlock(base: string, s: number): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = s;
  const ctx = canvas.getContext('2d')!;
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

  const quad = (pts: number[], fill: string) => {
    ctx.beginPath();
    ctx.moveTo(pts[0], pts[1]);
    for (let i = 2; i < pts.length; i += 2) ctx.lineTo(pts[i], pts[i + 1]);
    ctx.closePath();
    ctx.fillStyle = fill;
    ctx.fill();
  };
  const i0 = x0 + bevel;
  const i1 = x1 - bevel;
  quad([x0, x0, x1, x0, i1, i0, i0, i0], shade(base, 0.42)); // top
  quad([x0, x0, i0, i0, i0, i1, x0, x1], shade(base, 0.2)); // left
  quad([x1, x0, x1, x1, i1, i1, i1, i0], shade(base, -0.18)); // right
  quad([x0, x1, i0, i1, i1, i1, x1, x1], shade(base, -0.32)); // bottom

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
  return canvas;
}
