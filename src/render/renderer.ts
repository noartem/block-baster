import { type Board, type Lines, SIZE, idx } from '../game/board';
import type { Piece } from '../game/game';
import type { Shape } from '../game/shapes';
import type { Layout } from './layout';
import { drawMaterialFrame, isMaterialFrame } from './frames';
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
  /**
   * Special particles: puffs of dust, tumbling block debris, confetti and
   * glowing motes. Plain particles leave it out and are drawn as `shape`.
   */
  kind?: 'dust' | 'block' | 'confetti' | 'mote';
  /** Colour id of the block a 'block' particle is drawn as. */
  block?: number;
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

/** A bright streak racing along a cleared line, away from where the piece landed. */
export interface Streak {
  row?: number;
  col?: number;
  /** Position along the line it starts from, in cells. */
  from: number;
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
  /** Keeps wobbling after popping in (combo counters). */
  bounce?: boolean;
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
  /** The tray is a lucky one (see isLuckyDeal) and glows softly. */
  lucky: boolean;
  playable: boolean[];
  hiddenSlot: number;
  drag: DragView | null;
  flying: FlyingPiece | null;
  trayDealtAt: number;
  placed: { cells: Set<number>; start: number } | null;
  /** Blocks near a placed piece bob in a wave spreading out from its centre (in cells). */
  ripple: { r: number; c: number; start: number } | null;
  clearing: { cells: ClearingCell[]; start: number } | null;
  particles: Particle[];
  rings: Ring[];
  beams: Beam[];
  streaks: Streak[];
  texts: FloatingText[];
  shake: { start: number; power: number } | null;
  /** A burst of light where lines were cleared; `power` 0–1. */
  flash: { x: number; y: number; power: number; color: string; start: number } | null;
  /** Glow pulsing in from the screen edges on combos and board clears; `power` 0–1. */
  pulse: { power: number; color: string; start: number } | null;
  gameOverAt: number | null;
}

export const CLEAR_DURATION = 420;
export const PLACE_DURATION = 260;
export const DEAL_DURATION = 320;
export const GAME_OVER_FILL = 900;
export const BEAM_DURATION = 520;
export const STYLE_WAVE_DURATION = 1100;
export const STREAK_DURATION = 380;
export const FLASH_DURATION = 280;
export const PULSE_DURATION = 750;
/** How long each block bobs, and how much later each cell of distance starts. */
const RIPPLE_BOB = 240;
const RIPPLE_STEP = 45;
const RIPPLE_REACH = 4;
/** A row of blocks hops as it turns grey at game over. */
const GAME_OVER_HOP = 260;
/** The shine sweeping over a lucky tray once its pieces have slid in. */
export const LUCKY_SHINE_DELAY = DEAL_DURATION + 80;
const LUCKY_SHINE_DURATION = 650;
const LUCKY_SHINE_STAGGER = 110;

const easeOutBack = (t: number) => 1 + 2.2 * Math.pow(t - 1, 3) + 1.2 * Math.pow(t - 1, 2);
const easeOutCubic = (t: number) => 1 - Math.pow(1 - t, 3);
const clamp01 = (t: number) => Math.max(0, Math.min(1, t));

