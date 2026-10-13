import { audioContext } from './audio/context';
import { ActionMeter, scoreProgress } from './audio/intensity';
import type { Music } from './audio/music';
import type { Sound } from './audio/sound';
import { idx, previewLines } from './game/board';
import { Game, type PlaceResult } from './game/game';
import { shapeById } from './game/shapes';
import { TOUCH_GAIN, nearestFit } from './input';
import { canVibrate, exitApp, isNative, onBackButton, vibrate } from './platform';
import { type Layout, computeLayout, slotAt } from './render/layout';
import { colorFor, shadeHex } from './render/palette';
import { CLEAR_DURATION, GAME_OVER_FILL, type DragView, Renderer, type Scene, trayPiecePosition } from './render/renderer';
import { clearSpriteCache, spriteCacheSize } from './render/sprites';
import { type Style, formatScore, nextTier, styleFor } from './render/styles';
import { type Settings, storage } from './storage';

interface DragState {
  slot: number;
  pointerId: number;
  /** Last pointer position. */
  px: number;
  py: number;
  /** Top-left corner of the piece at full size. */
  x: number;
  y: number;
  /** How far the piece moves per pixel of pointer movement. */
  gain: { x: number; y: number };
  /** Where the piece started (in the tray), for the pick-up animation. */
  from: { x: number; y: number; cell: number };
  start: number;
}

const PICKUP_MS = 110;
const PRAISE = ['', '', 'Good!', 'Great!', 'Excellent!', 'Amazing!', 'Unbelievable!'];

function $(id: string): HTMLElement {
  return document.getElementById(id)!;
}

export class BlockBasterApp {
  game: Game;
  layout: Layout;
  best: number;
  settings: Settings;

  private renderer: Renderer;
  private scene: Scene;
  private drag: DragState | null = null;
  private shownScore = 0;
  private locked = false;
  private frame = 0;
  private gameOverTimer = 0;
  /** A style earned by the last move, applied on the first frame at or after `at`. */
  private pendingStyle: { style: Style; at: number } | null = null;
  private bestAtStart = 0;
  /** How fast the player is moving, for the music. */
  private meter = new ActionMeter();

  constructor(
    private canvas: HTMLCanvasElement,
    private sound: Sound,
    readonly music: Music,
    seed?: number,
  ) {
    this.settings = storage.loadSettings();
    this.sound.volume = this.settings.sound;
    this.music.volume = this.settings.music;
    this.best = storage.loadBest();
    this.game = this.restoreOrCreate(seed);
    this.bestAtStart = this.game.moves > 0 ? (storage.loadBestAtStart() ?? this.best) : this.best;
    storage.saveBestAtStart(this.bestAtStart);
    this.shownScore = this.game.score;
    this.renderer = new Renderer(canvas);
    this.layout = computeLayout(canvas.clientWidth || 360, canvas.clientHeight || 600);
    this.scene = {
      layout: this.layout,
      dpr: window.devicePixelRatio || 1,
      now: performance.now(),
      style: styleFor(this.game.score, this.game.boardClears),
      styleWaveAt: null,
      board: this.game.board,
      tray: this.game.tray,
      lucky: this.game.lucky,
      playable: [],
      hiddenSlot: -1,
      drag: null,
      flying: null,
      trayDealtAt: performance.now(),
      placed: null,
      clearing: null,
      particles: [],
      rings: [],
      beams: [],
      texts: [],
      shake: null,
      gameOverAt: null,
    };
    this.applyStyle(this.scene.style);
    this.syncScene();
    this.bindInput();
    this.bindUi();
    this.resize();
    new ResizeObserver(() => this.resize()).observe(canvas.parentElement!);
    this.updateHud(true);
    this.music.setInput(() => this.musicInput());
  }

  private restoreOrCreate(seed?: number): Game {
    if (seed === undefined) {
      const saved = storage.loadGame();
      if (saved && !saved.over) {
        try {
          return Game.deserialize(saved);
        } catch {
          // Fall through to a fresh game when the save is corrupt or outdated.
        }
      }
    }
    return new Game(seed);
  }

  // ---------------------------------------------------------------------------
  // Rendering loop
  // ---------------------------------------------------------------------------

  /** Requests a frame; the loop keeps running only while something animates. */
  invalidate(): void {
    if (!this.frame) this.frame = requestAnimationFrame((t) => this.tick(t));
  }

