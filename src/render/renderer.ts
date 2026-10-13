import { type Board, type Lines, SIZE, idx } from '../game/board';
import type { Piece } from '../game/game';
import type { Shape } from '../game/shapes';
import type { Layout } from './layout';
import { rgba } from './palette';
import { blockSprite } from './sprites';
import type { ParticleShape, Style } from './styles';

export interface DragView {
  piece: Piece;
  /** Top-left of the piece in CSS pixels. */
  x: number;
  y: number;
  cell: number;
  /** Snap target on the board, if the piece can be dropped there. */
  target: { r: number; c: number; lines: Lines } | null;
}

export interface FlyingPiece {
  piece: Piece;
  from: { x: number; y: number; cell: number };
  slot: number;
  start: number;
  duration: number;
}

export interface ClearingCell {
  r: number;
  c: number;
  color: number;
  delay: number;
}

export interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  size: number;
  color: string;
  born: number;
  life: number;
  shape: ParticleShape;
  gravity: number;
  /** Rotation speed in radians per second. */
  spin: number;
}

/** An expanding shockwave ring. */
export interface Ring {
  x: number;
  y: number;
  radius: number;
  color: string;
  start: number;
  duration: number;
}

/** A light beam along a cleared row or column. */
export interface Beam {
  row?: number;
  col?: number;
  color: string;
  start: number;
}

export interface FloatingText {
  text: string;
  x: number;
  y: number;
  size: number;
  color: string;
  start: number;
  duration: number;
  /** Big centred banners pop in; small popups just float up. */
  banner?: boolean;
}

export interface Scene {
  layout: Layout;
  dpr: number;
  now: number;
  style: Style;
  /** Start of the shimmer that sweeps the board when the style changes. */
  styleWaveAt: number | null;
  board: Board;
  tray: (Piece | null)[];
  playable: boolean[];
  hiddenSlot: number;
  drag: DragView | null;
  flying: FlyingPiece | null;
  trayDealtAt: number;
  placed: { cells: Set<number>; start: number } | null;
  clearing: { cells: ClearingCell[]; start: number } | null;
  particles: Particle[];
  rings: Ring[];
  beams: Beam[];
  texts: FloatingText[];
  shake: { start: number; power: number } | null;
  gameOverAt: number | null;
}

export const CLEAR_DURATION = 420;
export const PLACE_DURATION = 180;
export const DEAL_DURATION = 320;
export const GAME_OVER_FILL = 900;
export const BEAM_DURATION = 520;
export const STYLE_WAVE_DURATION = 1100;

const easeOutBack = (t: number) => 1 + 2.2 * Math.pow(t - 1, 3) + 1.2 * Math.pow(t - 1, 2);
const easeOutCubic = (t: number) => 1 - Math.pow(1 - t, 3);
const clamp01 = (t: number) => Math.max(0, Math.min(1, t));

export function trayPiecePosition(layout: Layout, slot: number, shape: Shape) {
  const s = layout.slots[slot];
  const cell = layout.trayCell;
  return { x: s.cx - (shape.cols * cell) / 2, y: s.cy - (shape.rows * cell) / 2, cell };
}

export class Renderer {
  private ctx: CanvasRenderingContext2D;
  private boardCache: HTMLCanvasElement | null = null;
  private boardCacheKey = '';
  private dpr = 1;
  /** Blocks of every size are drawn from one sprite per colour at board-cell size. */
  private spriteSize = 0;
  private style!: Style;

  constructor(private canvas: HTMLCanvasElement) {
    this.ctx = canvas.getContext('2d')!;
  }

  /** True while any time-based effect still needs frames. */
  isAnimating(scene: Scene): boolean {
    const t = scene.now;
    return (
      !!scene.flying ||
      !!scene.placed ||
      !!scene.clearing ||
      scene.particles.length > 0 ||
      scene.rings.length > 0 ||
      scene.beams.length > 0 ||
      scene.texts.length > 0 ||
      scene.styleWaveAt !== null ||
      !!scene.shake ||
      t - scene.trayDealtAt < DEAL_DURATION + 200 ||
      (scene.gameOverAt !== null && t - scene.gameOverAt < GAME_OVER_FILL + 100)
    );
  }