const motionQuery = typeof matchMedia === 'function' ? matchMedia('(prefers-reduced-motion: reduce)') : null;
/** Players who ask for less motion get calmer screen-wide effects. */
export const prefersReducedMotion = () => !!motionQuery?.matches;

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
      !!scene.ripple ||
      !!scene.clearing ||
      scene.particles.length > 0 ||
      scene.rings.length > 0 ||
      scene.beams.length > 0 ||
      scene.streaks.length > 0 ||
      scene.texts.length > 0 ||
      scene.styleWaveAt !== null ||
      !!scene.shake ||
      !!scene.flash ||
      !!scene.pulse ||
      t - scene.trayDealtAt < DEAL_DURATION + 200 ||
      (scene.lucky && t - scene.trayDealtAt < LUCKY_SHINE_DELAY + LUCKY_SHINE_DURATION + LUCKY_SHINE_STAGGER * 2 + 50) ||
      (scene.gameOverAt !== null && t - scene.gameOverAt < GAME_OVER_FILL + GAME_OVER_HOP + 100)
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
    this.drawClearPreview(scene);
    this.drawBeams(scene);
    this.drawStreaks(scene);
    this.drawStyleWave(scene);
    ctx.restore();

    this.drawFlash(scene);
    this.drawTray(scene);
    this.drawRings(scene);
    this.drawParticles(scene);
    if (scene.flying) this.drawFlying(scene, scene.flying);
    if (scene.drag) {
      this.drawDragHalo(scene, scene.drag);
      this.drawPiece(scene.drag.piece.shape, scene.drag.piece.color, scene.drag.x, scene.drag.y, scene.drag.cell, 1, true);
    }
    this.drawGameOverShade(scene);
    this.drawTexts(scene);
    this.drawPulse(scene);
  }

  private expire(scene: Scene): void {
    const t = scene.now;
    if (scene.placed && t - scene.placed.start > PLACE_DURATION) scene.placed = null;
    if (scene.ripple && t - scene.ripple.start > RIPPLE_BOB + RIPPLE_STEP * RIPPLE_REACH) scene.ripple = null;
    if (scene.flash && t - scene.flash.start > FLASH_DURATION) scene.flash = null;
    if (scene.pulse && t - scene.pulse.start > PULSE_DURATION) scene.pulse = null;
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
    scene.streaks = scene.streaks.filter((b) => t - b.start < STREAK_DURATION);
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
    const material = isMaterialFrame(tier.frame);
    const inset = material ? pad * 0.8 : tier.frame === 'metal' || tier.frame === 'gold' ? pad * 0.5 : pad * 0.35;
    g.fillStyle = tier.frame === 'glass' ? rgba(p.board, 0.82) : p.board;
    roundRect(g, inset, inset, size - inset * 2, size - inset * 2, innerR);
    g.fill();
    // Cookie, cardboard, iron and wood frames are drawn over the plain one.
    if (material) drawMaterialFrame(g, tier.frame, { size, inset, cell, outerR, innerR, palette: p });
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
    const breath = 0.5 + 0.5 * Math.sin(now / 130);
    const ripple = scene.ripple;

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
          // The ghost of the dragged piece breathes softly.
          if (ghost.has(i) && drag) this.blit(drag.piece.color, x, y, cell, dpr, 0.32 + 0.14 * breath);
          continue;
        }
        let sx = 1;
        let sy = 1;
        let dy = 0;
        let impact = 0;
        if (grayRows > SIZE - 1 - r) {
          color = 0;
          // Each row hops as it turns grey, so the game over runs up the board in a wave.
          const hop = (now - scene.gameOverAt! - ((SIZE - r) / (SIZE + 1)) * GAME_OVER_FILL) / GAME_OVER_HOP;
          if (hop > 0 && hop < 1) {
            const h = Math.sin(hop * Math.PI);
            dy = -cell * 0.16 * h;
            sx = 1 - 0.06 * h;
            sy = 1 + 0.06 * h;
          }
        }
        if (scene.placed?.cells.has(i)) {
          // Lands squashed, springs up a little tall, then settles on its bottom edge.
          const t = clamp01((now - scene.placed.start) / PLACE_DURATION);
          const k = Math.cos(t * Math.PI * 2.5) * (1 - t) * (1 - t);
          const grow = 0.9 + 0.1 * easeOutCubic(Math.min(1, t * 3));
          sx = grow * (1 + 0.16 * k);
          sy = grow * (1 - 0.16 * k);
          dy = (cell * (1 - sy)) / 2;
          impact = 1 - t / 0.35;
        } else if (ripple) {
          // Neighbours dip as the wave from the placed piece passes them.
          const d = Math.hypot(r + 0.5 - ripple.r, c + 0.5 - ripple.c);
          const t = (now - ripple.start - d * RIPPLE_STEP) / RIPPLE_BOB;
          if (d < RIPPLE_REACH && t > 0 && t < 1) sx = sy = 1 - 0.1 * (1 - d / RIPPLE_REACH) * Math.sin(t * Math.PI);
        }
        this.blit(color, x, y + dy, cell, dpr, 1, sx, sy);
        if (impact > 0) {
          ctx.fillStyle = `rgba(255,255,255,${0.5 * impact})`;
          roundRect(ctx, x + cell * 0.1, y + dy + cell * 0.1, cell * 0.8, cell * 0.8, cell * 0.14);
          ctx.fill();
        }
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

  private blit(color: number, x: number, y: number, size: number, dpr: number, alpha = 1, scale = 1, scaleY = scale): void {
    const { ctx } = this;
    // Animated pieces change size every frame; caching a sprite per size would
    // create hundreds of canvases, so they all scale the board-sized one.
    const sprite = blockSprite(color, this.spriteSize, dpr, this.style);
    const w = size * scale;
    const h = size * scaleY;
    if (alpha !== 1) ctx.globalAlpha = alpha;
    ctx.drawImage(sprite, x + (size - w) / 2, y + (size - h) / 2, w, h);
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
    const shown = tray.flatMap((piece, i) => {
      if (!piece || i === scene.hiddenSlot || scene.flying?.slot === i) return [];
      const t = clamp01((now - scene.trayDealtAt - i * 60) / DEAL_DURATION);
      const offset = (1 - easeOutCubic(t)) * layout.width * 0.6;
      const alpha = (scene.playable[i] ? 1 : 0.35) * clamp01(t * 2);
      return alpha > 0 ? [{ piece, i, offset, alpha }] : [];
    });
    // Halos go first so a neighbour's halo never covers a piece.
    if (scene.lucky) for (const { i, offset, alpha } of shown) this.drawLuckyGlow(scene, i, offset, alpha);
    for (const { piece, i, offset, alpha } of shown) {
      const pos = trayPiecePosition(layout, i, piece.shape);
      this.drawPiece(piece.shape, scene.playable[i] ? piece.color : 0, pos.x + offset, pos.y, pos.cell, alpha);
      if (scene.lucky && scene.playable[i]) this.drawLuckyShine(scene, piece.shape, i, pos.x + offset, pos.y, pos.cell);
    }
  }

  /** A shine sweeping diagonally over each piece of a lucky tray once it has slid in. */
  private drawLuckyShine(scene: Scene, shape: Shape, slot: number, x: number, y: number, cell: number): void {
    const t = (scene.now - scene.trayDealtAt - LUCKY_SHINE_DELAY - slot * LUCKY_SHINE_STAGGER) / LUCKY_SHINE_DURATION;
    if (t <= 0 || t >= 1) return;
    const { ctx } = this;
    const w = shape.cols * cell;
    const h = shape.rows * cell;
    ctx.save();
    ctx.beginPath();
    for (const [r, c] of shape.cells) ctx.rect(x + c * cell + cell * 0.06, y + r * cell + cell * 0.06, cell * 0.88, cell * 0.88);
    ctx.clip();
    // The gradient itself slides along the diagonal; beyond its ends it stays clear.
    const band = 0.22;
    const mid = -band + easeOutCubic(t) * (1 + band * 2);
    const grad = ctx.createLinearGradient(x + w * (mid - band), y + h * (mid - band), x + w * (mid + band), y + h * (mid + band));
    grad.addColorStop(0, 'rgba(255,255,255,0)');
    grad.addColorStop(0.5, 'rgba(255,255,255,0.8)');
    grad.addColorStop(1, 'rgba(255,255,255,0)');
    ctx.globalCompositeOperation = 'lighter';
    ctx.fillStyle = grad;
    ctx.fillRect(x, y, w, h);
    ctx.restore();
  }

  /** Soft halo behind a piece of a lucky tray, so help from the dealer is visible. */
  private drawLuckyGlow(scene: Scene, slot: number, offset: number, alpha: number): void {
    const { ctx } = this;
    const s = scene.layout.slots[slot];
    const radius = scene.layout.trayCell * 3;
    const glow = ctx.createRadialGradient(s.cx + offset, s.cy, 0, s.cx + offset, s.cy, radius);
    glow.addColorStop(0, rgba(scene.style.palette.glow, 0.42 * alpha));
    glow.addColorStop(1, rgba(scene.style.palette.glow, 0));
    ctx.fillStyle = glow;
    ctx.fillRect(s.cx + offset - radius, s.cy - radius, radius * 2, radius * 2);
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
    // Block debris, confetti and dust are solid; sparks and motes add light.
    let blend: GlobalCompositeOperation = 'source-over';
    const setBlend = (op: GlobalCompositeOperation) => {
      if (op !== blend) ctx.globalCompositeOperation = blend = op;
    };
    for (const p of scene.particles) {
      if (scene.now < p.born) continue;
      const age = (scene.now - p.born) / 1000;
      const k = 1 - (scene.now - p.born) / p.life;
      const x = p.x + p.vx * age;
      const y = p.y + p.vy * age + p.gravity * age * age;
      if (p.kind) {
        this.drawSpecialParticle(p, x, y, age, Math.max(0, k), setBlend);
        continue;
      }
      setBlend(glow ? 'lighter' : 'source-over');
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
    ctx.globalCompositeOperation = 'source-over';
  }

  /** Draws a particle with a `kind`; `k` runs from 1 at birth to 0 at the end of its life. */
  private drawSpecialParticle(p: Particle, x: number, y: number, age: number, k: number, setBlend: (op: GlobalCompositeOperation) => void): void {
    const { ctx } = this;
    switch (p.kind) {
      case 'dust': {
        // A soft puff that swells and thins out: a faint outer cloud around a denser core.
        setBlend('source-over');
        const r = (p.size * (1.8 - k)) / 2;
        ctx.fillStyle = rgba(p.color, 0.22 * k);
        ctx.beginPath();
        ctx.arc(x, y, r, 0, Math.PI * 2);
        ctx.fill();
        ctx.beginPath();
        ctx.arc(x, y, r * 0.55, 0, Math.PI * 2);
        ctx.fill();
        break;
      }
      case 'block': {
        // A chunk of a cleared block, tumbling away.
        setBlend('source-over');
        const s = p.size * (0.5 + 0.5 * k);
        ctx.globalAlpha = Math.min(1, k * 2.5);
        ctx.save();
        ctx.translate(x, y);
        ctx.rotate(p.spin * age);
        ctx.drawImage(blockSprite(p.block ?? 0, this.spriteSize, this.dpr, this.style), -s / 2, -s / 2, s, s);
        ctx.restore();
        ctx.globalAlpha = 1;
        break;
      }
      case 'confetti': {
        // A paper strip swaying as it falls and flipping over, so its width flickers.
        setBlend('source-over');
        const sx = x + Math.sin(age * 3 + p.spin) * p.size * 1.6;
        ctx.fillStyle = rgba(p.color, Math.min(1, k * 4));
        ctx.save();
        ctx.translate(sx, y);
        ctx.rotate(p.spin * 0.35 + age * p.spin * 0.4);
        ctx.scale(1, Math.cos(age * (5 + Math.abs(p.spin))));
        ctx.fillRect(-p.size / 2, -p.size * 0.3, p.size, p.size * 0.6);
        ctx.restore();
        break;
      }
      case 'mote': {
        // A soft glint drifting upwards, twinkling as it goes.
        setBlend('lighter');
        const twinkle = 0.65 + 0.35 * Math.sin(age * 9 + p.spin * 3);
        const s = p.size * (0.6 + 0.4 * twinkle);
        const mx = x + Math.sin(age * 2.2 + p.spin) * this.spriteSize * 0.25;
        ctx.fillStyle = rgba(p.color, Math.sin(k * Math.PI) * twinkle * 0.9);
        ctx.beginPath();
        ctx.arc(mx, y, s, 0, Math.PI * 2);
        ctx.fill();
        ctx.fillStyle = rgba(p.color, Math.sin(k * Math.PI) * twinkle * 0.25);
        ctx.beginPath();
        ctx.arc(mx, y, s * 2.6, 0, Math.PI * 2);
        ctx.fill();
        break;
      }
    }
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

  /** Rows and columns the dragged piece would clear pulse with light, framed in the glow colour. */
  private drawClearPreview(scene: Scene): void {
    const lines = scene.drag?.target?.lines;
    if (!lines || lines.rows.length + lines.cols.length === 0) return;
    const { ctx } = this;
    const { cell, boardX, boardY, boardSize } = scene.layout;
    const { palette } = scene.style;
    const pulse = 0.5 + 0.5 * Math.sin(scene.now / 110);
    const rects = [
      ...lines.rows.map((r) => [boardX, boardY + r * cell, boardSize, cell]),
      ...lines.cols.map((c) => [boardX + c * cell, boardY, cell, boardSize]),
    ];
    ctx.globalCompositeOperation = 'lighter';
    ctx.fillStyle = `rgba(255,255,255,${0.06 + 0.12 * pulse})`;
    for (const [x, y, w, h] of rects) ctx.fillRect(x, y, w, h);
    ctx.globalCompositeOperation = 'source-over';
    ctx.strokeStyle = rgba(palette.glow, 0.45 + 0.4 * pulse);
    ctx.lineWidth = Math.max(1.5, cell * (0.05 + 0.03 * pulse));
    for (const [x, y, w, h] of rects) {
      roundRect(ctx, x + 1, y + 1, w - 2, h - 2, cell * 0.16);
      ctx.stroke();
    }
  }

  /** Bright streaks racing from where the piece landed to both ends of each cleared line. */
  private drawStreaks(scene: Scene): void {
    if (!scene.streaks.length) return;
    const { ctx } = this;
    const { cell, boardX, boardY } = scene.layout;
    ctx.globalCompositeOperation = 'lighter';
    for (const st of scene.streaks) {
      const t = clamp01((scene.now - st.start) / STREAK_DURATION);
      if (t <= 0) continue;
      const horizontal = st.row !== undefined;
      const across = horizontal ? boardY + (st.row! + 0.5) * cell : boardX + (st.col! + 0.5) * cell;
      const origin = horizontal ? boardX : boardY;
      const width = cell * 0.26 * (1 - t * 0.6);
      const length = cell * (1 + 2.5 * t);
      for (const dir of [-1, 1]) {
        // Both halves reach just past their end of the line at the same moment.
        const reach = (dir > 0 ? SIZE - st.from : st.from) + 0.8;
        const head = origin + (st.from + dir * easeOutCubic(t) * reach) * cell;
        const tail = head - dir * length;
        const grad = horizontal ? ctx.createLinearGradient(tail, 0, head, 0) : ctx.createLinearGradient(0, tail, 0, head);
        grad.addColorStop(0, rgba(st.color, 0));
        grad.addColorStop(0.7, rgba(st.color, 0.7 * (1 - t)));
        grad.addColorStop(1, `rgba(255,255,255,${1 - t})`);
        ctx.fillStyle = grad;
        const a = Math.min(head, tail);
        if (horizontal) ctx.fillRect(a, across - width / 2, length, width);
        else ctx.fillRect(across - width / 2, a, width, length);
      }
    }
    ctx.globalCompositeOperation = 'source-over';
  }

  /** A burst of light from the cleared lines; big clears also tint the whole screen for a moment. */
  private drawFlash(scene: Scene): void {
    const f = scene.flash;
    if (!f) return;
    const t = clamp01((scene.now - f.start) / FLASH_DURATION);
    if (t <= 0) return;
    const { ctx } = this;
    const { cell, width, height } = scene.layout;
    const fade = (1 - t) * (1 - t);
    ctx.globalCompositeOperation = 'lighter';
    if (f.power > 0.5 && !prefersReducedMotion()) {
      ctx.fillStyle = rgba(f.color, 0.18 * (f.power - 0.4) * fade);
      ctx.fillRect(0, 0, width, height);
    }
    const radius = cell * (2.5 + 4 * f.power) * (0.5 + 0.5 * easeOutCubic(t));
    const grad = ctx.createRadialGradient(f.x, f.y, 0, f.x, f.y, radius);
    grad.addColorStop(0, `rgba(255,255,255,${0.6 * f.power * fade})`);
    grad.addColorStop(0.35, rgba(f.color, 0.45 * f.power * fade));
    grad.addColorStop(1, rgba(f.color, 0));
    ctx.fillStyle = grad;
    ctx.fillRect(f.x - radius, f.y - radius, radius * 2, radius * 2);
    ctx.globalCompositeOperation = 'source-over';
  }

  /** Soft glow under the dragged piece, brighter and pulsing when the drop would clear lines. */
  private drawDragHalo(scene: Scene, d: DragView): void {
    const { ctx } = this;
    const { palette, tier } = scene.style;
    const w = d.piece.shape.cols * d.cell;
    const h = d.piece.shape.rows * d.cell;
    const cx = d.x + w / 2;
    const cy = d.y + h / 2;
    const lines = d.target ? d.target.lines.rows.length + d.target.lines.cols.length : 0;
    const strength = lines ? 0.4 + 0.2 * Math.sin(scene.now / 110) : 0.22;
    const radius = Math.max(w, h) * 0.6 + d.cell * 0.9;
    const grad = ctx.createRadialGradient(cx, cy, 0, cx, cy, radius);
    grad.addColorStop(0, rgba(palette.glow, strength));
    grad.addColorStop(1, rgba(palette.glow, 0));
    if (tier.fx.glow) ctx.globalCompositeOperation = 'lighter';
    ctx.fillStyle = grad;
    ctx.fillRect(cx - radius, cy - radius, radius * 2, radius * 2);
    ctx.globalCompositeOperation = 'source-over';
  }

  /** Light pulsing in from the screen edges on combos and board clears. */
  private drawPulse(scene: Scene): void {
    const p = scene.pulse;
    if (!p) return;
    const t = clamp01((scene.now - p.start) / PULSE_DURATION);
    if (t <= 0) return;
    const { ctx } = this;
    const { width, height } = scene.layout;
    const envelope = t < 0.18 ? t / 0.18 : 1 - easeOutCubic((t - 0.18) / 0.82);
    const alpha = p.power * envelope * (prefersReducedMotion() ? 0.35 : 0.7);
    const depth = Math.min(width, height) * (0.08 + 0.08 * p.power);
    const edge = (x0: number, y0: number, x1: number, y1: number) => {
      const grad = ctx.createLinearGradient(x0, y0, x1, y1);
      grad.addColorStop(0, rgba(p.color, alpha));
      grad.addColorStop(1, rgba(p.color, 0));
      return grad;
    };
    if (scene.style.tier.fx.glow) ctx.globalCompositeOperation = 'lighter';
    ctx.fillStyle = edge(0, 0, 0, depth);
    ctx.fillRect(0, 0, width, depth);
    ctx.fillStyle = edge(0, height, 0, height - depth);
    ctx.fillRect(0, height - depth, width, depth);
    ctx.fillStyle = edge(0, 0, depth, 0);
    ctx.fillRect(0, 0, depth, height);
    ctx.fillStyle = edge(width, 0, width - depth, 0);
    ctx.fillRect(width - depth, 0, depth, height);
    ctx.globalCompositeOperation = 'source-over';
  }

  /** The screen dims towards the edges once no piece fits any more. */
  private drawGameOverShade(scene: Scene): void {
    if (scene.gameOverAt === null) return;
    const t = clamp01((scene.now - scene.gameOverAt) / GAME_OVER_FILL);
    if (t <= 0) return;
    const { ctx } = this;
    const { width, height, boardX, boardY, boardSize } = scene.layout;
    const cx = boardX + boardSize / 2;
    const cy = boardY + boardSize / 2;
    const grad = ctx.createRadialGradient(cx, cy, boardSize * 0.45, cx, cy, Math.hypot(width, height) * 0.6);
    grad.addColorStop(0, 'rgba(0,0,0,0)');
    grad.addColorStop(1, `rgba(0,0,0,${0.45 * easeOutCubic(t)})`);
    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, width, height);
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
      let tilt = 0;
      if (ft.banner) {
        scale = t < 0.2 ? easeOutBack(t / 0.2) : 1;
        alpha = t > 0.75 ? 1 - (t - 0.75) / 0.25 : 1;
        dy = -t * 20;
      } else {
        // Small popups punch in, then float up.
        scale = t < 0.12 ? 0.4 + 0.6 * easeOutBack(t / 0.12) : 1;
        dy = -easeOutCubic(t) * 50;
        alpha = t > 0.6 ? 1 - (t - 0.6) / 0.4 : 1;
      }
      if (ft.bounce && t >= 0.2) {
        // Combo counters keep wobbling, dying down as they fade.
        const w = Math.sin((t - 0.2) * Math.PI * 7) * (1 - t);
        scale *= 1 + 0.12 * w;
        tilt = 0.06 * w;
      }
      ctx.save();
      ctx.globalAlpha = Math.max(0, alpha);
      // Scale the text down if it would not fit the screen in this style's font.
      ctx.font = `${tier.fontWeight} ${ft.size}px ${tier.font}`;
      const textWidth = Math.max(1, ctx.measureText(ft.text).width);
      const fit = Math.min(1, (scene.layout.width * 0.92) / textWidth);
      // Popups near the screen edges move in so they stay readable.
      const half = (textWidth * fit) / 2 + ft.size * 0.15;
      const x = ft.banner ? ft.x : Math.max(half, Math.min(scene.layout.width - half, ft.x));
      ctx.translate(x, ft.y + dy);
      if (tilt) ctx.rotate(tilt);
      ctx.scale(Math.max(0.01, scale), Math.max(0.01, scale));
      if (fit < 1) ctx.scale(fit, fit);
      ctx.lineJoin = 'round';
      ctx.lineWidth = Math.max(3, ft.size * 0.14);
      ctx.strokeStyle = rgba(palette.boardEdge, 0.9);
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