  private tick(now: number): void {
    this.frame = 0;
    const s = this.scene;
    s.now = now;
    s.drag = this.dragView(now);
    if (this.pendingStyle && now >= this.pendingStyle.at) {
      this.switchStyle(this.pendingStyle.style, now);
      this.pendingStyle = null;
    }
    this.renderer.render(s);
    this.updateHud();
    if (this.renderer.isAnimating(s) || this.drag || this.pendingStyle || this.shownScore !== this.game.score) this.invalidate();
  }

  private resize(): void {
    const rect = this.canvas.getBoundingClientRect();
    this.layout = computeLayout(rect.width, rect.height);
    const dpr = Math.min(3, window.devicePixelRatio || 1);
    if (dpr !== this.scene.dpr || this.layout.cell !== this.scene.layout.cell) clearSpriteCache();
    this.scene.layout = this.layout;
    this.scene.dpr = dpr;
    this.invalidate();
  }

  private syncScene(): void {
    this.scene.board = this.game.board;
    this.scene.tray = this.game.tray;
    this.scene.lucky = this.game.lucky;
    this.scene.playable = this.game.tray.map((_, i) => this.game.isPlayable(i));
  }

  private updateHud(force = false): void {
    const target = this.game.score;
    if (this.shownScore !== target || force) {
      const diff = target - this.shownScore;
      this.shownScore = Math.abs(diff) <= 1 ? target : this.shownScore + Math.ceil(Math.abs(diff) * 0.18) * Math.sign(diff);
      $('score').textContent = String(this.shownScore);
      this.updateTierProgress();
    }
    // Runs every frame while dragging; only touch the DOM when the text changes.
    const best = String(Math.max(this.best, this.game.score));
    if ($('best').textContent !== best) $('best').textContent = best;
  }

  /** Progress bar towards the next style tier. */
  private updateTierProgress(): void {
    const { tier } = this.scene.style;
    const next = nextTier(tier);
    const progress = next ? (this.shownScore - tier.minScore) / (next.minScore - tier.minScore) : 1;
    $('tier-bar').style.transform = `scaleX(${Math.max(0, Math.min(1, progress))})`;
    $('tier-next').textContent = next ? formatScore(next.minScore) : 'MAX';
    $('style-info').title = next ? `Next style: ${next.name} at ${next.minScore.toLocaleString('en-US')} points` : 'Top style unlocked';
  }

  // ---------------------------------------------------------------------------
  // Styles
  // ---------------------------------------------------------------------------

  /** Switches the board, blocks, page colours, fonts and sounds to a style. */
  applyStyle(style: Style): void {
    this.scene.style = style;
    clearSpriteCache();
    this.sound.profile = style.tier.sound;
    const { palette: p, tier } = style;
    const vars: Record<string, string> = {
      '--bg-top': p.bg[0],
      '--bg-mid': p.bg[1],
      '--bg-bottom': p.bg[2],
      '--card': p.card,
      '--card-edge': p.cardEdge,
      '--accent': p.accent,
      '--accent-dark': shadeHex(p.accent, -0.28),
      '--btn': shadeHex(p.card, 0.14),
      '--btn-edge': p.boardEdge,
      '--glow': p.glow,
      '--glow2': p.glow2,
      '--font-display': tier.font,
      '--font-display-weight': String(tier.fontWeight),
    };
    const root = document.documentElement;
    for (const [k, v] of Object.entries(vars)) root.style.setProperty(k, v);
    document.body.dataset.tier = String(tier.level);
    document.body.dataset.style = style.key;
    document.querySelector('meta[name=theme-color]')?.setAttribute('content', p.bg[1]);
    $('style-name').textContent = `${tier.name} · ${p.name}`;
    this.updateTierProgress();
    // Redraw once the tier's font is ready, so canvas text doesn't stay in the fallback font.
    document.fonts
      ?.load(`${tier.fontWeight} 32px ${tier.font}`)
      .then(() => this.invalidate())
      .catch(() => {});
    this.invalidate();
  }

  /**
   * Schedules the style earned by the current game to appear after the line-clear animation.
   * It runs on the animation clock rather than a timer, so it lands on the same frame every time.
   */
  private updateStyle(delay: number): void {
    const style = styleFor(this.game.score, this.game.boardClears);
    this.pendingStyle = style.key === this.scene.style.key ? null : { style, at: performance.now() + delay };
    this.invalidate();
  }