  render(scene: Scene): void {
    const { ctx, canvas } = this;
    const { layout, dpr, now } = scene;
    this.dpr = dpr;
    this.spriteSize = layout.cell;
    this.style = scene.style;
    const w = Math.round(layout.width * dpr);
    const h = Math.round(layout.height * dpr);
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
    }
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, w, h);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    this.expire(scene);

    ctx.save();
    if (scene.shake) {
      const t = (now - scene.shake.start) / 350;
      const p = scene.shake.power * (1 - t);
      ctx.translate(Math.sin(t * 45) * p, Math.cos(t * 38) * p * 0.6);
    }
    this.drawBoard(scene);
    this.drawBeams(scene);
    this.drawStyleWave(scene);
    ctx.restore();

    this.drawTray(scene);
    this.drawRings(scene);
    this.drawParticles(scene);
    if (scene.flying) this.drawFlying(scene, scene.flying);
    if (scene.drag) this.drawPiece(scene.drag.piece.shape, scene.drag.piece.color, scene.drag.x, scene.drag.y, scene.drag.cell, 1, true);
    this.drawTexts(scene);
  }

  private expire(scene: Scene): void {
    const t = scene.now;
    if (scene.placed && t - scene.placed.start > PLACE_DURATION) scene.placed = null;
    if (scene.clearing) {
      const maxDelay = Math.max(0, ...scene.clearing.cells.map((c) => c.delay));
      if (t - scene.clearing.start > CLEAR_DURATION + maxDelay) scene.clearing = null;
    }
    if (scene.shake && t - scene.shake.start > 350) scene.shake = null;
    if (scene.flying && t - scene.flying.start > scene.flying.duration) scene.flying = null;
    if (scene.styleWaveAt !== null && t - scene.styleWaveAt > STYLE_WAVE_DURATION) scene.styleWaveAt = null;
    scene.particles = scene.particles.filter((p) => t - p.born < p.life);
    scene.rings = scene.rings.filter((r) => t - r.start < r.duration);
    scene.beams = scene.beams.filter((b) => t - b.start < BEAM_DURATION);
    scene.texts = scene.texts.filter((p) => t - p.start < p.duration);
  }

  /** Extra room around the board for frames and glows, in CSS pixels. */
  private boardMargin(layout: Layout): number {
    return Math.round(layout.cell * 0.6);
  }

  private boardBackground(layout: Layout, dpr: number, style: Style): HTMLCanvasElement {
    const key = `${layout.boardSize}:${dpr}:${style.key}`;
    if (this.boardCache && this.boardCacheKey === key) return this.boardCache;
    const { palette: p, tier } = style;
    const m = this.boardMargin(layout);
    const pad = Math.round(layout.cell * 0.18);
    const size = layout.boardSize + pad * 2;
    const c = document.createElement('canvas');
    c.width = c.height = Math.round((size + m * 2) * dpr);
    const g = c.getContext('2d')!;
    g.scale(dpr, dpr);
    g.translate(m, m);
    const cell = layout.cell;
    const outerR = cell * (tier.frame === 'soft' ? 0.45 : 0.3);
    const innerR = cell * (tier.frame === 'soft' ? 0.38 : 0.25);

    // Outer frame.
    g.save();
    if (tier.frame === 'neon' || tier.frame === 'glass' || tier.frame === 'gold') {
      g.shadowColor = tier.frame === 'gold' ? rgba(p.glow, 0.8) : p.glow;
      g.shadowBlur = cell * (tier.frame === 'glass' ? 0.6 : 0.45);
    }
    if (tier.frame === 'metal' || tier.frame === 'gold') {
      const frame = g.createLinearGradient(0, 0, size, size);
      const stops = tier.frame === 'gold' ? ['#fff3b0', '#e2b13c', '#8a5d0a', '#f5d36b', '#6b4500'] : ['#f2f4ff', '#9aa3c0', '#4b5272', '#c9cfe6', '#3a3f58'];
      stops.forEach((col, i) => frame.addColorStop(i / (stops.length - 1), col));
      g.fillStyle = frame;
    } else if (tier.frame === 'glass') {
      g.fillStyle = rgba(p.boardEdge, 0.75);
    } else {
      g.fillStyle = p.boardEdge;
    }
    roundRect(g, 0, 0, size, size, outerR);
    g.fill();
    g.restore();
    if (tier.frame === 'neon') {
      g.save();
      g.shadowColor = p.glow;
      g.shadowBlur = cell * 0.3;
      g.lineWidth = Math.max(2, cell * 0.06);
      g.strokeStyle = p.glow;
      roundRect(g, g.lineWidth / 2, g.lineWidth / 2, size - g.lineWidth, size - g.lineWidth, outerR);
      g.stroke();
      g.restore();
    }

    // Board surface.
    const inset = tier.frame === 'metal' || tier.frame === 'gold' ? pad * 0.5 : pad * 0.35;
    g.fillStyle = tier.frame === 'glass' ? rgba(p.board, 0.82) : p.board;
    roundRect(g, inset, inset, size - inset * 2, size - inset * 2, innerR);
    g.fill();
    if (tier.frame === 'glass') {
      g.lineWidth = Math.max(1, cell * 0.03);
      g.strokeStyle = 'rgba(255,255,255,0.35)';
      g.stroke();
    }

    // Cells.
    const ci = Math.max(1, cell * 0.04);
    for (let r = 0; r < SIZE; r++) {
      for (let col = 0; col < SIZE; col++) {
        const x = pad + col * cell + ci;
        const y = pad + r * cell + ci;
        const w = cell - ci * 2;
        if (tier.frame === 'neon') {
          g.strokeStyle = rgba(p.glow, 0.16);
          g.lineWidth = Math.max(1, cell * 0.025);
          roundRect(g, x + ci, y + ci, w - ci * 2, w - ci * 2, cell * 0.1);
          g.fillStyle = p.cellInner;
          g.fill();
          g.stroke();
          continue;
        }
        const cr = cell * (tier.frame === 'soft' ? 0.2 : 0.1);
        g.fillStyle = p.cell;
        roundRect(g, x, y, w, w, cr);
        g.fill();
        g.fillStyle = p.cellInner;
        roundRect(g, x + ci, y + ci * 2, w - ci * 2, w - ci * 2, cr * 0.8);
        g.fill();
      }
    }
    this.boardCache = c;
    this.boardCacheKey = key;
    return c;
  }

  private drawBoard(scene: Scene): void {
    const { ctx } = this;
    const { layout, board, drag, now, dpr, style } = scene;
    const bg = this.boardBackground(layout, dpr, style);
    const pad = Math.round(layout.cell * 0.18) + this.boardMargin(layout);
    ctx.drawImage(bg, layout.boardX - pad, layout.boardY - pad, layout.boardSize + pad * 2, layout.boardSize + pad * 2);

    const cell = layout.cell;
    const target = drag?.target ?? null;
    const highlight = new Set<number>();
    if (target) {
      for (const r of target.lines.rows) for (let c = 0; c < SIZE; c++) highlight.add(idx(r, c));
      for (const c of target.lines.cols) for (let r = 0; r < SIZE; r++) highlight.add(idx(r, c));
    }
    const ghost = new Set<number>();
    if (target && drag) for (const [dr, dc] of drag.piece.shape.cells) ghost.add(idx(target.r + dr, target.c + dc));

    const grayRows = scene.gameOverAt === null ? -1 : Math.floor(clamp01((now - scene.gameOverAt) / GAME_OVER_FILL) * (SIZE + 1));

    for (let r = 0; r < SIZE; r++) {
      for (let c = 0; c < SIZE; c++) {
        const i = idx(r, c);
        let color = board[i];
        const x = layout.boardX + c * cell;
        const y = layout.boardY + r * cell;
        if (highlight.has(i) && drag && (color || ghost.has(i))) {
          // Blocks in lines about to be cleared take the colour of the dragged piece.
          this.blit(drag.piece.color, x, y, cell, dpr, 1);
          continue;
        }
        if (!color) {
          if (ghost.has(i) && drag) this.blit(drag.piece.color, x, y, cell, dpr, 0.4);
          continue;
        }
        if (grayRows > SIZE - 1 - r) color = 0;
        let scale = 1;
        if (scene.placed?.cells.has(i)) {
          const t = clamp01((now - scene.placed.start) / PLACE_DURATION);
          scale = 0.85 + 0.15 * easeOutBack(t);
        }
        this.blit(color, x, y, cell, dpr, 1, scale);
      }
    }

    if (scene.clearing) {
      for (const cc of scene.clearing.cells) {
        const t = clamp01((now - scene.clearing.start - cc.delay) / CLEAR_DURATION);
        if (t >= 1) continue;
        const x = layout.boardX + cc.c * cell;
        const y = layout.boardY + cc.r * cell;
        const scale = t < 0.25 ? 1 + t * 0.6 : 1.15 * (1 - easeOutCubic((t - 0.25) / 0.75));
        this.blit(cc.color, x, y, cell, dpr, 1 - t * 0.3, scale);
        if (t < 0.3) {
          ctx.fillStyle = rgba(style.tier.fx.flash, 0.7 * (1 - t / 0.3));
          ctx.fillRect(x + cell * 0.08, y + cell * 0.08, cell * 0.84, cell * 0.84);
        }
      }
    }
  }

  private blit(color: number, x: number, y: number, size: number, dpr: number, alpha = 1, scale = 1): void {
    const { ctx } = this;
    // Animated pieces change size every frame; caching a sprite per size would
    // create hundreds of canvases, so they all scale the board-sized one.
    const sprite = blockSprite(color, this.spriteSize, dpr, this.style);
    const s = size * scale;
    const o = (size - s) / 2;
    if (alpha !== 1) ctx.globalAlpha = alpha;
    ctx.drawImage(sprite, x + o, y + o, s, s);
    if (alpha !== 1) ctx.globalAlpha = 1;
  }

  drawPiece(shape: Shape, color: number, x: number, y: number, cell: number, alpha = 1, shadow = false): void {
    const { ctx } = this;
    const dpr = this.dpr;
    if (shadow) {
      ctx.fillStyle = 'rgba(0,0,0,0.18)';
      for (const [r, c] of shape.cells) ctx.fillRect(x + c * cell + cell * 0.12, y + r * cell + cell * 0.2, cell * 0.9, cell * 0.9);
    }
    for (const [r, c] of shape.cells) this.blit(color, x + c * cell, y + r * cell, cell, dpr, alpha);
  }

  private drawTray(scene: Scene): void {
    const { layout, tray, now } = scene;
    tray.forEach((piece, i) => {
      if (!piece || i === scene.hiddenSlot) return;
      if (scene.flying?.slot === i) return;
      const pos = trayPiecePosition(layout, i, piece.shape);
      const t = clamp01((now - scene.trayDealtAt - i * 60) / DEAL_DURATION);
      const offset = (1 - easeOutCubic(t)) * layout.width * 0.6;
      const alpha = (scene.playable[i] ? 1 : 0.35) * clamp01(t * 2);
      if (alpha <= 0) return;
      this.drawPiece(piece.shape, scene.playable[i] ? piece.color : 0, pos.x + offset, pos.y, pos.cell, alpha);
    });
  }

  private drawFlying(scene: Scene, f: FlyingPiece): void {
    const to = trayPiecePosition(scene.layout, f.slot, f.piece.shape);
    const t = easeOutCubic(clamp01((scene.now - f.start) / f.duration));
    const lerp = (a: number, b: number) => a + (b - a) * t;
    this.drawPiece(f.piece.shape, f.piece.color, lerp(f.from.x, to.x), lerp(f.from.y, to.y), lerp(f.from.cell, to.cell));
  }

  private drawParticles(scene: Scene): void {
    const { ctx } = this;
    const glow = scene.style.tier.fx.glow;
    if (glow) ctx.globalCompositeOperation = 'lighter';
    for (const p of scene.particles) {
      if (scene.now < p.born) continue;
      const age = (scene.now - p.born) / 1000;
      const k = 1 - (scene.now - p.born) / p.life;
      const x = p.x + p.vx * age;
      const y = p.y + p.vy * age + p.gravity * age * age;
      const s = p.size * (0.4 + 0.6 * k);
      ctx.fillStyle = rgba(p.color, Math.max(0, k));
      switch (p.shape) {
        case 'square':
          ctx.fillRect(x - s / 2, y - s / 2, s, s);
          break;
        case 'circle':
          ctx.beginPath();
          ctx.arc(x, y, s / 2, 0, Math.PI * 2);
          ctx.fill();
          break;
        case 'spark': {
          // A streak pointing along the direction of travel.
          const vx = p.vx;
          const vy = p.vy + 2 * p.gravity * age;
          const len = Math.hypot(vx, vy) || 1;
          const tail = s * 2.2;
          ctx.strokeStyle = ctx.fillStyle;
          ctx.lineWidth = Math.max(1, s * 0.35);
          ctx.lineCap = 'round';
          ctx.beginPath();
          ctx.moveTo(x, y);
          ctx.lineTo(x - (vx / len) * tail, y - (vy / len) * tail);
          ctx.stroke();
          break;
        }
        default: {
          ctx.save();
          ctx.translate(x, y);
          ctx.rotate(p.spin * age);
          ctx.beginPath();
          if (p.shape === 'diamond') {
            ctx.moveTo(0, -s * 0.7);
            ctx.lineTo(s * 0.45, 0);
            ctx.lineTo(0, s * 0.7);
            ctx.lineTo(-s * 0.45, 0);
          } else {
            // Five-pointed star.
            for (let i = 0; i < 10; i++) {
              const r = i % 2 ? s * 0.28 : s * 0.7;
              const a = (i * Math.PI) / 5 - Math.PI / 2;
              ctx.lineTo(Math.cos(a) * r, Math.sin(a) * r);
            }
          }
          ctx.closePath();
          ctx.fill();
          ctx.restore();
        }
      }
    }
    if (glow) ctx.globalCompositeOperation = 'source-over';
  }

  private drawRings(scene: Scene): void {
    const { ctx } = this;
    if (!scene.rings.length) return;
    if (scene.style.tier.fx.glow) ctx.globalCompositeOperation = 'lighter';
    for (const ring of scene.rings) {
      const t = clamp01((scene.now - ring.start) / ring.duration);
      if (t <= 0) continue;
      ctx.strokeStyle = rgba(ring.color, 0.85 * (1 - t));
      ctx.lineWidth = Math.max(1, scene.layout.cell * 0.22 * (1 - t));
      ctx.beginPath();
      ctx.arc(ring.x, ring.y, ring.radius * easeOutCubic(t), 0, Math.PI * 2);
      ctx.stroke();
    }
    ctx.globalCompositeOperation = 'source-over';
  }

  private drawBeams(scene: Scene): void {
    const { ctx } = this;
    if (!scene.beams.length) return;
    const { layout } = scene;
    const { cell, boardX, boardY, boardSize } = layout;
    ctx.globalCompositeOperation = 'lighter';
    for (const b of scene.beams) {
      const t = clamp01((scene.now - b.start) / BEAM_DURATION);
      if (t <= 0) continue;
      // The beam flares open and then thins out.
      const width = cell * (t < 0.2 ? t / 0.2 : 1 - easeOutCubic((t - 0.2) / 0.8)) * 1.1;
      const alpha = 0.9 * (1 - t);
      const horizontal = b.row !== undefined;
      const mid = horizontal ? boardY + (b.row! + 0.5) * cell : boardX + (b.col! + 0.5) * cell;
      const grad = horizontal ? ctx.createLinearGradient(0, mid - width, 0, mid + width) : ctx.createLinearGradient(mid - width, 0, mid + width, 0);
      grad.addColorStop(0, rgba(b.color, 0));
      grad.addColorStop(0.5, rgba(b.color, alpha));
      grad.addColorStop(1, rgba(b.color, 0));
      ctx.fillStyle = grad;
      const over = cell * 0.6 * (1 - t);
      if (horizontal) ctx.fillRect(boardX - over, mid - width, boardSize + over * 2, width * 2);
      else ctx.fillRect(mid - width, boardY - over, width * 2, boardSize + over * 2);
      // Bright core.
      ctx.fillStyle = `rgba(255,255,255,${alpha * 0.8})`;
      const core = Math.max(1, width * 0.12);
      if (horizontal) ctx.fillRect(boardX, mid - core, boardSize, core * 2);
      else ctx.fillRect(mid - core, boardY, core * 2, boardSize);
    }
    ctx.globalCompositeOperation = 'source-over';
  }

  /** A diagonal shimmer across the board, revealing the new style. */
  private drawStyleWave(scene: Scene): void {
    if (scene.styleWaveAt === null) return;
    const { ctx } = this;
    const { layout, style } = scene;
    const t = (scene.now - scene.styleWaveAt) / STYLE_WAVE_DURATION;
    if (t < 0 || t > 1) return;
    const { cell, boardX, boardY } = layout;
    ctx.globalCompositeOperation = 'lighter';
    for (let r = 0; r < SIZE; r++) {
      for (let c = 0; c < SIZE; c++) {
        // Each cell lights up as the wave front passes it.
        const local = (t * 1.6 - (r + c) / (2 * SIZE - 2) * 0.9) / 0.7;
        if (local <= 0 || local >= 1) continue;
        const a = Math.sin(local * Math.PI);
        const inset = cell * (0.5 - 0.42 * a);
        ctx.fillStyle = rgba(local < 0.5 ? '#ffffff' : style.palette.glow, 0.55 * a);
        roundRect(ctx, boardX + c * cell + inset, boardY + r * cell + inset, cell - inset * 2, cell - inset * 2, cell * 0.12);
        ctx.fill();
      }
    }
    ctx.globalCompositeOperation = 'source-over';
  }

  private drawTexts(scene: Scene): void {
    const { ctx } = this;
    const { tier, palette } = scene.style;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    for (const ft of scene.texts) {
      if (scene.now < ft.start) continue;
      const t = clamp01((scene.now - ft.start) / ft.duration);
      let scale = 1;
      let dy = 0;
      let alpha = 1;
      if (ft.banner) {
        scale = t < 0.2 ? easeOutBack(t / 0.2) : 1;
        alpha = t > 0.75 ? 1 - (t - 0.75) / 0.25 : 1;
        dy = -t * 20;
      } else {
        dy = -easeOutCubic(t) * 50;
        alpha = t > 0.6 ? 1 - (t - 0.6) / 0.4 : 1;
      }
      ctx.save();
      ctx.globalAlpha = Math.max(0, alpha);
      ctx.translate(ft.x, ft.y + dy);
      ctx.scale(Math.max(0.01, scale), Math.max(0.01, scale));
      // Scale the text down if it would not fit the screen in this style's font.
      ctx.font = `${tier.fontWeight} ${ft.size}px ${tier.font}`;
      const fit = Math.min(1, (scene.layout.width * 0.92) / Math.max(1, ctx.measureText(ft.text).width));
      if (fit < 1) ctx.scale(fit, fit);
      ctx.lineJoin = 'round';
      ctx.lineWidth = Math.max(3, ft.size * 0.14);
      ctx.strokeStyle = tier.level === 0 ? 'rgba(20,24,60,0.85)' : rgba(palette.boardEdge, 0.9);
      if (tier.textGlow) {
        ctx.shadowColor = palette.glow;
        ctx.shadowBlur = ft.size * 0.45;
      }
      ctx.strokeText(ft.text, 0, 0);
      ctx.shadowBlur = 0;
      if (tier.textGradient && ft.banner) {
        const grad = ctx.createLinearGradient(0, -ft.size / 2, 0, ft.size / 2);
        grad.addColorStop(0, '#fffbe6');
        grad.addColorStop(0.45, ft.color);
        grad.addColorStop(0.55, ft.color);
        grad.addColorStop(1, '#b07a10');
        ctx.fillStyle = grad;
      } else {
        ctx.fillStyle = ft.color;
      }
      ctx.fillText(ft.text, 0, 0);
      ctx.restore();
    }
  }
}

export function roundRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}
