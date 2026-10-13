import { type Board, type Lines, SIZE, idx } from '../game/board';
import type { Piece } from '../game/game';
import type { Shape } from '../game/shapes';
import type { Layout } from './layout';
import { THEME, rgba } from './palette';
import { blockSprite } from './sprites';

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
  texts: FloatingText[];
  shake: { start: number; power: number } | null;
  gameOverAt: number | null;
}

export const CLEAR_DURATION = 420;
export const PLACE_DURATION = 180;
export const DEAL_DURATION = 320;
export const GAME_OVER_FILL = 900;

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
      scene.texts.length > 0 ||
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
    ctx.restore();

    this.drawTray(scene);
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
    scene.particles = scene.particles.filter((p) => t - p.born < p.life);
    scene.texts = scene.texts.filter((p) => t - p.start < p.duration);
  }

  private boardBackground(layout: Layout, dpr: number): HTMLCanvasElement {
    const key = `${layout.boardSize}:${dpr}`;
    if (this.boardCache && this.boardCacheKey === key) return this.boardCache;
    const pad = Math.round(layout.cell * 0.18);
    const size = layout.boardSize + pad * 2;
    const c = document.createElement('canvas');
    c.width = c.height = Math.round(size * dpr);
    const g = c.getContext('2d')!;
    g.scale(dpr, dpr);
    g.fillStyle = THEME.boardEdge;
    roundRect(g, 0, 0, size, size, layout.cell * 0.3);
    g.fill();
    g.fillStyle = THEME.board;
    roundRect(g, pad * 0.35, pad * 0.35, size - pad * 0.7, size - pad * 0.7, layout.cell * 0.25);
    g.fill();
    const cell = layout.cell;
    const inset = Math.max(1, cell * 0.04);
    for (let r = 0; r < SIZE; r++) {
      for (let col = 0; col < SIZE; col++) {
        const x = pad + col * cell + inset;
        const y = pad + r * cell + inset;
        g.fillStyle = THEME.cell;
        roundRect(g, x, y, cell - inset * 2, cell - inset * 2, cell * 0.1);
        g.fill();
        g.fillStyle = THEME.cellInner;
        roundRect(g, x + inset, y + inset * 2, cell - inset * 4, cell - inset * 4, cell * 0.08);
        g.fill();
      }
    }
    this.boardCache = c;
    this.boardCacheKey = key;
    return c;
  }

  private drawBoard(scene: Scene): void {
    const { ctx } = this;
    const { layout, board, drag, now, dpr } = scene;
    const bg = this.boardBackground(layout, dpr);
    const pad = Math.round(layout.cell * 0.18);
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
          ctx.fillStyle = `rgba(255,255,255,${0.7 * (1 - t / 0.3)})`;
          ctx.fillRect(x + cell * 0.08, y + cell * 0.08, cell * 0.84, cell * 0.84);
        }
      }
    }
  }

  private blit(color: number, x: number, y: number, size: number, dpr: number, alpha = 1, scale = 1): void {
    const { ctx } = this;
    // Animated pieces change size every frame; caching a sprite per size would
    // create hundreds of canvases, so they all scale the board-sized one.
    const sprite = blockSprite(color, this.spriteSize, dpr);
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
    for (const p of scene.particles) {
      const age = (scene.now - p.born) / 1000;
      const k = 1 - (scene.now - p.born) / p.life;
      const x = p.x + p.vx * age;
      const y = p.y + p.vy * age + 900 * age * age;
      ctx.fillStyle = rgba(p.color, Math.max(0, k));
      const s = p.size * (0.4 + 0.6 * k);
      ctx.fillRect(x - s / 2, y - s / 2, s, s);
    }
  }

  private drawTexts(scene: Scene): void {
    const { ctx } = this;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    for (const ft of scene.texts) {
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
      ctx.font = `800 ${ft.size}px ${THEME.font}`;
      ctx.lineJoin = 'round';
      ctx.lineWidth = Math.max(3, ft.size * 0.14);
      ctx.strokeStyle = 'rgba(20,24,60,0.85)';
      ctx.strokeText(ft.text, 0, 0);
      ctx.fillStyle = ft.color;
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