  /** Applies a newly earned style with its reveal: a shimmer, a banner, sounds and, for a new tier, fireworks. */
  private switchStyle(style: Style, now: number): void {
    const prev = this.scene.style;
    const s = this.scene;
    const { cell, boardY, boardSize, width } = this.layout;
    const mid = boardY + boardSize / 2;
    this.applyStyle(style);
    s.styleWaveAt = now;
    if (style.tier.level > prev.tier.level) {
      const at = now + 700;
      s.texts.push({ text: 'New style!', x: width / 2, y: mid - cell * 0.75, size: cell * 0.6, color: style.palette.glow, start: at, duration: 2200, banner: true });
      s.texts.push({ text: style.tier.name, x: width / 2, y: mid + cell * 0.35, size: cell * 1.25, color: style.palette.accent, start: at + 120, duration: 2100, banner: true });
      this.sound.tierUp(style.tier.level);
      this.vibrate('heavy');
      this.fireworks(Math.max(3, style.tier.fx.fireworks), now + 600);
    } else {
      s.texts.push({ text: style.palette.name, x: width / 2, y: mid + cell * 2.1, size: cell * 0.6, color: style.palette.glow, start: now + 500, duration: 1500, banner: true });
      this.sound.styleShift();
    }
  }

  // ---------------------------------------------------------------------------
  // Dragging
  // ---------------------------------------------------------------------------

  private bindInput(): void {
    const c = this.canvas;
    const point = (e: PointerEvent) => {
      const r = c.getBoundingClientRect();
      return { x: e.clientX - r.left, y: e.clientY - r.top };
    };
    c.addEventListener('pointerdown', (e) => {
      this.unlockAudio();
      if (this.drag || this.locked || this.game.over) return;
      const { x, y } = point(e);
      const slot = slotAt(this.layout, x, y);
      const piece = slot >= 0 ? this.game.tray[slot] : null;
      if (!piece) return;
      e.preventDefault();
      try {
        c.setPointerCapture(e.pointerId);
      } catch {
        // Synthetic or already-released pointers can't be captured; dragging still works.
      }
      const cell = this.layout.cell;
      const w = piece.shape.cols * cell;
      const h = piece.shape.rows * cell;
      // Touch: lift the piece above the finger so it stays visible. Mouse: grab the centre.
      const mouse = e.pointerType === 'mouse';
      this.drag = {
        slot,
        pointerId: e.pointerId,
        px: x,
        py: y,
        x: x - w / 2,
        y: mouse ? y - h / 2 : y - h - cell * 1.1,
        gain: mouse ? { x: 1, y: 1 } : TOUCH_GAIN,
        from: trayPiecePosition(this.layout, slot, piece.shape),
        start: performance.now(),
      };
      this.scene.hiddenSlot = slot;
      this.scene.flying = null;
      this.sound.pickup();
      this.invalidate();
    });
    c.addEventListener('pointermove', (e) => {
      const d = this.drag;
      if (!d || e.pointerId !== d.pointerId) return;
      const { x, y } = point(e);
      const shape = this.game.tray[d.slot]!.shape;
      const w = shape.cols * this.layout.cell;
      const h = shape.rows * this.layout.cell;
      // Move by the scaled delta and keep the piece's centre on screen, so it
      // follows the finger back at once after overshooting an edge.
      d.x = Math.min(this.layout.width - w / 2, Math.max(-w / 2, d.x + (x - d.px) * d.gain.x));
      d.y = Math.min(this.layout.height - h / 2, Math.max(-h / 2, d.y + (y - d.py) * d.gain.y));
      d.px = x;
      d.py = y;
      this.invalidate();
    });
    // The drop uses the last move: fingers often slide a little while lifting off.
    const end = (e: PointerEvent, cancelled: boolean) => {
      if (!this.drag || e.pointerId !== this.drag.pointerId) return;
      this.drop(cancelled);
    };
    c.addEventListener('pointerup', (e) => end(e, false));
    c.addEventListener('pointercancel', (e) => end(e, true));
    c.addEventListener('contextmenu', (e) => e.preventDefault());
  }

  /** Where the dragged piece would land: the closest spot near it where it fits. */
  private snapTarget(d: DragState) {
    if (!this.game.tray[d.slot]) return null;
    const { boardX, boardY, cell } = this.layout;
    return nearestFit((d.y - boardY) / cell, (d.x - boardX) / cell, (r, c) => this.game.canPlace(d.slot, r, c));
  }

  private dragView(now: number): DragView | null {
    const d = this.drag;
    if (!d) return null;
    const piece = this.game.tray[d.slot];
    if (!piece) return null;
    const t = Math.min(1, (now - d.start) / PICKUP_MS);
    const to = { x: d.x, y: d.y };
    const lerp = (a: number, b: number) => a + (b - a) * t;
    const target = this.snapTarget(d);
    return {
      piece,
      x: lerp(d.from.x, to.x),
      y: lerp(d.from.y, to.y),
      cell: lerp(d.from.cell, this.layout.cell),
      target: target ? { ...target, lines: previewLines(this.game.board, piece.shape, target.r, target.c) } : null,
    };
  }

  private drop(cancelled: boolean): void {
    const d = this.drag!;
    const view = this.dragView(performance.now());
    this.drag = null;
    this.scene.hiddenSlot = -1;
    const target = cancelled ? null : this.snapTarget(d);
    if (target) {
      this.commit(d.slot, target.r, target.c);
    } else if (view) {
      // Fly back to the tray.
      this.scene.flying = { piece: view.piece, from: { x: view.x, y: view.y, cell: view.cell }, slot: d.slot, start: performance.now(), duration: 200 };
      const { x, y } = d;
      const overBoard =
        x + view.piece.shape.cols * this.layout.cell > this.layout.boardX && x < this.layout.boardX + this.layout.boardSize && y < this.layout.boardY + this.layout.boardSize;
      if (overBoard) this.sound.invalid();
    }
    this.invalidate();
  }

  // ---------------------------------------------------------------------------
  // Moves & effects
  // ---------------------------------------------------------------------------

  /** Places the piece in `slot` at (r, c). Public so tests and tools can drive the game. */
  commit(slot: number, r: number, c: number): PlaceResult | null {
    const res = this.game.place(slot, r, c);
    if (!res) return null;
    const now = performance.now();
    const s = this.scene;
    const { cell, boardX, boardY } = this.layout;

    s.placed = { cells: new Set(res.piece.shape.cells.map(([dr, dc]) => idx(r + dr, c + dc))), start: now };
    const cx = boardX + (c + res.piece.shape.cols / 2) * cell;
    const cy = boardY + (r + res.piece.shape.rows / 2) * cell;
    const lines = res.lines.rows.length + res.lines.cols.length;

    this.sound.place();
    this.meter.move(now, this.game.combo);
    this.vibrate(lines ? 'medium' : 'light');

    if (lines > 0) {
      const pr = r + res.piece.shape.rows / 2;
      const pc = c + res.piece.shape.cols / 2;
      s.clearing = {
        start: now,
        cells: res.cleared.map((cc) => ({ ...cc, delay: Math.hypot(cc.r + 0.5 - pr, cc.c + 0.5 - pc) * 28 })),
      };
      const { palette, tier } = s.style;
      const fx = tier.fx;
      for (const cc of res.cleared) this.burst(boardX + (cc.c + 0.5) * cell, boardY + (cc.r + 0.5) * cell, colorFor(cc.color, palette), now);
      if (fx.beam) {
        const color = colorFor(res.piece.color, palette);
        for (const row of res.lines.rows) s.beams.push({ row, color, start: now });
        for (const col of res.lines.cols) s.beams.push({ col, color, start: now });
      }
      if (fx.ring) {
        s.rings.push({ x: cx, y: cy, radius: cell * (2 + lines), color: palette.glow, start: now, duration: 520 });
        if (lines >= 2) s.rings.push({ x: cx, y: cy, radius: cell * (3 + lines), color: colorFor(res.piece.color, palette), start: now + 90, duration: 620 });
      }
      if (lines >= 3 || (lines >= 2 && fx.shake > 1)) s.shake = { start: now, power: Math.min(14, lines * 2.5 * fx.shake) };
      this.sound.clear(lines, res.combo);
      if (lines >= 3) this.vibrate('heavy');

      const mid = boardY + this.layout.boardSize / 2;
      s.texts.push({ text: `+${res.points}`, x: cx, y: cy, size: cell * 0.7, color: '#ffffff', start: now, duration: 900 });
      if (res.boardCleared) {
        s.texts.push({ text: 'Board Clear!', x: this.layout.width / 2, y: mid, size: cell * 0.95, color: this.bannerColor(), start: now + 150, duration: 1400, banner: true });
        this.sound.boardClear();
        this.fireworks(fx.fireworks, now + 200);
      } else if (lines >= 2) {
        const word = PRAISE[Math.min(lines, PRAISE.length - 1)];
        s.texts.push({ text: word, x: this.layout.width / 2, y: mid, size: cell * 0.95, color: this.bannerColor(), start: now + 80, duration: 1100, banner: true });
      }
      if (res.combo >= 2) {
        s.texts.push({ text: `Combo ${res.combo}`, x: this.layout.width / 2, y: mid + cell * 1.05, size: cell * 0.62, color: palette.glow, start: now + 80, duration: 1100, banner: true });
      }
    }

    if (res.refilled) s.trayDealtAt = now + (lines ? 120 : 0);
    this.syncScene();

    if (this.game.score > this.best) {
      this.best = this.game.score;
      storage.saveBest(this.best);
    }
    storage.saveGame(this.game.over ? null : this.game.serialize());
    this.updateStyle(lines ? CLEAR_DURATION + 80 : 0);

    if (res.gameOver) this.startGameOver(lines ? CLEAR_DURATION + 250 : 300);
    this.invalidate();
    return res;
  }

  private vibrate(strength: 'light' | 'medium' | 'heavy'): void {
    vibrate(strength, this.settings.haptics);
  }

  private bannerColor(): string {
    return this.scene.style.palette.accent;
  }

  private burst(x: number, y: number, color: string, now: number, count?: number, speed = 1): void {
    const { tier, palette } = this.scene.style;
    const fx = tier.fx;
    const sparkles = [palette.glow, palette.glow2, palette.accent];
    const n = count ?? fx.particles;
    for (let i = 0; i < n; i++) {
      const a = Math.random() * Math.PI * 2;
      const v = (80 + Math.random() * 220) * speed;
      this.scene.particles.push({
        x,
        y,
        vx: Math.cos(a) * v,
        vy: Math.sin(a) * v - 220 * Math.min(1, fx.gravity / 900),
        size: this.layout.cell * (0.14 + Math.random() * 0.14) * (fx.shape === 'star' || fx.shape === 'diamond' ? 1.4 : 1),
        color: Math.random() < fx.sparkle ? sparkles[i % sparkles.length] : color,
        born: now + Math.random() * 120,
        life: 550 + Math.random() * 300 + (fx.gravity < 400 ? 250 : 0),
        shape: fx.shape,
        gravity: fx.gravity,
        spin: (Math.random() - 0.5) * 12,
      });
    }
  }

  /** Celebration bursts scattered over the board. */
  private fireworks(count: number, start: number): void {
    const { cell, boardX, boardY, boardSize } = this.layout;
    const { palette } = this.scene.style;
    const colors = [...palette.blocks, palette.glow, palette.accent];
    for (let i = 0; i < count; i++) {
      const x = boardX + boardSize * (0.15 + Math.random() * 0.7);
      const y = boardY + boardSize * (0.15 + Math.random() * 0.6);
      const at = start + i * 140 + Math.random() * 80;
      const color = colors[Math.floor(Math.random() * colors.length)];
      this.burst(x, y, color, at, 14, 1.6);
      if (this.scene.style.tier.fx.ring) this.scene.rings.push({ x, y, radius: cell * 1.8, color, start: at, duration: 600 });
    }
  }

  private startGameOver(delay: number): void {
    this.locked = true;
    const record = this.game.score > this.bestAtStart;
    this.gameOverTimer = window.setTimeout(() => {
      this.scene.gameOverAt = performance.now();
      this.sound.gameOver();
      this.vibrate('heavy');
      this.invalidate();
      this.gameOverTimer = window.setTimeout(() => this.showGameOver(record), GAME_OVER_FILL + 500);
    }, delay);
  }

  private showGameOver(record: boolean): void {
    $('final-score').textContent = String(this.game.score);
    $('final-best').textContent = String(this.best);
    $('record').hidden = !record;
    $('gameover').hidden = false;
    this.syncMusicPause();
    ($('again-btn') as HTMLButtonElement).focus({ preventScroll: true });
  }

  newGame(seed?: number): void {
    clearTimeout(this.gameOverTimer);
    this.pendingStyle = null;
    this.game = new Game(seed);
    this.bestAtStart = this.best;
    storage.saveBestAtStart(this.best);
    this.meter.reset();
    this.locked = false;
    this.drag = null;
    const s = this.scene;
    Object.assign(s, { hiddenSlot: -1, flying: null, placed: null, clearing: null, particles: [], rings: [], beams: [], texts: [], shake: null, gameOverAt: null, styleWaveAt: null });
    s.trayDealtAt = performance.now();
    this.shownScore = 0;
    const style = styleFor(0, 0);
    if (style.key !== s.style.key) {
      this.applyStyle(style);
      s.styleWaveAt = performance.now();
    }
    this.syncScene();
    storage.saveGame(this.game.serialize());
    $('gameover').hidden = true;
    $('menu').hidden = true;
    this.syncMusicPause();
    this.updateHud(true);
    this.invalidate();
  }

  /** Replaces the tray with the given shape ids (for tests and debugging). */
  debugSetTray(ids: (string | null)[]): void {
    this.game.tray = ids.map((id, i) => (id ? { shape: shapeById(id), color: i + 1 } : null));
    this.syncScene();
    this.invalidate();
  }

  /** Sizes of everything that accumulates while playing (for leak tests). */
  debugStats() {
    const s = this.scene;
    return { sprites: spriteCacheSize(), particles: s.particles.length, rings: s.rings.length, beams: s.beams.length, texts: s.texts.length, animating: this.renderer.isAnimating(s) };
  }

  save(): void {
    if (!this.game.over) storage.saveGame(this.game.serialize());
  }

  // ---------------------------------------------------------------------------
  // Audio
  // ---------------------------------------------------------------------------

  /** Unlocks audio (browsers need a user gesture) and starts the music when it's on. */
  private unlockAudio(): void {
    this.sound.unlock();
    if (!this.settings.music || this.music.playing) return;
    const ctx = audioContext();
    if (ctx) this.music.start(ctx);
  }

  /** What the soundtrack follows: progress towards the best score and the pace of play. */
  private musicInput() {
    if (this.game.over) return { score: 0, speed: 0 };
    return { score: scoreProgress(this.game.score, this.bestAtStart), speed: this.meter.speed(performance.now()) };
  }

  /** Muffles the music while a menu or the game over screen is up. */
  private syncMusicPause(): void {
    this.music.setPaused(!$('menu').hidden || !$('gameover').hidden);
  }

  // ---------------------------------------------------------------------------
  // Menus
  // ---------------------------------------------------------------------------

  /** Wires a 0–100% settings slider to a setting. */
  private bindLevel(key: keyof Settings, apply: (v: number) => void, preview?: () => void): void {
    const input = $(`${key}-level`) as HTMLInputElement;
    const output = $(`${key}-value`);
    const show = (v: number) => (output.textContent = v > 0 ? `${Math.round(v * 100)}%` : 'Off');
    input.value = String(Math.round(this.settings[key] * 100));
    show(this.settings[key]);
    input.addEventListener('input', () => {
      const v = Number(input.value) / 100;
      this.settings[key] = v;
      show(v);
      apply(v);
    });
    input.addEventListener('change', () => {
      storage.saveSettings(this.settings);
      if (this.settings[key] > 0) preview?.();
    });
  }

  private bindUi(): void {
    const menu = $('menu');
    $('haptics-row').hidden = !canVibrate;
    $('version').textContent = __APP_VERSION__;
    $('exit-btn').hidden = !isNative;

    const openMenu = () => {
      menu.hidden = false;
      this.syncMusicPause();
      ($('resume-btn') as HTMLButtonElement).focus({ preventScroll: true });
    };
    const closeMenu = () => {
      menu.hidden = true;
      this.syncMusicPause();
    };

    $('menu-btn').addEventListener('click', openMenu);
    $('resume-btn').addEventListener('click', closeMenu);
    $('restart-btn').addEventListener('click', () => this.newGame());
    $('again-btn').addEventListener('click', () => this.newGame());
    $('exit-btn').addEventListener('click', () => exitApp());
    menu.addEventListener('click', (e) => e.target === menu && closeMenu());
    // Sliders apply as they move; the sound and vibration preview once they are let go.
    this.bindLevel('sound', (v) => (this.sound.volume = v), () => {
      this.sound.unlock();
      this.sound.pickup();
    });
    this.bindLevel('music', (v) => {
      this.music.volume = v;
      if (v > 0) this.unlockAudio();
      else this.music.stop();
    });
    this.bindLevel('haptics', () => {}, () => this.vibrate('medium'));
    window.addEventListener('keydown', (e) => {
      if (e.key !== 'Escape') return;
      if (menu.hidden) openMenu();
      else closeMenu();
    });

    onBackButton(() => {
      if (!menu.hidden) closeMenu();
      else if (!$('gameover').hidden) exitApp();
      else openMenu();
    });

    document.addEventListener('visibilitychange', () => {
      if (document.hidden) {
        this.save();
        this.sound.suspend();
      } else {
        this.sound.resume();
      }
    });
    window.addEventListener('pagehide', () => this.save());
  }
}

